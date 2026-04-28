//! Per-request configuration overrides.
//!
//! [`RequestOptions`] is what every `_with` and `_with_response` resource
//! method accepts. It overrides the client-level timeout, retry count, extra
//! headers, and query parameters for a single call without mutating the
//! shared [`crate::Client`].
//!
//! Header names and values are validated when [`RequestOptionsBuilder::build`]
//! is called, so invalid characters surface as [`RequestOptionsBuildError`]
//! at construction rather than as opaque transport errors. Headers are
//! additive on top of the client default unless the caller explicitly
//! overrides a key.

use std::{fmt, time::Duration};

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, InvalidHeaderName, InvalidHeaderValue};
use thiserror::Error as ThisError;

use crate::MaxRetries;

/// Per-request options applied on top of client configuration.
#[derive(Clone, Default)]
pub struct RequestOptions {
    timeout: Option<Duration>,
    max_retries: Option<MaxRetries>,
    headers: HeaderMap,
    replaced_headers: Vec<HeaderName>,
}

impl RequestOptions {
    /// Creates an empty set of request options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a builder for request options.
    pub fn builder() -> RequestOptionsBuilder {
        RequestOptionsBuilder::default()
    }

    /// Returns the request timeout override, when present.
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Returns the retry-count override, when present.
    pub fn max_retries(&self) -> Option<MaxRetries> {
        self.max_retries
    }

    /// Returns additive request headers.
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    pub(crate) fn effective_timeout(&self, default: Duration) -> Duration {
        self.timeout.unwrap_or(default)
    }

    pub(crate) fn effective_max_retries(&self, default: MaxRetries) -> MaxRetries {
        self.max_retries.unwrap_or(default)
    }

    pub(crate) fn replaced_header_names(&self) -> impl Iterator<Item = &HeaderName> {
        self.replaced_headers.iter()
    }
}

impl fmt::Debug for RequestOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RequestOptions")
            .field("timeout", &self.timeout)
            .field("max_retries", &self.max_retries)
            .field("header_count", &self.headers.len())
            .finish()
    }
}

/// Builder for [`RequestOptions`].
#[derive(Debug, Default)]
pub struct RequestOptionsBuilder {
    timeout: Option<Duration>,
    max_retries: Option<MaxRetries>,
    headers: HeaderMap,
    replaced_headers: Vec<HeaderName>,
    error: Option<RequestOptionsBuildError>,
}

impl RequestOptionsBuilder {
    /// Sets the per-request timeout override.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Sets the maximum number of retries for this request.
    pub fn max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = Some(MaxRetries::new(max_retries));
        self
    }

    /// Sets the maximum number of retries from a typed value.
    pub fn max_retries_value(mut self, max_retries: MaxRetries) -> Self {
        self.max_retries = Some(max_retries);
        self
    }

    /// Appends an HTTP header value.
    pub fn header(self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.header_inner(name.as_ref(), value.as_ref(), HeaderMode::Append)
    }

    /// Appends a validated HTTP header value.
    pub fn header_value(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.headers.append(name, value);
        self
    }

    /// Sets an HTTP header value, replacing existing values for that name.
    pub fn set_header(self, name: impl AsRef<str>, value: impl AsRef<str>) -> Self {
        self.header_inner(name.as_ref(), value.as_ref(), HeaderMode::Set)
    }

    /// Sets a validated HTTP header value, replacing existing values for that name.
    pub fn set_header_value(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.mark_replaced(&name);
        self.headers.insert(name, value);
        self
    }

    /// Builds validated request options.
    pub fn build(self) -> Result<RequestOptions, RequestOptionsBuildError> {
        if let Some(error) = self.error {
            return Err(error);
        }

        Ok(RequestOptions {
            timeout: self.timeout,
            max_retries: self.max_retries,
            headers: self.headers,
            replaced_headers: self.replaced_headers,
        })
    }

    fn header_inner(mut self, name: &str, value: &str, mode: HeaderMode) -> Self {
        if self.error.is_some() {
            return self;
        }

        let name = match HeaderName::from_bytes(name.as_bytes()) {
            Ok(name) => name,
            Err(source) => {
                self.error = Some(RequestOptionsBuildError::InvalidHeaderName { source });
                return self;
            }
        };
        let value = match HeaderValue::from_str(value) {
            Ok(value) => value,
            Err(source) => {
                self.error = Some(RequestOptionsBuildError::InvalidHeaderValue { source });
                return self;
            }
        };

        match mode {
            HeaderMode::Append => {
                self.headers.append(name, value);
            }
            HeaderMode::Set => {
                self.mark_replaced(&name);
                self.headers.insert(name, value);
            }
        }

        self
    }

    fn mark_replaced(&mut self, name: &HeaderName) {
        if !self
            .replaced_headers
            .iter()
            .any(|replaced| replaced == name)
        {
            self.replaced_headers.push(name.clone());
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum HeaderMode {
    Append,
    Set,
}

/// Errors produced while building [`RequestOptions`].
#[derive(Debug, ThisError)]
pub enum RequestOptionsBuildError {
    /// Header name validation failed.
    #[error("invalid request header name")]
    InvalidHeaderName {
        /// Header parser source error.
        #[source]
        source: InvalidHeaderName,
    },
    /// Header value validation failed.
    #[error("invalid request header value")]
    InvalidHeaderValue {
        /// Header parser source error.
        #[source]
        source: InvalidHeaderValue,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_records_timeout_retries_and_additive_headers()
    -> Result<(), Box<dyn std::error::Error>> {
        let options = RequestOptions::builder()
            .timeout(Duration::from_secs(30))
            .max_retries(0)
            .header("anthropic-beta", "first-beta")
            .header("anthropic-beta", "second-beta")
            .set_header("x-request-source", "builder")
            .set_header("x-request-source", "override")
            .build()?;

        assert_eq!(options.timeout(), Some(Duration::from_secs(30)));
        assert_eq!(options.max_retries().map(MaxRetries::get), Some(0));

        let betas = options
            .headers()
            .get_all("anthropic-beta")
            .iter()
            .map(HeaderValue::to_str)
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(betas, vec!["first-beta", "second-beta"]);
        assert_eq!(
            options
                .headers()
                .get("x-request-source")
                .and_then(|value| value.to_str().ok()),
            Some("override")
        );
        Ok(())
    }

    #[test]
    fn builder_rejects_invalid_header_name() {
        let result = RequestOptions::builder()
            .header("bad header", "value")
            .build();

        assert!(matches!(
            result,
            Err(RequestOptionsBuildError::InvalidHeaderName { .. })
        ));
    }

    #[test]
    fn builder_rejects_invalid_header_value() {
        let result = RequestOptions::builder()
            .header("x-test", "bad\nvalue")
            .build();

        assert!(matches!(
            result,
            Err(RequestOptionsBuildError::InvalidHeaderValue { .. })
        ));
    }

    #[test]
    fn debug_redacts_header_values() -> Result<(), RequestOptionsBuildError> {
        let options = RequestOptions::builder()
            .header("authorization", "Bearer secret")
            .build()?;

        let rendered = format!("{options:?}");

        assert!(rendered.contains("header_count"));
        assert!(!rendered.contains("Bearer secret"));
        Ok(())
    }
}
