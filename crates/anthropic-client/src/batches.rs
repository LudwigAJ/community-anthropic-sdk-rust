//! Message Batches API service boundary.
//!
//! Nested under the Messages service: `client.messages().batches()`. Hosts
//! every operation under `/v1/messages/batches`:
//!
//! - `create` — submit a [`anthropic_types::BatchCreateParams`] payload and
//!   receive a [`anthropic_types::MessageBatch`].
//! - `retrieve` / `cancel` / `delete` — lifecycle calls keyed by
//!   [`anthropic_types::MessageBatchId`]. Blank IDs are rejected before the
//!   request leaves the process.
//! - `list` / `list_with_params` / `list_auto_paging_with(_params)` /
//!   `list_pages_with` — single-page and auto-paginated reads, sharing the
//!   pagination machinery in [`crate::pagination`].
//! - `results` / `results_with` — typed JSONL streaming
//!   ([`BatchResultsStream`]) over the batch's `results_url`. The method
//!   first calls `retrieve(id)`; if the batch has not finished, it returns
//!   [`crate::Error::BatchResultsUnavailable`] without issuing the JSONL
//!   download.
//!
//! Each public stream type ([`BatchResultsStream`], [`MessageBatchStream`],
//! [`MessageBatchesPageStream`]) implements `futures_core::Stream` and is
//! cancellable by drop.

use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
};

use anthropic_types::{
    BatchCreateParams, DeletedMessageBatch, ListParams, MessageBatch, MessageBatchId,
    MessageBatchIndividualResponse, Page, RequestId,
};
use bytes::Bytes;
use futures_util::{Stream, future::BoxFuture};
use reqwest::StatusCode;

use crate::{ApiResponse, AutoItemStream, AutoPageStream, Client, Error, RequestOptions};

/// Page of message batches returned by the list endpoint.
pub type MessageBatchesPage = Page<MessageBatch>;
/// Stream of message batch pages returned by auto-pagination helpers.
pub type MessageBatchesPageStream<'a> = AutoPageStream<'a, MessageBatch>;
/// Stream of message batches returned by auto-pagination helpers.
pub type MessageBatchStream<'a> = AutoItemStream<'a, MessageBatch>;

/// Service for the Message Batches API.
#[derive(Debug, Clone, Copy)]
pub struct Batches<'a> {
    client: &'a Client,
}

