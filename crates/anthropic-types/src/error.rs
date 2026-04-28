//! Wire-level Anthropic API error payloads.
//!
//! These types model the JSON Anthropic returns on a failed request.
//! [`ApiErrorBody`] is the outer envelope and [`ApiErrorDetail`] is the
//! inner `error` object; [`ApiErrorType`] is the typed `type` discriminator
//! with a forward-compatible `Other(String)` variant.
//!
//! `anthropic-client` decodes these into the public `ApiError` /
//! `ApiErrorKind` shape exposed on `crate::Error`. Applications usually
//! match on those higher-level types rather than these wire models, but
//! the raw body is still preserved on `ApiError::body` for diagnostics.

use std::{borrow::Cow, fmt};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Top-level API error response body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiErrorBody {
    /// Error details returned by the API.
    pub error: ApiErrorDetail,
}

/// Structured API error details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiErrorDetail {
    /// Machine-readable error category.
    #[serde(rename = "type")]
    pub error_type: ApiErrorType,
    /// Human-readable error message.
    pub message: String,
}

/// Known API error categories.
#[derive(Clone, PartialEq, Eq)]
pub enum ApiErrorType {
    /// The request was malformed or semantically invalid.
    InvalidRequest,
    /// The request did not contain valid authentication.
    Authentication,
    /// The credentials are valid but do not grant access.
    Permission,
    /// The requested resource was not found.
    NotFound,
    /// The request exceeded a rate limit.
    RateLimit,
    /// The API encountered an internal error.
    Api,
    /// The API is overloaded.
    Overloaded,
    /// The API gateway timed out.
    Timeout,
    /// The account or workspace has a billing-related problem.
    Billing,
    /// An Anthropic-compatible gateway returned an unrecognized error type.
    Unknown(String),
}

impl ApiErrorType {
    /// Returns the wire-format error type string.
    pub fn as_str(&self) -> &str {
        match self {
            Self::InvalidRequest => "invalid_request_error",
            Self::Authentication => "authentication_error",
            Self::Permission => "permission_error",
            Self::NotFound => "not_found_error",
            Self::RateLimit => "rate_limit_error",
            Self::Api => "api_error",
            Self::Overloaded => "overloaded_error",
            Self::Timeout => "timeout_error",
            Self::Billing => "billing_error",
            Self::Unknown(value) => value.as_str(),
        }
    }

    /// Converts a wire-format error type string into a typed value.
    pub fn from_wire(value: impl Into<String>) -> Self {
        let value = value.into();
        match value.as_str() {
            "invalid_request_error" => Self::InvalidRequest,
            "authentication_error" => Self::Authentication,
            "permission_error" => Self::Permission,
            "not_found_error" => Self::NotFound,
            "rate_limit_error" => Self::RateLimit,
            "api_error" => Self::Api,
            "overloaded_error" => Self::Overloaded,
            "timeout_error" => Self::Timeout,
            "billing_error" => Self::Billing,
            _ => Self::Unknown(value),
        }
    }
}

impl fmt::Debug for ApiErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest => f.write_str("InvalidRequest"),
            Self::Authentication => f.write_str("Authentication"),
            Self::Permission => f.write_str("Permission"),
            Self::NotFound => f.write_str("NotFound"),
            Self::RateLimit => f.write_str("RateLimit"),
            Self::Api => f.write_str("Api"),
            Self::Overloaded => f.write_str("Overloaded"),
            Self::Timeout => f.write_str("Timeout"),
            Self::Billing => f.write_str("Billing"),
            Self::Unknown(value) => f.debug_tuple("Unknown").field(value).finish(),
        }
    }
}

impl Serialize for ApiErrorType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ApiErrorType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Cow::<str>::deserialize(deserializer)?;
        Ok(Self::from_wire(value.into_owned()))
    }
}
