//! Public error types for the client.
//!
//! [`Error`] is the single error returned by every public method. It groups
//! configuration, transport, timeout, JSON, streaming, batch-id validation,
//! and API-side failures so callers can pattern-match on the failure mode.
//!
//! [`ApiError`] preserves HTTP status, the response [`anthropic_types::RequestId`],
//! a typed [`ApiErrorKind`] (rate-limit, authentication, overloaded, etc.),
//! the API message, and the raw `ApiErrorBody` for downstream tooling.
//! Forward-compatible API error types deserialize into
//! [`ApiErrorKind::Unknown`] without losing their original tag.

use std::fmt;

use anthropic_types::{
    ApiErrorBody, ApiErrorType, BatchProcessingStatus, MessageBatchIdError, RequestId,
    StructuredOutputError,
};
use reqwest::StatusCode;
use thiserror::Error as ThisError;

use crate::{ApiKeyError, BaseUrlError};

/// Error type returned by the SDK client.
#[derive(Debug, ThisError)]
pub enum Error {
    /// No API key was configured.
    #[error("missing API key; set ANTHROPIC_API_KEY or configure ClientBuilder::api_key")]
    MissingApiKey,

    /// The configured API key was invalid.
    #[error("invalid API key")]
    InvalidApiKey {
        /// API key validation source error.
        #[source]
        source: ApiKeyError,
    },

    /// The configured base URL was invalid.
    #[error("invalid base URL")]
    InvalidBaseUrl {
        /// URL parser source error.
        #[source]
        source: BaseUrlError,
    },

    /// The HTTP transport failed before a valid API response was produced.
    #[error("transport error")]
    Transport {
        /// HTTP client source error.
        #[source]
        source: reqwest::Error,
    },

    /// JSON serialization or deserialization failed.
    #[error("JSON error")]
    Json {
        /// JSON source error.
        #[source]
        source: serde_json::Error,
    },

    /// The API returned a non-success status.
    #[error("API returned HTTP status {0}")]
    Api(Box<ApiError>),

    /// Streaming failed.
    #[error("stream error: {message}")]
    Stream {
        /// Stream error message.
        message: String,
    },

    /// Structured-output parsing failed.
    #[error("structured output parse error")]
    StructuredOutput {
        /// Structured-output parsing source error.
        #[source]
        source: StructuredOutputError,
    },

    /// A planned SDK feature has not been implemented yet.
    #[error("{feature} is not implemented yet")]
    NotImplemented {
        /// Feature name.
        feature: &'static str,
    },

    /// A message batch identifier was invalid before sending a request.
    #[error("invalid message batch ID")]
    InvalidMessageBatchId {
        /// Message batch ID validation source error.
        #[source]
        source: MessageBatchIdError,
    },

    /// A message batch was not yet ready for results download.
    #[error(
        "message batch `{batch_id}` has no results_url yet (processing_status = {processing_status})"
    )]
    BatchResultsUnavailable {
        /// Identifier of the batch the caller asked for.
        batch_id: String,
        /// Processing status reported when the request was made.
        processing_status: BatchProcessingStatus,
    },
}

impl Error {
    /// Returns the API request ID when this error carries one.
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Api(error) => error.request_id(),
            _ => None,
        }
    }
}

/// Structured error details returned for a non-success API response.
#[derive(Clone)]
pub struct ApiError {
    /// HTTP status code.
    pub status: StatusCode,
    /// Request identifier returned by the API, when present.
    pub request_id: Option<RequestId>,
    /// Stable SDK-level API error category.
    pub kind: ApiErrorKind,
    /// Human-readable API error message, or a status-based fallback when the
    /// body could not be parsed.
    pub message: String,
    /// Structured API error body, when it could be parsed.
    pub body: Option<ApiErrorBody>,
    /// Raw error response body, when it was valid UTF-8.
    pub raw_body: Option<String>,
}

