//! Internal HTTP request construction.
//!
//! Not part of the public API. Provides a small builder used by the
//! resource modules to assemble path, query, body, headers, timeout, and
//! per-request retry overrides before handing the request to
//! [`crate::transport`]. Centralizing this logic keeps service modules
//! focused on URL shape and serde, and ensures that header redaction,
//! query encoding, and timeout merging happen in exactly one place.

use std::{fmt, time::Duration};

use anthropic_types::{
    BatchCreateParams, ListParams, MessageBatchId, MessageCountTokensParams, MessageCreateParams,
};
use reqwest::{
    Request,
    header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
};
use url::Url;

use crate::{ClientConfig, Error, MaxRetries, RequestOptions};

const MESSAGES_CREATE_PATH: &str = "/v1/messages";
const MESSAGES_COUNT_TOKENS_PATH: &str = "/v1/messages/count_tokens";
const MESSAGES_BATCHES_PATH: &str = "/v1/messages/batches";
const MODELS_PATH: &str = "/v1/models";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Fully prepared request plus transport settings needed by retry code later.
pub(crate) struct PreparedRequest {
    request: Request,
    timeout: Duration,
    max_retries: MaxRetries,
}

impl PreparedRequest {
    #[cfg(test)]
    pub(crate) fn request(&self) -> &Request {
        &self.request
    }

    pub(crate) fn into_request(self) -> Request {
        self.request
    }

    #[cfg(test)]
    pub(crate) fn timeout(&self) -> Duration {
        self.timeout
    }

    pub(crate) fn max_retries(&self) -> MaxRetries {
        self.max_retries
    }
}

impl fmt::Debug for PreparedRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedRequest")
            .field("method", self.request.method())
            .field("url", self.request.url())
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .field("header_count", &self.request.headers().len())
            .finish()
    }
}

pub(crate) fn build_messages_create(
    http: &reqwest::Client,
    config: &ClientConfig,
    params: &MessageCreateParams,
    options: &RequestOptions,
) -> Result<PreparedRequest, Error> {
    build_post_json(http, config, MESSAGES_CREATE_PATH, params, options)
}

pub(crate) fn build_messages_create_stream(
    http: &reqwest::Client,
    config: &ClientConfig,
    params: &MessageCreateParams,
    options: &RequestOptions,
) -> Result<PreparedRequest, Error> {
    let mut params = params.clone();
    params.stream = Some(true);
    build_messages_create(http, config, &params, options)
}

pub(crate) fn build_models_list(
    http: &reqwest::Client,
    config: &ClientConfig,
    params: &ListParams,
    options: &RequestOptions,
) -> Result<PreparedRequest, Error> {
    let timeout = options.effective_timeout(config.timeout());
    let max_retries = options.effective_max_retries(config.max_retries());
    let mut url = join_base_url(config.base_url(), MODELS_PATH)?;
    apply_list_query(&mut url, params);
    let headers = build_headers(config, options)?;
    let request = http
        .get(url)
        .headers(headers)
        .timeout(timeout)
        .build()
        .map_err(|source| Error::Transport { source })?;

    Ok(PreparedRequest {
        request,
        timeout,
        max_retries,
    })
}

pub(crate) fn build_models_retrieve(
    http: &reqwest::Client,
    config: &ClientConfig,
    model_id: &str,
    options: &RequestOptions,
) -> Result<PreparedRequest, Error> {
    let path = format!("{MODELS_PATH}/{}", encode_path_segment(model_id));
    build_get(http, config, &path, options)
}

fn apply_list_query(url: &mut Url, params: &ListParams) {
    if params.is_empty() {
        return;
    }
    let mut query = url.query_pairs_mut();
    if let Some(limit) = params.limit {
        query.append_pair("limit", &limit.to_string());
    }
    if let Some(before_id) = &params.before_id {
        query.append_pair("before_id", before_id);
    }
    if let Some(after_id) = &params.after_id {
        query.append_pair("after_id", after_id);
    }
}

pub(crate) fn build_messages_count_tokens(
    http: &reqwest::Client,
    config: &ClientConfig,
    params: &MessageCountTokensParams,
    options: &RequestOptions,
) -> Result<PreparedRequest, Error> {
    build_post_json(http, config, MESSAGES_COUNT_TOKENS_PATH, params, options)
}

pub(crate) fn build_messages_batches_create(
    http: &reqwest::Client,
    config: &ClientConfig,
    params: &BatchCreateParams,
    options: &RequestOptions,
) -> Result<PreparedRequest, Error> {
    build_post_json(http, config, MESSAGES_BATCHES_PATH, params, options)
}

