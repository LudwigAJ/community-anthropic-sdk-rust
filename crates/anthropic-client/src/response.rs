//! Successful-response metadata wrapper.
//!
//! [`ApiResponse<T>`] is what `_with_response` resource methods return when
//! callers need the response [`anthropic_types::RequestId`] alongside the
//! decoded body. The wrapper is intentionally minimal — pulling metadata
//! out into a separate type keeps response models like
//! [`anthropic_types::Message`] free of HTTP-specific fields.
//!
//! API failures preserve their request IDs through [`crate::ApiError`]
//! instead, so `Result<ApiResponse<T>, Error>` covers both happy and error
//! paths without losing the ID.

use std::fmt;

use anthropic_types::RequestId;

/// Decoded API data plus response metadata returned by the HTTP layer.
///
/// `Debug` intentionally redacts the decoded data because API responses can
/// contain model output or other application-sensitive content.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiResponse<T> {
    /// Decoded response data.
    pub data: T,
    /// Request identifier returned by the API, when present.
    pub request_id: Option<RequestId>,
}

impl<T> ApiResponse<T> {
    /// Creates a response wrapper from decoded data and optional metadata.
    pub fn new(data: T, request_id: Option<RequestId>) -> Self {
        Self { data, request_id }
    }

    /// Returns the request identifier as a string slice, when present.
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_ref().map(RequestId::as_str)
    }

    /// Converts this wrapper into the decoded response data.
    pub fn into_data(self) -> T {
        self.data
    }
}

impl<T> fmt::Debug for ApiResponse<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiResponse")
            .field("data", &"[redacted]")
            .field("request_id", &self.request_id)
            .finish()
    }
}
