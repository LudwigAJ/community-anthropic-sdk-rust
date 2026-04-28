//! Cursor-pagination shapes shared by `models.list` and `batches.list`.
//!
//! [`Page<T>`] is the on-wire response: `data`, `first_id`, `last_id`,
//! `has_more`. [`Page::next_page_params`] turns a page plus the caller's
//! original `limit` into a follow-up [`ListParams`] without manual cursor
//! threading.
//!
//! [`ListParams`] / [`ListParamsBuilder`] is the typed query input.
//! Building it rejects `limit == 0` and conflicting `before_id` /
//! `after_id` cursors at construction (`ListParamsError`), so invalid
//! cursors never reach the wire.
//!
//! The async stream side of pagination — typed `AutoItemStream` and
//! `AutoPageStream` — lives in `anthropic_client::pagination` and reuses
//! these data shapes.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A cursor-paginated API response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page<T> {
    /// Page items.
    pub data: Vec<T>,
    /// Identifier for the first item in this page.
    pub first_id: Option<String>,
    /// Identifier for the last item in this page.
    pub last_id: Option<String>,
    /// Whether another page is available.
    pub has_more: bool,
}

impl<T> Page<T> {
    /// Returns parameters for fetching the next page, when one is available.
    ///
    /// Carries forward the requested page size and uses the current page's
    /// `last_id` as the next cursor.
    pub fn next_page_params(&self, limit: Option<u32>) -> Option<ListParams> {
        if !self.has_more {
            return None;
        }

        let after_id = self.last_id.as_ref()?.clone();
        let mut builder = ListParams::builder().after_id(after_id);
        if let Some(limit) = limit {
            builder = builder.limit(limit);
        }
        builder.build().ok()
    }
}

/// Common cursor-pagination request parameters for list endpoints.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListParams {
    /// Number of items to return per page (server-defined bounds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Return items immediately before this object identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_id: Option<String>,
    /// Return items immediately after this object identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_id: Option<String>,
}

impl ListParams {
    /// Creates an empty set of list parameters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a builder for list parameters.
    pub fn builder() -> ListParamsBuilder {
        ListParamsBuilder::default()
    }

    /// Returns true when no parameters are set.
    pub fn is_empty(&self) -> bool {
        self.limit.is_none() && self.before_id.is_none() && self.after_id.is_none()
    }
}

/// Builder for [`ListParams`].
#[derive(Debug, Default, Clone)]
pub struct ListParamsBuilder {
    limit: Option<u32>,
    before_id: Option<String>,
    after_id: Option<String>,
}

impl ListParamsBuilder {
    /// Sets the page size.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the `before_id` cursor.
    pub fn before_id(mut self, before_id: impl Into<String>) -> Self {
        self.before_id = Some(before_id.into());
        self
    }

    /// Sets the `after_id` cursor.
    pub fn after_id(mut self, after_id: impl Into<String>) -> Self {
        self.after_id = Some(after_id.into());
        self
    }

    /// Builds validated list parameters.
    pub fn build(self) -> Result<ListParams, ListParamsError> {
        if let Some(limit) = self.limit {
            if limit == 0 {
                return Err(ListParamsError::ZeroLimit);
            }
        }
        if self.before_id.is_some() && self.after_id.is_some() {
            return Err(ListParamsError::CursorConflict);
        }

        Ok(ListParams {
            limit: self.limit,
            before_id: self.before_id,
            after_id: self.after_id,
        })
    }
}

/// Errors produced while building [`ListParams`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ListParamsError {
    /// `limit` must be at least 1 when set.
    #[error("list params limit must be greater than zero")]
    ZeroLimit,
    /// `before_id` and `after_id` are mutually exclusive.
    #[error("list params cannot set both before_id and after_id")]
    CursorConflict,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_rejects_zero_limit() {
        assert_eq!(
            ListParams::builder().limit(0).build(),
            Err(ListParamsError::ZeroLimit)
        );
    }

    #[test]
    fn builder_rejects_conflicting_cursors() {
        assert_eq!(
            ListParams::builder().after_id("a").before_id("b").build(),
            Err(ListParamsError::CursorConflict)
        );
    }

    #[test]
    fn empty_builder_serializes_to_empty_object() -> Result<(), Box<dyn std::error::Error>> {
        let params = ListParams::builder().build()?;
        let value = serde_json::to_value(&params)?;
        assert_eq!(value, serde_json::json!({}));
        assert!(params.is_empty());
        Ok(())
    }

    #[test]
    fn next_page_params_uses_last_id() -> Result<(), Box<dyn std::error::Error>> {
        let page: Page<String> = Page {
            data: vec!["a".to_owned(), "b".to_owned()],
            first_id: Some("a".to_owned()),
            last_id: Some("b".to_owned()),
            has_more: true,
        };
        let next = page
            .next_page_params(Some(20))
            .ok_or("expected next page params")?;
        assert_eq!(next.after_id.as_deref(), Some("b"));
        assert_eq!(next.limit, Some(20));
        assert_eq!(next.before_id, None);
        Ok(())
    }

    #[test]
    fn next_page_params_returns_none_at_end() {
        let page: Page<String> = Page {
            data: vec![],
            first_id: None,
            last_id: None,
            has_more: false,
        };
        assert!(page.next_page_params(None).is_none());
    }
}