impl<'a> Batches<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Creates a new message batch.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches"))]
    pub async fn create(&self, params: BatchCreateParams) -> Result<MessageBatch, Error> {
        self.create_with(params, RequestOptions::new()).await
    }

    /// Creates a new message batch with per-request options.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches"))]
    pub async fn create_with(
        &self,
        params: BatchCreateParams,
        options: RequestOptions,
    ) -> Result<MessageBatch, Error> {
        self.create_with_response(params, options)
            .await
            .map(ApiResponse::into_data)
    }

    /// Creates a new message batch with per-request options and response metadata.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches"))]
    pub async fn create_with_response(
        &self,
        params: BatchCreateParams,
        options: RequestOptions,
    ) -> Result<ApiResponse<MessageBatch>, Error> {
        let prepared = crate::request::build_messages_batches_create(
            self.client.http_client(),
            self.client.config(),
            &params,
            &options,
        )?;
        crate::transport::execute_json_response(self.client.http_client(), prepared).await
    }

    /// Retrieves a message batch by identifier.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches/{batch}"))]
    pub async fn retrieve(&self, batch_id: impl AsRef<str>) -> Result<MessageBatch, Error> {
        self.retrieve_with(batch_id, RequestOptions::new()).await
    }

    /// Retrieves a message batch with per-request options.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches/{batch}"))]
    pub async fn retrieve_with(
        &self,
        batch_id: impl AsRef<str>,
        options: RequestOptions,
    ) -> Result<MessageBatch, Error> {
        self.retrieve_with_response(batch_id, options)
            .await
            .map(ApiResponse::into_data)
    }

    /// Retrieves a message batch with per-request options and response metadata.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches/{batch}"))]
    pub async fn retrieve_with_response(
        &self,
        batch_id: impl AsRef<str>,
        options: RequestOptions,
    ) -> Result<ApiResponse<MessageBatch>, Error> {
        let prepared = crate::request::build_messages_batches_retrieve(
            self.client.http_client(),
            self.client.config(),
            batch_id.as_ref(),
            &options,
        )?;
        crate::transport::execute_json_response(self.client.http_client(), prepared).await
    }

    /// Lists message batches with default pagination.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches"))]
    pub async fn list(&self) -> Result<MessageBatchesPage, Error> {
        self.list_with_params(ListParams::new()).await
    }

    /// Lists message batches with explicit pagination parameters.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches"))]
    pub async fn list_with_params(&self, params: ListParams) -> Result<MessageBatchesPage, Error> {
        self.list_with(params, RequestOptions::new()).await
    }

    /// Lists message batches with pagination and per-request options.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches"))]
    pub async fn list_with(
        &self,
        params: ListParams,
        options: RequestOptions,
    ) -> Result<MessageBatchesPage, Error> {
        self.list_with_response(params, options)
            .await
            .map(ApiResponse::into_data)
    }

    /// Lists message batches with pagination, options, and response metadata.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches"))]
    pub async fn list_with_response(
        &self,
        params: ListParams,
        options: RequestOptions,
    ) -> Result<ApiResponse<MessageBatchesPage>, Error> {
        let prepared = crate::request::build_messages_batches_list(
            self.client.http_client(),
            self.client.config(),
            &params,
            &options,
        )?;
        crate::transport::execute_json_response(self.client.http_client(), prepared).await
    }

    /// Returns a stream that auto-paginates message batch items with default parameters.
    pub fn list_auto_paging(&self) -> MessageBatchStream<'a> {
        self.list_auto_paging_with(ListParams::new(), RequestOptions::new())
    }

    /// Returns a stream that auto-paginates message batch items with explicit parameters.
    pub fn list_auto_paging_with_params(&self, params: ListParams) -> MessageBatchStream<'a> {
        self.list_auto_paging_with(params, RequestOptions::new())
    }

    /// Returns a stream that auto-paginates message batch items with parameters and options.
    ///
    /// Request options are applied to every page request.
    pub fn list_auto_paging_with(
        &self,
        params: ListParams,
        options: RequestOptions,
    ) -> MessageBatchStream<'a> {
        AutoItemStream::new(self.list_pages_with(params, options))
    }

    /// Returns a stream that auto-paginates message batch pages with default parameters.
    pub fn list_pages(&self) -> MessageBatchesPageStream<'a> {
        self.list_pages_with(ListParams::new(), RequestOptions::new())
    }

    /// Returns a stream that auto-paginates message batch pages with explicit parameters.
    pub fn list_pages_with_params(&self, params: ListParams) -> MessageBatchesPageStream<'a> {
        self.list_pages_with(params, RequestOptions::new())
    }

    /// Returns a stream that auto-paginates message batch pages with parameters and options.
    ///
    /// Each yielded page includes response metadata such as `request-id`.
    /// Request options are applied to every page request.
    pub fn list_pages_with(
        &self,
        params: ListParams,
        options: RequestOptions,
    ) -> MessageBatchesPageStream<'a> {
        AutoPageStream::new(self.client, params, options, fetch_message_batches_page)
    }

    /// Cancels a message batch.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches/{batch}/cancel"))]
    pub async fn cancel(&self, batch_id: impl AsRef<str>) -> Result<MessageBatch, Error> {
        self.cancel_with(batch_id, RequestOptions::new()).await
    }

    /// Cancels a message batch with per-request options.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches/{batch}/cancel"))]
    pub async fn cancel_with(
        &self,
        batch_id: impl AsRef<str>,
        options: RequestOptions,
    ) -> Result<MessageBatch, Error> {
        self.cancel_with_response(batch_id, options)
            .await
            .map(ApiResponse::into_data)
    }

    /// Cancels a message batch with per-request options and response metadata.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches/{batch}/cancel"))]
    pub async fn cancel_with_response(
        &self,
        batch_id: impl AsRef<str>,
        options: RequestOptions,
    ) -> Result<ApiResponse<MessageBatch>, Error> {
        let prepared = crate::request::build_messages_batches_cancel(
            self.client.http_client(),
            self.client.config(),
            batch_id.as_ref(),
            &options,
        )?;
        crate::transport::execute_json_response(self.client.http_client(), prepared).await
    }

    /// Deletes a message batch after it has finished processing.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches/{batch}"))]
    pub async fn delete(&self, batch_id: impl AsRef<str>) -> Result<DeletedMessageBatch, Error> {
        self.delete_with(batch_id, RequestOptions::new()).await
    }

    /// Deletes a message batch with per-request options.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches/{batch}"))]
    pub async fn delete_with(
        &self,
        batch_id: impl AsRef<str>,
        options: RequestOptions,
    ) -> Result<DeletedMessageBatch, Error> {
        self.delete_with_response(batch_id, options)
            .await
            .map(ApiResponse::into_data)
    }

    /// Deletes a message batch with per-request options and response metadata.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches/{batch}"))]
    pub async fn delete_with_response(
        &self,
        batch_id: impl AsRef<str>,
        options: RequestOptions,
    ) -> Result<ApiResponse<DeletedMessageBatch>, Error> {
        let prepared = crate::request::build_messages_batches_delete(
            self.client.http_client(),
            self.client.config(),
            batch_id.as_ref(),
            &options,
        )?;
        crate::transport::execute_json_response(self.client.http_client(), prepared).await
    }

    /// Streams the JSONL results of a message batch.
    ///
    /// Retrieves the batch first, returns
    /// [`Error::BatchResultsUnavailable`](crate::Error::BatchResultsUnavailable)
    /// if the batch has not yet ended (no `results_url`), then streams the
    /// `application/x-ndjson` response one decoded
    /// [`MessageBatchIndividualResponse`] at a time.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches/{batch}/results"))]
    pub async fn results(&self, batch_id: impl AsRef<str>) -> Result<BatchResultsStream, Error> {
        self.results_with(batch_id, RequestOptions::new()).await
    }

    /// Streams batch results with per-request options applied to lookup and download.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/batches/{batch}/results"))]
    pub async fn results_with(
        &self,
        batch_id: impl AsRef<str>,
        options: RequestOptions,
    ) -> Result<BatchResultsStream, Error> {
        let batch_id = MessageBatchId::try_new(batch_id.as_ref())
            .map_err(|source| Error::InvalidMessageBatchId { source })?;
        let batch = self
            .retrieve_with(batch_id.as_str(), options.clone())
            .await?;
        let Some(results_url) = batch.results_url.clone() else {
            return Err(Error::BatchResultsUnavailable {
                batch_id: batch.id.into_string(),
                processing_status: batch.processing_status,
            });
        };

        let prepared = crate::request::build_messages_batches_results(
            self.client.http_client(),
            self.client.config(),
            &results_url,
            &options,
        )?;
        let response =
            crate::transport::execute_raw_response(self.client.http_client(), prepared).await?;
        Ok(BatchResultsStream::from_response(response))
    }
}

