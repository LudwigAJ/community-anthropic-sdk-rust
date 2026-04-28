//! Client configuration values and validating newtypes.
//!
//! Wraps the values that must be valid before the client can issue a
//! request: [`ApiKey`] (secret-aware, redacts in `Debug`), [`BaseUrl`] (parsed
//! and normalized so resource paths can be appended deterministically), and
//! [`MaxRetries`]. [`ClientConfig`] bundles these with the request timeout.
//!
//! Construction is fallible — invalid keys, malformed base URLs, or out-of-
//! range retry counts surface as [`ApiKeyError`] / [`BaseUrlError`] and are
//! mapped into [`crate::Error::Config`] when [`crate::Client::builder`]
//! finalizes a client.

use std::{fmt, time::Duration};

use reqwest::header::HeaderValue;
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error as ThisError;
use url::Url;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(600);
const DEFAULT_MAX_RETRIES: MaxRetries = MaxRetries::new(2);

/// Immutable client configuration.
#[derive(Clone)]
pub struct ClientConfig {
    api_key: ApiKey,
    base_url: BaseUrl,
    timeout: Duration,
    max_retries: MaxRetries,
}

impl ClientConfig {
    /// Creates client configuration from validated parts.
    pub fn new(api_key: ApiKey, base_url: BaseUrl) -> Self {
        Self {
            api_key,
            base_url,
            timeout: DEFAULT_TIMEOUT,
            max_retries: DEFAULT_MAX_RETRIES,
        }
    }

    pub(crate) fn with_request_defaults(
        api_key: ApiKey,
        base_url: BaseUrl,
        timeout: Duration,
        max_retries: MaxRetries,
    ) -> Self {
        Self {
            api_key,
            base_url,
            timeout,
            max_retries,
        }
    }

    /// Returns true when an API key has been configured without exposing the key.
    pub fn api_key_is_configured(&self) -> bool {
        !self.api_key.expose_secret().is_empty()
    }

    pub(crate) fn api_key_secret(&self) -> &str {
        self.api_key.expose_secret()
    }

    /// Returns the base API URL.
    pub fn base_url(&self) -> &Url {
        self.base_url.as_url()
    }

    /// Returns the default timeout for each non-streaming request attempt.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Returns the default maximum number of retries.
    pub fn max_retries(&self) -> MaxRetries {
        self.max_retries
    }
}

impl fmt::Debug for ClientConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientConfig")
            .field("api_key", &"[redacted]")
            .field("base_url", &self.base_url)
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .finish()
    }
}

/// Maximum number of request retries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaxRetries(u32);

impl MaxRetries {
    /// Creates a retry count.
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the raw retry count.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl From<u32> for MaxRetries {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

impl From<MaxRetries> for u32 {
    fn from(value: MaxRetries) -> Self {
        value.get()
    }
}

/// Secret-aware Anthropic API key.
#[derive(Clone)]
pub struct ApiKey(SecretString);

impl ApiKey {
    /// Creates an API key from a non-blank string.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ApiKeyError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ApiKeyError::Empty);
        }
        if HeaderValue::from_str(&value).is_err() {
            return Err(ApiKeyError::InvalidHeaderValue);
        }

        Ok(Self(SecretString::new(value.into())))
    }

    pub(crate) fn expose_secret(&self) -> &str {
        self.0.expose_secret()
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[redacted]")
    }
}

/// Errors produced while constructing [`ApiKey`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, ThisError)]
pub enum ApiKeyError {
    /// API keys must not be blank.
    #[error("API key must not be blank")]
    Empty,
    /// API keys must be valid HTTP header values.
    #[error("API key must be a valid HTTP header value")]
    InvalidHeaderValue,
}

/// Base URL for Anthropic-compatible API requests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseUrl(Url);

impl BaseUrl {
    /// Parses a base URL.
    pub fn parse(value: &str) -> Result<Self, BaseUrlError> {
        let url = Url::parse(value).map_err(BaseUrlError::Parse)?;
        Ok(Self(url))
    }

    /// Returns the parsed URL.
    pub fn as_url(&self) -> &Url {
        &self.0
    }

    /// Returns the normalized URL string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Errors produced while constructing [`BaseUrl`].
#[derive(Debug, Clone, PartialEq, Eq, ThisError)]
pub enum BaseUrlError {
    /// URL parsing failed.
    #[error("invalid base URL")]
    Parse(#[source] url::ParseError),
}

pub(crate) fn default_base_url() -> Result<BaseUrl, BaseUrlError> {
    BaseUrl::parse(DEFAULT_BASE_URL)
}

pub(crate) fn default_timeout() -> Duration {
    DEFAULT_TIMEOUT
}

pub(crate) fn default_max_retries() -> MaxRetries {
    DEFAULT_MAX_RETRIES
}