pub(crate) fn build_messages_batches_retrieve(
    http: &reqwest::Client,
    config: &ClientConfig,
    batch_id: &str,
    options: &RequestOptions,
) -> Result<PreparedRequest, Error> {
    let batch_id = MessageBatchId::try_new(batch_id)
        .map_err(|source| Error::InvalidMessageBatchId { source })?;
    let path = format!(
        "{MESSAGES_BATCHES_PATH}/{}",
        encode_path_segment(batch_id.as_str())
    );
    build_get(http, config, &path, options)
}

pub(crate) fn build_messages_batches_list(
    http: &reqwest::Client,
    config: &ClientConfig,
    params: &ListParams,
    options: &RequestOptions,
) -> Result<PreparedRequest, Error> {
    let timeout = options.effective_timeout(config.timeout());
    let max_retries = options.effective_max_retries(config.max_retries());
    let mut url = join_base_url(config.base_url(), MESSAGES_BATCHES_PATH)?;
    apply_list_query(&mut url, params);
    let headers = build_headers(config, options)?;
    let request = http
        .get(url)
        .headers(headers)
        .timeout(timeout)
        .build()
        .map_err(|source| Error::Transport { source })?;

    Ok(PreparedRequest {
        request,
        timeout,
        max_retries,
    })
}

pub(crate) fn build_messages_batches_cancel(
    http: &reqwest::Client,
    config: &ClientConfig,
    batch_id: &str,
    options: &RequestOptions,
) -> Result<PreparedRequest, Error> {
    let batch_id = MessageBatchId::try_new(batch_id)
        .map_err(|source| Error::InvalidMessageBatchId { source })?;
    let timeout = options.effective_timeout(config.timeout());
    let max_retries = options.effective_max_retries(config.max_retries());
    let path = format!(
        "{MESSAGES_BATCHES_PATH}/{}/cancel",
        encode_path_segment(batch_id.as_str())
    );
    let url = join_base_url(config.base_url(), &path)?;
    let headers = build_headers(config, options)?;
    let request = http
        .post(url)
        .headers(headers)
        .timeout(timeout)
        .build()
        .map_err(|source| Error::Transport { source })?;

    Ok(PreparedRequest {
        request,
        timeout,
        max_retries,
    })
}

pub(crate) fn build_messages_batches_delete(
    http: &reqwest::Client,
    config: &ClientConfig,
    batch_id: &str,
    options: &RequestOptions,
) -> Result<PreparedRequest, Error> {
    let batch_id = MessageBatchId::try_new(batch_id)
        .map_err(|source| Error::InvalidMessageBatchId { source })?;
    let timeout = options.effective_timeout(config.timeout());
    let max_retries = options.effective_max_retries(config.max_retries());
    let path = format!(
        "{MESSAGES_BATCHES_PATH}/{}",
        encode_path_segment(batch_id.as_str())
    );
    let url = join_base_url(config.base_url(), &path)?;
    let headers = build_headers(config, options)?;
    let request = http
        .delete(url)
        .headers(headers)
        .timeout(timeout)
        .build()
        .map_err(|source| Error::Transport { source })?;

    Ok(PreparedRequest {
        request,
        timeout,
        max_retries,
    })
}

pub(crate) fn build_messages_batches_results(
    http: &reqwest::Client,
    config: &ClientConfig,
    results_url: &str,
    options: &RequestOptions,
) -> Result<PreparedRequest, Error> {
    let timeout = options.effective_timeout(config.timeout());
    let max_retries = options.effective_max_retries(config.max_retries());
    let url = Url::parse(results_url).map_err(|source| Error::InvalidBaseUrl {
        source: crate::BaseUrlError::Parse(source),
    })?;
    let mut headers = build_headers(config, options)?;
    headers.insert(ACCEPT, HeaderValue::from_static("application/x-ndjson"));
    let request = http
        .get(url)
        .headers(headers)
        .timeout(timeout)
        .build()
        .map_err(|source| Error::Transport { source })?;

    Ok(PreparedRequest {
        request,
        timeout,
        max_retries,
    })
}

fn build_get(
    http: &reqwest::Client,
    config: &ClientConfig,
    path: &str,
    options: &RequestOptions,
) -> Result<PreparedRequest, Error> {
    let timeout = options.effective_timeout(config.timeout());
    let max_retries = options.effective_max_retries(config.max_retries());
    let url = join_base_url(config.base_url(), path)?;
    let headers = build_headers(config, options)?;
    let request = http
        .get(url)
        .headers(headers)
        .timeout(timeout)
        .build()
        .map_err(|source| Error::Transport { source })?;

    Ok(PreparedRequest {
        request,
        timeout,
        max_retries,
    })
}