impl ApiError {
    /// Builds structured API error details from an HTTP status and response body.
    pub(crate) fn from_response_parts(
        status: StatusCode,
        request_id: Option<RequestId>,
        body: Option<ApiErrorBody>,
        raw_body: Option<String>,
    ) -> Self {
        let kind = api_error_kind(status, body.as_ref());
        let message = api_error_message(status, body.as_ref(), raw_body.as_deref());

        Self {
            status,
            request_id,
            kind,
            message,
            body,
            raw_body,
        }
    }

    /// Returns the request ID as a string slice.
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_ref().map(RequestId::as_str)
    }
}

impl fmt::Debug for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiError")
            .field("status", &self.status)
            .field("request_id", &self.request_id)
            .field("kind", &self.kind)
            .field("message", &"[redacted]")
            .field("body", &self.body.as_ref().map(|_| "[redacted]"))
            .field("raw_body", &self.raw_body.as_ref().map(|_| "[redacted]"))
            .finish()
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.status, self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<ApiError> for Error {
    fn from(error: ApiError) -> Self {
        Self::Api(Box::new(error))
    }
}

/// Stable SDK-level API error categories callers can match on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiErrorKind {
    /// The request was malformed or semantically invalid.
    InvalidRequest,
    /// The request did not contain valid authentication.
    Authentication,
    /// The credentials are valid but do not grant access.
    Permission,
    /// The requested resource was not found.
    NotFound,
    /// The request conflicted with the current state of the resource.
    Conflict,
    /// The request was well-formed JSON but semantically unprocessable.
    UnprocessableEntity,
    /// The request exceeded a rate limit.
    RateLimit,
    /// The API encountered an internal server error.
    InternalServer,
    /// The API or compatible gateway is overloaded.
    Overloaded,
    /// The SDK does not recognize this API error type or status.
    Unknown(String),
}

fn api_error_kind(status: StatusCode, body: Option<&ApiErrorBody>) -> ApiErrorKind {
    if let Some(error_type) = body.map(|body| &body.error.error_type) {
        return match error_type {
            ApiErrorType::InvalidRequest => ApiErrorKind::InvalidRequest,
            ApiErrorType::Authentication => ApiErrorKind::Authentication,
            ApiErrorType::Permission => ApiErrorKind::Permission,
            ApiErrorType::NotFound => ApiErrorKind::NotFound,
            ApiErrorType::RateLimit => ApiErrorKind::RateLimit,
            ApiErrorType::Api => ApiErrorKind::InternalServer,
            ApiErrorType::Overloaded => ApiErrorKind::Overloaded,
            ApiErrorType::Unknown(value) => ApiErrorKind::Unknown(value.clone()),
            other => ApiErrorKind::Unknown(other.as_str().to_owned()),
        };
    }

    match status {
        StatusCode::BAD_REQUEST => ApiErrorKind::InvalidRequest,
        StatusCode::UNAUTHORIZED => ApiErrorKind::Authentication,
        StatusCode::FORBIDDEN => ApiErrorKind::Permission,
        StatusCode::NOT_FOUND => ApiErrorKind::NotFound,
        StatusCode::CONFLICT => ApiErrorKind::Conflict,
        StatusCode::UNPROCESSABLE_ENTITY => ApiErrorKind::UnprocessableEntity,
        StatusCode::TOO_MANY_REQUESTS => ApiErrorKind::RateLimit,
        status if status.is_server_error() => ApiErrorKind::InternalServer,
        status => ApiErrorKind::Unknown(format!("status_{}", status.as_u16())),
    }
}

fn api_error_message(
    status: StatusCode,
    body: Option<&ApiErrorBody>,
    raw_body: Option<&str>,
) -> String {
    if let Some(message) = body.map(|body| body.error.message.as_str()) {
        return message.to_owned();
    }

    match raw_body {
        Some("") => format!("HTTP status {} with empty error body", status.as_u16()),
        Some(_) => format!(
            "HTTP status {} with unparseable error body",
            status.as_u16()
        ),
        None => format!("HTTP status {} with no error body", status.as_u16()),
    }
}