fn fetch_message_batches_page<'a>(
    client: &'a Client,
    params: ListParams,
    options: RequestOptions,
) -> BoxFuture<'a, Result<ApiResponse<MessageBatchesPage>, Error>> {
    Box::pin(async move {
        let prepared = crate::request::build_messages_batches_list(
            client.http_client(),
            client.config(),
            &params,
            &options,
        )?;
        crate::transport::execute_json_response(client.http_client(), prepared).await
    })
}

type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>;

/// Streamed JSONL results for a message batch.
///
/// Yields one [`MessageBatchIndividualResponse`] per line. Dropping the value
/// cancels the underlying HTTP response body.
pub struct BatchResultsStream {
    inner: ByteStream,
    buffer: Vec<u8>,
    pending: VecDeque<Result<MessageBatchIndividualResponse, Error>>,
    finished: bool,
    request_id: Option<RequestId>,
    status: StatusCode,
}

impl BatchResultsStream {
    pub(crate) fn from_response(response: reqwest::Response) -> Self {
        let status = response.status();
        let request_id = crate::transport::response_request_id(&response);
        Self {
            inner: Box::pin(response.bytes_stream()),
            buffer: Vec::new(),
            pending: VecDeque::new(),
            finished: false,
            request_id,
            status,
        }
    }

    /// Returns the request ID from the results download, when present.
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_ref().map(RequestId::as_str)
    }

    /// Returns the HTTP status of the results download.
    pub fn status(&self) -> StatusCode {
        self.status
    }

    fn drain_complete_lines(&mut self) {
        while let Some(newline) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let mut line = self.buffer.drain(..=newline).collect::<Vec<_>>();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            self.push_line(line);
            if self.finished {
                break;
            }
        }
    }

    fn drain_final_line(&mut self) {
        if self.buffer.is_empty() {
            return;
        }
        let line = std::mem::take(&mut self.buffer);
        self.push_line(line);
    }

    fn push_line(&mut self, line: Vec<u8>) {
        if line.iter().all(u8::is_ascii_whitespace) {
            return;
        }
        match serde_json::from_slice::<MessageBatchIndividualResponse>(&line) {
            Ok(response) => self.pending.push_back(Ok(response)),
            Err(source) => {
                self.pending.push_back(Err(Error::Json { source }));
                self.buffer.clear();
                self.finished = true;
            }
        }
    }
}