fn encode_path_segment(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn build_post_json<T: serde::Serialize>(
    http: &reqwest::Client,
    config: &ClientConfig,
    path: &str,
    body: &T,
    options: &RequestOptions,
) -> Result<PreparedRequest, Error> {
    let timeout = options.effective_timeout(config.timeout());
    let max_retries = options.effective_max_retries(config.max_retries());
    let url = join_base_url(config.base_url(), path)?;
    let headers = build_headers(config, options)?;
    let body = serde_json::to_vec(body).map_err(|source| Error::Json { source })?;
    let request = http
        .post(url)
        .headers(headers)
        .timeout(timeout)
        .body(body)
        .build()
        .map_err(|source| Error::Transport { source })?;

    Ok(PreparedRequest {
        request,
        timeout,
        max_retries,
    })
}

fn build_headers(config: &ClientConfig, options: &RequestOptions) -> Result<HeaderMap, Error> {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-api-key"),
        HeaderValue::from_str(config.api_key_secret()).map_err(|_| Error::InvalidApiKey {
            source: crate::ApiKeyError::InvalidHeaderValue,
        })?,
    );
    headers.insert(
        HeaderName::from_static("anthropic-version"),
        HeaderValue::from_static(ANTHROPIC_VERSION),
    );
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    for name in options.replaced_header_names() {
        headers.remove(name);
    }
    for (name, value) in options.headers() {
        headers.append(name, value.clone());
    }

    Ok(headers)
}

