//! Client construction and shared state.
//!
//! [`Client`] is the entry point for every API call in this crate. It wraps a
//! configured [`reqwest::Client`], a validated [`crate::ClientConfig`], and a
//! retry policy, and hands resource handles ([`crate::Messages`],
//! [`crate::Models`], [`crate::Batches`]) to callers.
//!
//! Use [`Client::from_env`] for the common path (reads `ANTHROPIC_API_KEY`
//! and optional `ANTHROPIC_BASE_URL`). Use [`Client::builder`] /
//! [`ClientBuilder`] when an application owns transport configuration, needs
//! to point at an Anthropic-compatible gateway, or sets explicit timeouts and
//! retry counts.
//!
//! `Client` is cheap to clone (`Arc` internally) and is `Send + Sync`. Drop
//! the future returned by any service method to cancel the request.

use std::time::Duration;

use crate::{ApiKey, BaseUrl, ClientConfig, Error, MaxRetries, Messages, Models};

const API_KEY_ENV: &str = "ANTHROPIC_API_KEY";
const BASE_URL_ENV: &str = "ANTHROPIC_BASE_URL";

/// Anthropic API client.
#[derive(Clone, Debug)]
pub struct Client {
    config: ClientConfig,
    http: reqwest::Client,
}

impl Client {
    /// Creates a client builder.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::default()
    }

    /// Creates a client from `ANTHROPIC_API_KEY` and optional `ANTHROPIC_BASE_URL`.
    pub fn from_env() -> Result<Self, Error> {
        let api_key = std::env::var(API_KEY_ENV).map_err(|_| Error::MissingApiKey)?;
        let mut builder = Self::builder().api_key(api_key);

        if let Ok(base_url) = std::env::var(BASE_URL_ENV) {
            builder = builder.base_url(&base_url)?;
        }

        builder.build()
    }

    /// Returns the immutable client configuration.
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Returns the Messages API service.
    pub fn messages(&self) -> Messages<'_> {
        Messages::new(self)
    }

    /// Returns the Models API service.
    pub fn models(&self) -> Models<'_> {
        Models::new(self)
    }

    pub(crate) fn http_client(&self) -> &reqwest::Client {
        &self.http
    }
}

/// Builder for [`Client`].
#[derive(Default)]
pub struct ClientBuilder {
    api_key: Option<Result<ApiKey, crate::ApiKeyError>>,
    base_url: Option<BaseUrl>,
    timeout: Option<Duration>,
    max_retries: Option<MaxRetries>,
    http: Option<reqwest::Client>,
}

impl ClientBuilder {
    /// Sets the API key.
    pub fn api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(ApiKey::try_new(api_key));
        self
    }

    /// Sets the API key from a validated value.
    pub fn api_key_value(mut self, api_key: ApiKey) -> Self {
        self.api_key = Some(Ok(api_key));
        self
    }

    /// Sets the base API URL.
    pub fn base_url(mut self, base_url: &str) -> Result<Self, Error> {
        self.base_url =
            Some(BaseUrl::parse(base_url).map_err(|source| Error::InvalidBaseUrl { source })?);
        Ok(self)
    }

    /// Sets the base API URL from a validated value.
    pub fn base_url_value(mut self, base_url: BaseUrl) -> Self {
        self.base_url = Some(base_url);
        self
    }

    /// Sets the default timeout for each non-streaming request attempt.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the default maximum number of retries.
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = Some(MaxRetries::new(max_retries));
        self
    }

    /// Sets the default maximum number of retries from a typed value.
    pub fn max_retries_value(mut self, max_retries: MaxRetries) -> Self {
        self.max_retries = Some(max_retries);
        self
    }

    /// Sets the underlying HTTP client.
    pub fn http_client(mut self, http: reqwest::Client) -> Self {
        self.http = Some(http);
        self
    }

    /// Builds a client.
    pub fn build(self) -> Result<Client, Error> {
        let api_key = self
            .api_key
            .ok_or(Error::MissingApiKey)?
            .map_err(|source| Error::InvalidApiKey { source })?;
        let base_url = match self.base_url {
            Some(base_url) => base_url,
            None => crate::config::default_base_url()
                .map_err(|source| Error::InvalidBaseUrl { source })?,
        };
        let http = match self.http {
            Some(http) => http,
            None => reqwest::Client::new(),
        };
        let timeout = self.timeout.unwrap_or_else(crate::config::default_timeout);
        let max_retries = self
            .max_retries
            .unwrap_or_else(crate::config::default_max_retries);

        Ok(Client {
            config: ClientConfig::with_request_defaults(api_key, base_url, timeout, max_retries),
            http,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_requires_api_key() {
        let result = Client::builder().build();

        assert!(matches!(result, Err(Error::MissingApiKey)));
    }

    #[test]
    fn config_debug_redacts_api_key() -> Result<(), Error> {
        let client = Client::builder().api_key("sk-ant-test-secret").build()?;

        let rendered = format!("{:?}", client.config());

        assert!(rendered.contains("[redacted]"));
        assert!(!rendered.contains("sk-ant-test-secret"));
        Ok(())
    }

    #[test]
    fn builder_accepts_custom_base_url() -> Result<(), Error> {
        let client = Client::builder()
            .api_key("sk-ant-test-secret")
            .base_url("https://example.test")?
            .build()?;

        assert_eq!(client.config().base_url().as_str(), "https://example.test/");
        Ok(())
    }

    #[test]
    fn builder_accepts_anthropic_compatible_base_url_with_path() -> Result<(), Error> {
        let client = Client::builder()
            .api_key("sk-ant-test-secret")
            .base_url("https://api.minimax.io/anthropic")?
            .build()?;

        assert_eq!(
            client.config().base_url().as_str(),
            "https://api.minimax.io/anthropic"
        );
        Ok(())
    }

    #[test]
    fn builder_configures_request_defaults() -> Result<(), Error> {
        let client = Client::builder()
            .api_key("sk-ant-test-secret")
            .timeout(Duration::from_secs(30))
            .max_retries(0)
            .build()?;

        assert_eq!(client.config().timeout(), Duration::from_secs(30));
        assert_eq!(client.config().max_retries().get(), 0);
        Ok(())
    }

    #[test]
    fn builder_uses_readme_request_defaults() -> Result<(), Error> {
        let client = Client::builder().api_key("sk-ant-test-secret").build()?;

        assert_eq!(client.config().timeout(), Duration::from_secs(600));
        assert_eq!(client.config().max_retries().get(), 2);
        Ok(())
    }
}