impl std::fmt::Debug for BatchResultsStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BatchResultsStream")
            .field("request_id", &self.request_id)
            .field("status", &self.status)
            .field("buffered_bytes", &self.buffer.len())
            .field("pending_lines", &self.pending.len())
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl Stream for BatchResultsStream {
    type Item = Result<MessageBatchIndividualResponse, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            if let Some(item) = this.pending.pop_front() {
                return Poll::Ready(Some(item));
            }

            if this.finished {
                return Poll::Ready(None);
            }

            match this.inner.as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(chunk))) => {
                    this.buffer.extend_from_slice(&chunk);
                    this.drain_complete_lines();
                }
                Poll::Ready(Some(Err(source))) => {
                    this.finished = true;
                    return Poll::Ready(Some(Err(Error::Transport { source })));
                }
                Poll::Ready(None) => {
                    this.finished = true;
                    this.drain_final_line();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    use anthropic_types::{
        BatchCreateRequest, BatchProcessingStatus, MessageBatchResult, MessageCreateParams,
        MessageParam, Model,
    };
    use futures_util::StreamExt;

    use super::*;
    use crate::{ApiErrorKind, Client};

    struct MockServer {
        base_url: String,
        handle: thread::JoinHandle<std::io::Result<String>>,
    }

    impl MockServer {
        fn base_url(&self) -> &str {
            &self.base_url
        }

        fn join(self) -> Result<String, Box<dyn std::error::Error>> {
            let request = self
                .handle
                .join()
                .map_err(|_| std::io::Error::other("mock server thread panicked"))??;
            Ok(request)
        }
    }

    struct MockSequenceServer {
        base_url: String,
        handle: thread::JoinHandle<std::io::Result<Vec<String>>>,
    }

    impl MockSequenceServer {
        fn base_url(&self) -> &str {
            &self.base_url
        }

        fn join(self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
            self.handle
                .join()
                .map_err(|_| std::io::Error::other("mock server thread panicked"))?
                .map_err(Into::into)
        }
    }

    fn write_response(
        stream: &mut std::net::TcpStream,
        status: u16,
        headers: &[(String, String)],
        body: &[u8],
    ) -> std::io::Result<()> {
        let mut response = format!(
            "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n",
            body.len()
        );
        for (name, value) in headers {
            response.push_str(name);
            response.push_str(": ");
            response.push_str(value);
            response.push_str("\r\n");
        }
        response.push_str("\r\n");
        stream.write_all(response.as_bytes())?;
        stream.write_all(body)?;
        Ok(())
    }

    fn read_request(stream: &mut std::net::TcpStream) -> std::io::Result<String> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 8192];
        loop {
            let read = stream.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if let Some(pos) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                let header_end = pos + 4;
                let content_length = parse_content_length(&request[..header_end]);
                let total = header_end + content_length;
                while request.len() < total {
                    let read = stream.read(&mut buffer)?;
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                }
                break;
            }
        }
        Ok(String::from_utf8_lossy(&request).into_owned())
    }

    fn parse_content_length(request: &[u8]) -> usize {
        let text = std::str::from_utf8(request).unwrap_or("");
        for line in text.lines() {
            let lower = line.to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("content-length:") {
                if let Ok(value) = rest.trim().parse::<usize>() {
                    return value;
                }
            }
        }
        0
    }

    fn spawn_mock(
        status: u16,
        headers: &[(&str, &str)],
        body: &[u8],
    ) -> Result<MockServer, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        let base_url = format!("http://{addr}");
        let headers = headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect::<Vec<_>>();
        let body = body.to_vec();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let request = read_request(&mut stream)?;
            write_response(&mut stream, status, &headers, &body)?;
            Ok(request)
        });

        Ok(MockServer { base_url, handle })
    }

    type MockResponse = (u16, Vec<(&'static str, &'static str)>, Vec<u8>);

    fn spawn_sequence(
        responses: Vec<MockResponse>,
    ) -> Result<MockSequenceServer, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        let base_url = format!("http://{addr}");

        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for (status, headers, body) in responses {
                let (mut stream, _) = listener.accept()?;
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                requests.push(read_request(&mut stream)?);
                let owned_headers = headers
                    .into_iter()
                    .map(|(n, v)| (n.to_owned(), v.to_owned()))
                    .collect::<Vec<_>>();
                write_response(&mut stream, status, &owned_headers, &body)?;
            }
            Ok(requests)
        });

        Ok(MockSequenceServer { base_url, handle })
    }

    fn make_client(base_url: &str) -> Result<Client, Error> {
        Client::builder()
            .api_key("sk-ant-test-safe")
            .base_url(base_url)?
            .build()
    }

    fn create_params() -> Result<BatchCreateParams, Box<dyn std::error::Error>> {
        let inner = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(128)
            .message(MessageParam::user("Hello"))
            .build()?;
        let request = BatchCreateRequest::new("req-1", inner)?;
        Ok(BatchCreateParams::builder().request(request).build()?)
    }

    fn batch_body(results_url: Option<&str>, status: BatchProcessingStatus) -> String {
        batch_body_with_id("msgbatch_01", results_url, status)
    }

    fn batch_body_with_id(
        batch_id: &str,
        results_url: Option<&str>,
        status: BatchProcessingStatus,
    ) -> String {
        let url_field = results_url
            .map(|value| format!("\"{value}\""))
            .unwrap_or_else(|| "null".to_owned());
        format!(
            r#"{{
                "id": "{batch_id}",
                "type": "message_batch",
                "processing_status": "{status}",
                "request_counts": {{
                    "processing": 0,
                    "succeeded": 1,
                    "errored": 0,
                    "canceled": 0,
                    "expired": 0
                }},
                "created_at": "2026-04-27T00:00:00Z",
                "expires_at": "2026-04-28T00:00:00Z",
                "ended_at": null,
                "archived_at": null,
                "cancel_initiated_at": null,
                "results_url": {url_field}
            }}"#,
            batch_id = batch_id,
            status = status,
            url_field = url_field
        )
    }

    fn deleted_batch_body() -> &'static [u8] {
        br#"{
            "id": "msgbatch_01",
            "type": "message_batch_deleted"
        }"#
    }

    #[tokio::test]
    async fn create_serializes_batch_requests_and_decodes_response()
    -> Result<(), Box<dyn std::error::Error>> {
        let body = batch_body(None, BatchProcessingStatus::InProgress);
        let server = spawn_mock(200, &[("request-id", "req_create")], body.as_bytes())?;
        let client = make_client(server.base_url())?;
        let batch = client.messages().batches().create(create_params()?).await?;

        assert_eq!(batch.id.as_str(), "msgbatch_01");
        assert_eq!(batch.processing_status, BatchProcessingStatus::InProgress);

        let request = server.join()?;
        assert!(request.starts_with("POST /v1/messages/batches HTTP/1.1"));
        assert!(request.contains("x-api-key: sk-ant-test-safe"));
        assert!(request.contains("\"custom_id\":\"req-1\""));
        assert!(request.contains("\"model\":\"claude-sonnet-4-5\""));
        assert!(request.contains("\"max_tokens\":128"));
        Ok(())
    }

    #[tokio::test]
    async fn retrieve_targets_path_with_url_encoded_id() -> Result<(), Box<dyn std::error::Error>> {
        let body = batch_body(None, BatchProcessingStatus::InProgress);
        let server = spawn_mock(200, &[("request-id", "req_get")], body.as_bytes())?;
        let client = make_client(server.base_url())?;
        let _batch = client
            .messages()
            .batches()
            .retrieve("msgbatch with space")
            .await?;

        let request = server.join()?;
        let request_line = request.lines().next().ok_or("no request line")?;
        assert!(
            request_line.starts_with("GET /v1/messages/batches/msgbatch+with+space HTTP/1.1"),
            "unexpected request line: {request_line}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn list_appends_pagination_query_string() -> Result<(), Box<dyn std::error::Error>> {
        let page_body = format!(
            r#"{{
                "data": [{}],
                "first_id": "msgbatch_01",
                "last_id": "msgbatch_01",
                "has_more": true
            }}"#,
            batch_body(None, BatchProcessingStatus::InProgress)
        );
        let server = spawn_mock(200, &[("request-id", "req_list")], page_body.as_bytes())?;
        let client = make_client(server.base_url())?;
        let params = ListParams::builder()
            .limit(10)
            .after_id("msgbatch_00")
            .build()?;
        let page = client.messages().batches().list_with_params(params).await?;

        assert_eq!(page.data.len(), 1);
        assert!(page.has_more);

        let request = server.join()?;
        let request_line = request.lines().next().ok_or("no request line")?;
        assert!(request_line.starts_with("GET /v1/messages/batches?"));
        assert!(request_line.contains("limit=10"));
        assert!(request_line.contains("after_id=msgbatch_00"));
        Ok(())
    }

    #[tokio::test]
    async fn auto_paging_fetches_multiple_batch_pages_with_limit_and_options()
    -> Result<(), Box<dyn std::error::Error>> {
        let first_page = format!(
            r#"{{
                "data": [{}],
                "first_id": "msgbatch_01",
                "last_id": "msgbatch_01",
                "has_more": true
            }}"#,
            batch_body_with_id("msgbatch_01", None, BatchProcessingStatus::InProgress)
        );
        let second_page = format!(
            r#"{{
                "data": [{}],
                "first_id": "msgbatch_02",
                "last_id": "msgbatch_02",
                "has_more": false
            }}"#,
            batch_body_with_id("msgbatch_02", None, BatchProcessingStatus::Ended)
        );
        let server = spawn_sequence(vec![
            (
                200,
                vec![("request-id", "req_batches_page_1")],
                first_page.into_bytes(),
            ),
            (
                200,
                vec![("request-id", "req_batches_page_2")],
                second_page.into_bytes(),
            ),
        ])?;
        let client = make_client(server.base_url())?;
        let params = ListParams::builder().limit(1).build()?;
        let options = RequestOptions::builder()
            .header("x-pagination-test", "batches")
            .build()?;
        let mut stream = client
            .messages()
            .batches()
            .list_auto_paging_with(params, options);

        let first = stream.next().await.ok_or("expected first batch")??;
        let second = stream.next().await.ok_or("expected second batch")??;
        assert_eq!(first.id.as_str(), "msgbatch_01");
        assert_eq!(second.id.as_str(), "msgbatch_02");
        assert!(stream.next().await.is_none());
        assert_eq!(stream.last_request_id(), Some("req_batches_page_2"));

        let requests = server.join()?;
        assert_eq!(requests.len(), 2);
        let first_line = requests[0].lines().next().ok_or("missing first line")?;
        let second_line = requests[1].lines().next().ok_or("missing second line")?;
        assert!(first_line.starts_with("GET /v1/messages/batches?"));
        assert!(first_line.contains("limit=1"));
        assert!(!first_line.contains("after_id="));
        assert!(second_line.starts_with("GET /v1/messages/batches?"));
        assert!(second_line.contains("limit=1"));
        assert!(second_line.contains("after_id=msgbatch_01"));
        assert!(requests[0].contains("x-pagination-test: batches"));
        assert!(requests[1].contains("x-pagination-test: batches"));
        Ok(())
    }

    #[tokio::test]
    async fn batch_auto_paging_propagates_api_errors_with_request_ids()
    -> Result<(), Box<dyn std::error::Error>> {
        let first_page = format!(
            r#"{{
                "data": [{}],
                "first_id": "msgbatch_01",
                "last_id": "msgbatch_01",
                "has_more": true
            }}"#,
            batch_body_with_id("msgbatch_01", None, BatchProcessingStatus::InProgress)
        );
        let error_body = br#"{"error":{"type":"api_error","message":"try later"}}"#;
        let server = spawn_sequence(vec![
            (200, vec![], first_page.into_bytes()),
            (
                500,
                vec![("request-id", "req_batches_internal")],
                error_body.to_vec(),
            ),
        ])?;
        let client = make_client(server.base_url())?;
        let options = RequestOptions::builder().max_retries(0).build()?;
        let mut stream = client
            .messages()
            .batches()
            .list_auto_paging_with(ListParams::new(), options);

        let first = stream.next().await.ok_or("expected first batch")??;
        assert_eq!(first.id.as_str(), "msgbatch_01");

        match stream.next().await {
            Some(Err(Error::Api(api_error))) => {
                assert_eq!(api_error.kind, ApiErrorKind::InternalServer);
                assert_eq!(api_error.request_id(), Some("req_batches_internal"));
                assert_eq!(api_error.message, "try later");
            }
            other => return Err(format!("expected API error, got {other:?}").into()),
        }
        assert!(stream.next().await.is_none());

        let requests = server.join()?;
        assert_eq!(requests.len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn cancel_targets_cancel_subpath() -> Result<(), Box<dyn std::error::Error>> {
        let body = batch_body(None, BatchProcessingStatus::Canceling);
        let server = spawn_mock(200, &[("request-id", "req_cancel")], body.as_bytes())?;
        let client = make_client(server.base_url())?;
        let batch = client.messages().batches().cancel("msgbatch_01").await?;

        assert_eq!(batch.processing_status, BatchProcessingStatus::Canceling);

        let request = server.join()?;
        let request_line = request.lines().next().ok_or("no request line")?;
        assert!(
            request_line.starts_with("POST /v1/messages/batches/msgbatch_01/cancel HTTP/1.1"),
            "unexpected request line: {request_line}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_targets_batch_path_and_decodes_response()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(200, &[("request-id", "req_delete")], deleted_batch_body())?;
        let client = make_client(server.base_url())?;
        let deleted = client.messages().batches().delete("msgbatch_01").await?;

        assert_eq!(deleted.id.as_str(), "msgbatch_01");
        assert_eq!(deleted.object_type, "message_batch_deleted");

        let request = server.join()?;
        let request_line = request.lines().next().ok_or("no request line")?;
        assert!(
            request_line.starts_with("DELETE /v1/messages/batches/msgbatch_01 HTTP/1.1"),
            "unexpected request line: {request_line}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn delete_with_response_returns_request_id() -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(
            200,
            &[("request-id", "req_delete_meta")],
            deleted_batch_body(),
        )?;
        let client = make_client(server.base_url())?;
        let response = client
            .messages()
            .batches()
            .delete_with_response("msgbatch_01", RequestOptions::new())
            .await?;

        assert_eq!(response.request_id(), Some("req_delete_meta"));
        assert_eq!(response.data.id.as_str(), "msgbatch_01");

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn blank_batch_ids_are_rejected_before_request() -> Result<(), Box<dyn std::error::Error>>
    {
        let client = make_client("http://127.0.0.1:9")?;

        for result in [
            client.messages().batches().retrieve(" ").await,
            client.messages().batches().cancel("").await,
        ] {
            assert!(matches!(result, Err(Error::InvalidMessageBatchId { .. })));
        }

        let delete = client.messages().batches().delete("\t").await;
        assert!(matches!(delete, Err(Error::InvalidMessageBatchId { .. })));

        let results = client.messages().batches().results("\n").await;
        assert!(matches!(results, Err(Error::InvalidMessageBatchId { .. })));

        Ok(())
    }

    #[tokio::test]
    async fn delete_api_errors_preserve_request_id() -> Result<(), Box<dyn std::error::Error>> {
        let body = br#"{
            "error": {
                "type": "not_found_error",
                "message": "missing batch"
            }
        }"#;
        let server = spawn_mock(404, &[("request-id", "req_delete_missing")], body)?;
        let client = make_client(server.base_url())?;
        let result = client.messages().batches().delete("msgbatch_missing").await;

        match result {
            Err(Error::Api(api_error)) => {
                assert_eq!(api_error.kind, ApiErrorKind::NotFound);
                assert_eq!(api_error.request_id(), Some("req_delete_missing"));
                assert_eq!(api_error.message, "missing batch");
            }
            other => return Err(format!("expected API error, got {other:?}").into()),
        }

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn results_streams_jsonl_lines_and_handles_chunk_boundaries()
    -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        let base_url = format!("http://{addr}");
        let results_url = format!("{base_url}/v1/messages/batches/msgbatch_01/results");

        let line_one = r#"{"custom_id":"req-1","result":{"type":"succeeded","message":{"id":"msg_01","type":"message","role":"assistant","model":"claude-sonnet-4-5","content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn","stop_sequence":null,"usage":{"input_tokens":1,"output_tokens":1}}}}"#;
        let line_two = r#"{"custom_id":"req-2","result":{"type":"errored","error":{"error":{"type":"invalid_request_error","message":"bad"}}}}"#;
        let line_three = r#"{"custom_id":"req-3","result":{"type":"canceled"}}"#;

        let retrieve_body = batch_body(Some(&results_url), BatchProcessingStatus::Ended);

        let handle = thread::spawn(move || -> std::io::Result<Vec<String>> {
            let mut requests = Vec::new();

            // First connection: GET retrieve
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            requests.push(read_request(&mut stream)?);
            write_response(
                &mut stream,
                200,
                &[("request-id".to_owned(), "req_retrieve".to_owned())],
                retrieve_body.as_bytes(),
            )?;

            // Second connection: GET results, send body in chunks via Transfer-Encoding: chunked
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            requests.push(read_request(&mut stream)?);

            let header = "HTTP/1.1 200 OK\r\nrequest-id: req_results\r\nContent-Type: application/x-ndjson\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n";
            stream.write_all(header.as_bytes())?;

            let part_a = format!("{line_one}\n{}", &line_two[..40]);
            let part_b = format!("{}\n{line_three}\n", &line_two[40..]);

            for chunk in [part_a, part_b] {
                let bytes = chunk.as_bytes();
                stream.write_all(format!("{:x}\r\n", bytes.len()).as_bytes())?;
                stream.write_all(bytes)?;
                stream.write_all(b"\r\n")?;
            }
            stream.write_all(b"0\r\n\r\n")?;
            Ok(requests)
        });

        let client = make_client(&base_url)?;
        let options = RequestOptions::builder()
            .header("x-batch-options", "applied")
            .build()?;
        let mut stream = client
            .messages()
            .batches()
            .results_with("msgbatch_01", options)
            .await?;

        let first = stream.next().await.ok_or("expected first result")??;
        assert_eq!(first.custom_id, "req-1");
        assert!(matches!(first.result, MessageBatchResult::Succeeded { .. }));

        let second = stream.next().await.ok_or("expected second result")??;
        assert_eq!(second.custom_id, "req-2");
        assert!(matches!(second.result, MessageBatchResult::Errored { .. }));

        let third = stream.next().await.ok_or("expected third result")??;
        assert_eq!(third.custom_id, "req-3");
        assert!(matches!(third.result, MessageBatchResult::Canceled));

        assert!(stream.next().await.is_none());
        assert_eq!(stream.request_id(), Some("req_results"));

        let requests = handle
            .join()
            .map_err(|_| std::io::Error::other("mock thread panicked"))??;
        assert_eq!(requests.len(), 2);
        assert!(requests[0].starts_with("GET /v1/messages/batches/msgbatch_01 HTTP/1.1"));
        assert!(requests[1].starts_with("GET /v1/messages/batches/msgbatch_01/results HTTP/1.1"));
        assert!(requests[0].contains("x-batch-options: applied"));
        assert!(requests[1].contains("x-batch-options: applied"));
        assert!(requests[1].contains("accept: application/x-ndjson"));
        Ok(())
    }

    #[tokio::test]
    async fn results_returns_unavailable_when_batch_still_processing()
    -> Result<(), Box<dyn std::error::Error>> {
        let body = batch_body(None, BatchProcessingStatus::InProgress);
        let server = spawn_sequence(vec![(200, vec![], body.into_bytes())])?;
        let client = make_client(server.base_url())?;
        let result = client.messages().batches().results("msgbatch_01").await;

        match result {
            Err(Error::BatchResultsUnavailable {
                batch_id,
                processing_status,
            }) => {
                assert_eq!(batch_id, "msgbatch_01");
                assert_eq!(processing_status, BatchProcessingStatus::InProgress);
            }
            other => {
                return Err(format!("expected BatchResultsUnavailable, got {other:?}").into());
            }
        }

        let _requests = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn results_propagates_malformed_jsonl_line() -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        let base_url = format!("http://{addr}");
        let results_url = format!("{base_url}/v1/messages/batches/msgbatch_01/results");
        let retrieve_body = batch_body(Some(&results_url), BatchProcessingStatus::Ended);

        let handle = thread::spawn(move || -> std::io::Result<()> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let _ = read_request(&mut stream)?;
            write_response(
                &mut stream,
                200,
                &[("request-id".to_owned(), "req_retrieve".to_owned())],
                retrieve_body.as_bytes(),
            )?;

            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let _ = read_request(&mut stream)?;
            write_response(
                &mut stream,
                200,
                &[
                    ("request-id".to_owned(), "req_results".to_owned()),
                    ("content-type".to_owned(), "application/x-ndjson".to_owned()),
                ],
                b"{not json}\n",
            )?;
            Ok(())
        });

        let client = make_client(&base_url)?;
        let mut stream = client.messages().batches().results("msgbatch_01").await?;

        match stream.next().await {
            Some(Err(Error::Json { .. })) => {}
            other => return Err(format!("expected Json error, got {other:?}").into()),
        }
        assert!(stream.next().await.is_none());

        handle
            .join()
            .map_err(|_| std::io::Error::other("mock thread panicked"))??;
        Ok(())
    }
}
