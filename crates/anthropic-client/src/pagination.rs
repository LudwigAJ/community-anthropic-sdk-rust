//! Generic auto-pagination machinery for cursor-paginated endpoints.
//!
//! Both the Models and Message Batches resources page identically: a request
//! returns [`anthropic_types::Page<T>`] with `first_id`, `last_id`, and
//! `has_more`. This module abstracts that pattern into two stream types:
//!
//! - [`AutoPageStream`] yields one
//!   `Result<crate::ApiResponse<Page<T>>, crate::Error>` per fetched page so
//!   callers can read per-page request IDs.
//! - [`AutoItemStream`] flattens a page stream into a stream of
//!   `Result<T, crate::Error>` for the common "walk every item" case.
//!
//! Both streams fetch the first page lazily on the first poll, follow
//! `last_id` as the next `after_id` (or `first_id` as the next `before_id`
//! when the caller starts with a `before_id` cursor), preserve the original
//! `limit`, and apply [`crate::RequestOptions`] to every page request. They
//! stop on `has_more == false`. API errors short-circuit and propagate with
//! the request ID intact. Cancellation is by drop.

use std::{
    collections::VecDeque,
    fmt,
    pin::Pin,
    task::{Context, Poll},
};

use anthropic_types::{ListParams, Page, RequestId};
use futures_util::{Stream, future::BoxFuture};

use crate::{ApiResponse, Client, Error, RequestOptions};

pub(crate) type PageFetcher<'a, T> = fn(
    &'a Client,
    ListParams,
    RequestOptions,
) -> BoxFuture<'a, Result<ApiResponse<Page<T>>, Error>>;

/// Stream of cursor-paginated pages.
///
/// The stream fetches the first page lazily on the first poll and then follows
/// `last_id` as `after_id` for forward pagination. If the initial parameters
/// use `before_id`, it follows `first_id` as the next `before_id`, matching the
/// reverse-pagination behavior of the reference SDKs.
pub struct AutoPageStream<'a, T> {
    client: &'a Client,
    options: RequestOptions,
    fetch: PageFetcher<'a, T>,
    next_params: Option<ListParams>,
    in_flight_params: Option<ListParams>,
    in_flight: Option<BoxFuture<'a, Result<ApiResponse<Page<T>>, Error>>>,
    finished: bool,
    last_request_id: Option<RequestId>,
}

impl<T> Unpin for AutoPageStream<'_, T> {}

impl<'a, T> AutoPageStream<'a, T> {
    pub(crate) fn new(
        client: &'a Client,
        params: ListParams,
        options: RequestOptions,
        fetch: PageFetcher<'a, T>,
    ) -> Self {
        Self {
            client,
            options,
            fetch,
            next_params: Some(params),
            in_flight_params: None,
            in_flight: None,
            finished: false,
            last_request_id: None,
        }
    }

    /// Returns the request ID from the most recently fetched page, when present.
    pub fn last_request_id(&self) -> Option<&str> {
        self.last_request_id.as_ref().map(RequestId::as_str)
    }
}

impl<T> fmt::Debug for AutoPageStream<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AutoPageStream")
            .field("next_params", &self.next_params)
            .field("in_flight", &self.in_flight.is_some())
            .field("finished", &self.finished)
            .field("last_request_id", &self.last_request_id)
            .finish_non_exhaustive()
    }
}

impl<'a, T> Stream for AutoPageStream<'a, T>
where
    T: Send + 'a,
{
    type Item = Result<ApiResponse<Page<T>>, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            if this.finished {
                return Poll::Ready(None);
            }

            if this.in_flight.is_none() {
                let Some(params) = this.next_params.take() else {
                    this.finished = true;
                    return Poll::Ready(None);
                };
                this.in_flight_params = Some(params.clone());
                this.in_flight = Some((this.fetch)(this.client, params, this.options.clone()));
            }

            let Some(future) = &mut this.in_flight else {
                continue;
            };

            match future.as_mut().poll(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(result) => {
                    this.in_flight = None;
                    let params = this.in_flight_params.take();
                    match result {
                        Ok(response) => {
                            if let Some(current_params) = params.as_ref() {
                                this.next_params = next_page_params(current_params, &response.data);
                            } else {
                                this.next_params = None;
                            }
                            if this.next_params.is_none() {
                                this.finished = true;
                            }
                            this.last_request_id = response.request_id.clone();
                            return Poll::Ready(Some(Ok(response)));
                        }
                        Err(error) => {
                            this.finished = true;
                            return Poll::Ready(Some(Err(error)));
                        }
                    }
                }
            }
        }
    }
}

/// Stream of individual items from a cursor-paginated endpoint.
///
/// This wraps [`AutoPageStream`] and yields each page item in order. Use the page
/// stream directly when callers need every page's response metadata.
pub struct AutoItemStream<'a, T> {
    pages: AutoPageStream<'a, T>,
    pending: VecDeque<T>,
}

impl<T> Unpin for AutoItemStream<'_, T> {}

impl<'a, T> AutoItemStream<'a, T> {
    pub(crate) fn new(pages: AutoPageStream<'a, T>) -> Self {
        Self {
            pages,
            pending: VecDeque::new(),
        }
    }

    /// Returns the request ID from the most recently fetched page, when present.
    pub fn last_request_id(&self) -> Option<&str> {
        self.pages.last_request_id()
    }
}

impl<T> fmt::Debug for AutoItemStream<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AutoItemStream")
            .field("pages", &self.pages)
            .field("pending_items", &self.pending.len())
            .finish_non_exhaustive()
    }
}

impl<'a, T> Stream for AutoItemStream<'a, T>
where
    T: Send + 'a,
{
    type Item = Result<T, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            if let Some(item) = this.pending.pop_front() {
                return Poll::Ready(Some(Ok(item)));
            }

            match Pin::new(&mut this.pages).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(page))) => {
                    this.pending = VecDeque::from(page.data.data);
                }
                Poll::Ready(Some(Err(error))) => return Poll::Ready(Some(Err(error))),
                Poll::Ready(None) => return Poll::Ready(None),
            }
        }
    }
}

fn next_page_params<T>(current: &ListParams, page: &Page<T>) -> Option<ListParams> {
    if !page.has_more || page.data.is_empty() {
        return None;
    }

    if current.before_id.is_some() {
        let before_id = page.first_id.clone()?;
        return Some(ListParams {
            limit: current.limit,
            before_id: Some(before_id),
            after_id: None,
        });
    }

    let after_id = page.last_id.clone()?;
    Some(ListParams {
        limit: current.limit,
        before_id: None,
        after_id: Some(after_id),
    })
}
