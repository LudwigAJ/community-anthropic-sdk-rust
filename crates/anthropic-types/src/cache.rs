//! Prompt-cache markers shared across request surfaces.
//!
//! [`CacheControl`] is the typed `cache_control` value attached to system
//! prompts, request content blocks, tool definitions, and the top-level
//! [`crate::MessageCreateParams`] field. Today the only kind is
//! `ephemeral`, optionally carrying a [`CacheControlTtl`] of
//! `FiveMinutes` or `OneHour`; the enum shape is forward-compatible for
//! additional kinds without breaking the wire format.

use serde::{Deserialize, Serialize};

/// Cache control marker for request-level prompt caching.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CacheControl {
    /// Mark a cacheable request block as ephemeral.
    Ephemeral {
        /// Optional time-to-live for the cache breakpoint.
        #[serde(skip_serializing_if = "Option::is_none")]
        ttl: Option<CacheControlTtl>,
    },
}

impl CacheControl {
    /// Creates an ephemeral cache control marker using the API default TTL.
    pub fn ephemeral() -> Self {
        Self::Ephemeral { ttl: None }
    }

    /// Creates an ephemeral cache control marker with an explicit TTL.
    pub fn ephemeral_with_ttl(ttl: CacheControlTtl) -> Self {
        Self::Ephemeral { ttl: Some(ttl) }
    }
}

/// Time-to-live for an ephemeral cache control marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheControlTtl {
    /// Five-minute cache lifetime.
    #[serde(rename = "5m")]
    FiveMinutes,
    /// One-hour cache lifetime.
    #[serde(rename = "1h")]
    OneHour,
}