fn join_base_url(base_url: &Url, path: &str) -> Result<Url, Error> {
    let base = base_url.as_str().trim_end_matches('/');
    let path = path.trim_start_matches('/');
    Url::parse(&format!("{base}/{path}")).map_err(|source| Error::InvalidBaseUrl {
        source: crate::BaseUrlError::Parse(source),
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use reqwest::{
        Method,
        header::{ACCEPT, CONTENT_TYPE},
    };
    use serde_json::Value;

    use super::*;
    use crate::{Client, RequestOptions};
    use anthropic_types::{
        BatchCreateParams, BatchCreateRequest, MessageCountTokensParams, MessageParam, Model,
    };

    fn client() -> Result<Client, Error> {
        Client::builder().api_key("sk-ant-test-safe").build()
    }

    fn client_with_base_url(base_url: &str) -> Result<Client, Error> {
        Client::builder()
            .api_key("sk-ant-test-safe")
            .base_url(base_url)?
            .build()
    }

    fn params(model: impl Into<Model>) -> Result<MessageCreateParams, Box<dyn std::error::Error>> {
        Ok(MessageCreateParams::builder()
            .model(model)
            .max_tokens(128)
            .message(MessageParam::user("Hello"))
            .build()?)
    }

    fn prepare(
        client: &Client,
        params: &MessageCreateParams,
        options: &RequestOptions,
    ) -> Result<PreparedRequest, Error> {
        build_messages_create(client.http_client(), client.config(), params, options)
    }

    fn prepare_stream(
        client: &Client,
        params: &MessageCreateParams,
        options: &RequestOptions,
    ) -> Result<PreparedRequest, Error> {
        build_messages_create_stream(client.http_client(), client.config(), params, options)
    }

    fn body_json(request: &Request) -> Result<Value, Box<dyn std::error::Error>> {
        let bytes = request
            .body()
            .and_then(reqwest::Body::as_bytes)
            .ok_or_else(|| std::io::Error::other("request body missing"))?;
        Ok(serde_json::from_slice(bytes)?)
    }

    #[test]
    fn builds_default_anthropic_messages_request() -> Result<(), Box<dyn std::error::Error>> {
        let client = client()?;
        let params = params(Model::ClaudeSonnet4_5)?;
        let prepared = prepare(&client, &params, &RequestOptions::new())?;
        let request = prepared.request();

        assert_eq!(request.method(), Method::POST);
        assert_eq!(
            request.url().as_str(),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(prepared.timeout(), Duration::from_secs(600));
        assert_eq!(prepared.max_retries().get(), 2);
        assert_eq!(request.timeout(), Some(&Duration::from_secs(600)));
        assert_eq!(
            request
                .headers()
                .get("x-api-key")
                .and_then(|value| value.to_str().ok()),
            Some("sk-ant-test-safe")
        );
        assert_eq!(
            request
                .headers()
                .get("anthropic-version")
                .and_then(|value| value.to_str().ok()),
            Some(ANTHROPIC_VERSION)
        );
        assert_eq!(
            request
                .headers()
                .get(ACCEPT)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        assert_eq!(
            request
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        Ok(())
    }

    #[test]
    fn joins_base_url_paths() -> Result<(), Box<dyn std::error::Error>> {
        let client = client_with_base_url("https://api.provider.test/anthropic")?;
        let params = params(Model::ClaudeSonnet4_5)?;
        let prepared = prepare(&client, &params, &RequestOptions::new())?;

        assert_eq!(
            prepared.request().url().as_str(),
            "https://api.provider.test/anthropic/v1/messages"
        );
        Ok(())
    }

    #[test]
    fn serializes_provider_specific_model_unchanged() -> Result<(), Box<dyn std::error::Error>> {
        let client = client_with_base_url("https://api.provider.test/anthropic")?;
        let params = params("MiniMax-M2.7")?;
        let prepared = prepare(&client, &params, &RequestOptions::new())?;
        let body = body_json(prepared.request())?;

        assert_eq!(body["model"], "MiniMax-M2.7");
        Ok(())
    }

    #[test]
    fn request_options_override_timeout_and_retries() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::builder()
            .api_key("sk-ant-test-safe")
            .timeout(Duration::from_secs(90))
            .max_retries(4)
            .build()?;
        let params = params(Model::ClaudeSonnet4_5)?;
        let options = RequestOptions::builder()
            .timeout(Duration::from_secs(30))
            .max_retries(0)
            .build()?;
        let prepared = prepare(&client, &params, &options)?;

        assert_eq!(prepared.timeout(), Duration::from_secs(30));
        assert_eq!(prepared.max_retries().get(), 0);
        assert_eq!(prepared.request().timeout(), Some(&Duration::from_secs(30)));
        Ok(())
    }

    #[test]
    fn streaming_request_forces_stream_true() -> Result<(), Box<dyn std::error::Error>> {
        let client = client()?;
        let mut params = params(Model::ClaudeSonnet4_5)?;
        params.stream = Some(false);

        let prepared = prepare_stream(&client, &params, &RequestOptions::new())?;
        let body = body_json(prepared.request())?;

        assert_eq!(body["stream"], true);
        assert_eq!(params.stream, Some(false));
        Ok(())
    }

    #[test]
    fn additive_headers_preserve_multiple_values() -> Result<(), Box<dyn std::error::Error>> {
        let client = client()?;
        let params = params(Model::ClaudeSonnet4_5)?;
        let options = RequestOptions::builder()
            .header("anthropic-beta", "first-beta")
            .header("anthropic-beta", "second-beta")
            .build()?;
        let prepared = prepare(&client, &params, &options)?;
        let values = prepared
            .request()
            .headers()
            .get_all("anthropic-beta")
            .iter()
            .map(HeaderValue::to_str)
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(values, vec!["first-beta", "second-beta"]);
        Ok(())
    }

    #[test]
    fn set_header_replaces_default_and_prior_values() -> Result<(), Box<dyn std::error::Error>> {
        let client = client()?;
        let params = params(Model::ClaudeSonnet4_5)?;
        let options = RequestOptions::builder()
            .header("accept", "text/event-stream")
            .set_header("accept", "application/custom-json")
            .build()?;
        let prepared = prepare(&client, &params, &options)?;
        let values = prepared
            .request()
            .headers()
            .get_all(ACCEPT)
            .iter()
            .map(HeaderValue::to_str)
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(values, vec!["application/custom-json"]);
        Ok(())
    }

    #[test]
    fn typed_set_header_replaces_default_values() -> Result<(), Box<dyn std::error::Error>> {
        let client = client()?;
        let params = params(Model::ClaudeSonnet4_5)?;
        let options = RequestOptions::builder()
            .set_header_value(
                CONTENT_TYPE,
                HeaderValue::from_static("application/custom-json"),
            )
            .build()?;
        let prepared = prepare(&client, &params, &options)?;
        let values = prepared
            .request()
            .headers()
            .get_all(CONTENT_TYPE)
            .iter()
            .map(HeaderValue::to_str)
            .collect::<Result<Vec<_>, _>>()?;

        assert_eq!(values, vec!["application/custom-json"]);
        Ok(())
    }

    #[test]
    fn prepared_request_debug_redacts_sensitive_values() -> Result<(), Box<dyn std::error::Error>> {
        let client = client()?;
        let params = params(Model::ClaudeSonnet4_5)?;
        let options = RequestOptions::builder()
            .header("anthropic-beta", "secret-beta")
            .build()?;
        let prepared = prepare(&client, &params, &options)?;
        let rendered = format!("{prepared:?}");

        assert!(rendered.contains("header_count"));
        assert!(!rendered.contains("sk-ant-test-safe"));
        assert!(!rendered.contains("secret-beta"));
        assert!(!rendered.contains("Hello"));
        Ok(())
    }

    fn count_tokens_params(
        model: impl Into<Model>,
    ) -> Result<MessageCountTokensParams, Box<dyn std::error::Error>> {
        Ok(MessageCountTokensParams::builder()
            .model(model)
            .message(MessageParam::user("Hello"))
            .build()?)
    }

    #[test]
    fn builds_count_tokens_request() -> Result<(), Box<dyn std::error::Error>> {
        let client = client()?;
        let params = count_tokens_params(Model::ClaudeSonnet4_5)?;
        let prepared = build_messages_count_tokens(
            client.http_client(),
            client.config(),
            &params,
            &RequestOptions::new(),
        )?;
        let request = prepared.request();

        assert_eq!(request.method(), Method::POST);
        assert_eq!(
            request.url().as_str(),
            "https://api.anthropic.com/v1/messages/count_tokens"
        );
        assert_eq!(
            request
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );
        assert_eq!(
            request
                .headers()
                .get("anthropic-version")
                .and_then(|value| value.to_str().ok()),
            Some(ANTHROPIC_VERSION)
        );

        let body = body_json(request)?;
        assert_eq!(body["model"], "claude-sonnet-4-5");
        assert_eq!(body["messages"][0]["role"], "user");
        assert!(body.get("max_tokens").is_none());
        assert!(body.get("stream").is_none());
        Ok(())
    }

    fn batch_create_params() -> Result<BatchCreateParams, Box<dyn std::error::Error>> {
        let request = BatchCreateRequest::new("req-1", params(Model::ClaudeSonnet4_5)?)?;
        Ok(BatchCreateParams::builder().request(request).build()?)
    }

    #[test]
    fn builds_message_batch_delete_request() -> Result<(), Box<dyn std::error::Error>> {
        let client = client()?;
        let prepared = build_messages_batches_delete(
            client.http_client(),
            client.config(),
            "msgbatch_01",
            &RequestOptions::new(),
        )?;
        let request = prepared.request();

        assert_eq!(request.method(), Method::DELETE);
        assert_eq!(
            request.url().as_str(),
            "https://api.anthropic.com/v1/messages/batches/msgbatch_01"
        );
        assert_eq!(
            request
                .headers()
                .get("anthropic-version")
                .and_then(|value| value.to_str().ok()),
            Some(ANTHROPIC_VERSION)
        );
        Ok(())
    }

    #[test]
    fn batch_lifecycle_requests_reject_blank_batch_ids() -> Result<(), Box<dyn std::error::Error>> {
        let client = client()?;

        for result in [
            build_messages_batches_retrieve(
                client.http_client(),
                client.config(),
                " ",
                &RequestOptions::new(),
            ),
            build_messages_batches_cancel(
                client.http_client(),
                client.config(),
                "",
                &RequestOptions::new(),
            ),
            build_messages_batches_delete(
                client.http_client(),
                client.config(),
                "\t",
                &RequestOptions::new(),
            ),
        ] {
            assert!(matches!(result, Err(Error::InvalidMessageBatchId { .. })));
        }

        Ok(())
    }

    #[test]
    fn batch_lifecycle_request_options_apply() -> Result<(), Box<dyn std::error::Error>> {
        let client = Client::builder()
            .api_key("sk-ant-test-safe")
            .timeout(Duration::from_secs(90))
            .max_retries(4)
            .build()?;
        let options = RequestOptions::builder()
            .timeout(Duration::from_secs(15))
            .max_retries(0)
            .set_header("x-request-source", "batch-test")
            .build()?;

        let create = build_messages_batches_create(
            client.http_client(),
            client.config(),
            &batch_create_params()?,
            &options,
        )?;
        let retrieve = build_messages_batches_retrieve(
            client.http_client(),
            client.config(),
            "msgbatch_01",
            &options,
        )?;
        let cancel = build_messages_batches_cancel(
            client.http_client(),
            client.config(),
            "msgbatch_01",
            &options,
        )?;
        let delete = build_messages_batches_delete(
            client.http_client(),
            client.config(),
            "msgbatch_01",
            &options,
        )?;

        for prepared in [create, retrieve, cancel, delete] {
            assert_eq!(prepared.timeout(), Duration::from_secs(15));
            assert_eq!(prepared.max_retries().get(), 0);
            assert_eq!(prepared.request().timeout(), Some(&Duration::from_secs(15)));
            assert_eq!(
                prepared
                    .request()
                    .headers()
                    .get("x-request-source")
                    .and_then(|value| value.to_str().ok()),
                Some("batch-test")
            );
        }

        Ok(())
    }
}
