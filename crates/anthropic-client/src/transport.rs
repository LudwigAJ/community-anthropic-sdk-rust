//! Internal HTTP transport, retry policy, and response decoding.
//!
//! Not part of the public API. This module owns the single `reqwest::Client`,
//! applies merged client + per-request headers and timeouts, retries
//! retryable failures (connection errors, request timeouts, HTTP `408`,
//! `409`, `429`, `5xx`) up to [`crate::config::MaxRetries`], and decodes
//! Anthropic API error bodies into [`crate::ApiError`] with the response
//! [`anthropic_types::RequestId`] preserved.
//!
//! Public surface area lives in [`crate::client`], [`crate::messages`],
//! [`crate::models`], and [`crate::batches`]; those modules call into this
//! one but never expose its types.

use std::time::Duration;

use reqwest::{
    Response, StatusCode,
    header::{HeaderMap, HeaderName},
};
use serde::de::DeserializeOwned;

use crate::{ApiResponse, Error, MessageStream, request::PreparedRequest};

const REQUEST_ID_HEADER: &str = "request-id";
const RETRY_AFTER_MS_HEADER: HeaderName = HeaderName::from_static("retry-after-ms");
const RETRY_AFTER_HEADER: HeaderName = HeaderName::from_static("retry-after");
const SHOULD_RETRY_HEADER: HeaderName = HeaderName::from_static("x-should-retry");
const INITIAL_RETRY_DELAY: Duration = Duration::from_millis(500);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(8);

pub(crate) async fn execute_json_response<T>(
    http: &reqwest::Client,
    prepared: PreparedRequest,
) -> Result<ApiResponse<T>, Error>
where
    T: DeserializeOwned,
{
    let response = execute_response(http, prepared).await?;
    let request_id = response_request_id(&response);
    let bytes = response
        .bytes()
        .await
        .map_err(|source| Error::Transport { source })?;
    let data = serde_json::from_slice(&bytes).map_err(|source| Error::Json { source })?;
    Ok(ApiResponse::new(data, request_id))
}

pub(crate) async fn execute_stream_response(
    http: &reqwest::Client,
    prepared: PreparedRequest,
) -> Result<MessageStream, Error> {
    let response = execute_response(http, prepared).await?;
    Ok(MessageStream::from_response(response))
}

pub(crate) async fn execute_raw_response(
    http: &reqwest::Client,
    prepared: PreparedRequest,
) -> Result<reqwest::Response, Error> {
    execute_response(http, prepared).await
}

async fn execute_response(
    http: &reqwest::Client,
    prepared: PreparedRequest,
) -> Result<Response, Error> {
    let mut retries_remaining = prepared.max_retries().get();
    let mut retry_count = 0;
    let mut request = prepared.into_request();

    loop {
        let retry_request = request.try_clone();
        let response = match http.execute(request).await {
            Ok(response) => response,
            Err(source) => {
                let Some(next_request) = retry_request else {
                    return Err(Error::Transport { source });
                };

                if retries_remaining == 0 {
                    return Err(Error::Transport { source });
                }

                tracing::debug!(
                    retry_attempt = retry_count + 1,
                    retries_remaining = retries_remaining - 1,
                    "retrying request after transport failure"
                );
                tokio::time::sleep(retry_delay(None, retry_count)).await;
                request = next_request;
                retries_remaining -= 1;
                retry_count += 1;
                continue;
            }
        };

        let status = response.status();
        let request_id = response_request_id(&response);

        if !status.is_success() {
            if retries_remaining > 0 && should_retry_response(&response) {
                if let Some(next_request) = retry_request {
                    tracing::debug!(
                        retry_attempt = retry_count + 1,
                        retries_remaining = retries_remaining - 1,
                        status = status.as_u16(),
                        request_id = request_id.as_ref().map(anthropic_types::RequestId::as_str),
                        "retrying request after retryable API status"
                    );
                    let delay = retry_delay(Some(response.headers()), retry_count);
                    drop(response);
                    tokio::time::sleep(delay).await;
                    request = next_request;
                    retries_remaining -= 1;
                    retry_count += 1;
                    continue;
                }
            }

            return Err(api_error_from_response(response, status, request_id).await);
        }

        return Ok(response);
    }
}

async fn api_error_from_response(
    response: Response,
    status: StatusCode,
    request_id: Option<anthropic_types::RequestId>,
) -> Error {
    match response.bytes().await {
        Ok(bytes) => {
            let body = serde_json::from_slice(&bytes).ok();
            let raw_body = std::str::from_utf8(&bytes)
                .ok()
                .map(std::borrow::ToOwned::to_owned);
            crate::ApiError::from_response_parts(status, request_id, body, raw_body).into()
        }
        Err(source) => Error::Transport { source },
    }
}

pub(crate) fn response_request_id(
    response: &reqwest::Response,
) -> Option<anthropic_types::RequestId> {
    response
        .headers()
        .get(REQUEST_ID_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| anthropic_types::RequestId::try_new(value).ok())
}

fn should_retry_response(response: &Response) -> bool {
    if let Some(value) = response
        .headers()
        .get(SHOULD_RETRY_HEADER)
        .and_then(|value| value.to_str().ok())
    {
        if value == "true" {
            return true;
        }
        if value == "false" {
            return false;
        }
    }

    matches!(
        response.status(),
        StatusCode::REQUEST_TIMEOUT
            | StatusCode::CONFLICT
            | StatusCode::TOO_MANY_REQUESTS
            | StatusCode::INTERNAL_SERVER_ERROR
            | StatusCode::BAD_GATEWAY
            | StatusCode::SERVICE_UNAVAILABLE
            | StatusCode::GATEWAY_TIMEOUT
    ) || response.status().is_server_error()
}

fn retry_delay(headers: Option<&HeaderMap>, retry_count: u32) -> Duration {
    headers
        .and_then(parse_retry_after)
        .unwrap_or_else(|| default_retry_delay(retry_count))
}

fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    headers
        .get(RETRY_AFTER_MS_HEADER)
        .and_then(duration_from_millis_header)
        .or_else(|| {
            headers
                .get(RETRY_AFTER_HEADER)
                .and_then(duration_from_seconds_header)
        })
}

fn duration_from_millis_header(value: &reqwest::header::HeaderValue) -> Option<Duration> {
    let millis = value.to_str().ok()?.parse::<f64>().ok()?;
    duration_from_seconds(millis / 1000.0)
}

fn duration_from_seconds_header(value: &reqwest::header::HeaderValue) -> Option<Duration> {
    let seconds = value.to_str().ok()?.parse::<f64>().ok()?;
    duration_from_seconds(seconds)
}

fn duration_from_seconds(seconds: f64) -> Option<Duration> {
    if !seconds.is_finite() {
        return None;
    }
    if seconds <= 0.0 {
        return Some(Duration::ZERO);
    }
    if seconds > u64::MAX as f64 {
        return None;
    }

    Some(Duration::from_secs_f64(seconds))
}

fn default_retry_delay(retry_count: u32) -> Duration {
    let multiplier = 1_u32.checked_shl(retry_count.min(4)).unwrap_or(16);
    let delay = INITIAL_RETRY_DELAY.saturating_mul(multiplier);
    delay.min(MAX_RETRY_DELAY)
}
