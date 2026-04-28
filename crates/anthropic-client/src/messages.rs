//! Messages API service boundary.
//!
//! Hosts the [`Messages`] resource returned by [`crate::Client::messages`]
//! and every operation that targets `/v1/messages`:
//!
//! - `create` / `create_with` / `create_with_response` — non-streaming
//!   message creation with optional [`crate::RequestOptions`] and
//!   [`crate::ApiResponse`] metadata.
//! - `create_and_parse` — convenience that calls `create` and parses the
//!   structured-output response into a caller-owned `T: DeserializeOwned`.
//! - `create_stream` / `create_streaming_text` — SSE streaming variants
//!   returning [`crate::MessageStream`] and [`crate::TextStream`].
//! - `count_tokens` / `count_tokens_with` / `count_tokens_with_response` —
//!   `/v1/messages/count_tokens`.
//! - `batches()` — nested [`crate::Batches`] sub-resource.
//!
//! Request shapes ([`MessageCreateParams`], [`MessageCountTokensParams`])
//! and response types ([`Message`], [`MessageTokensCount`]) come from
//! [`anthropic_types`] and are re-exported by this crate.

use anthropic_types::{Message, MessageCountTokensParams, MessageCreateParams, MessageTokensCount};
use serde::de::DeserializeOwned;

use crate::{ApiResponse, Batches, Client, Error, MessageStream, RequestOptions, TextStream};

/// Service for the Messages API.
#[derive(Debug, Clone, Copy)]
pub struct Messages<'a> {
    client: &'a Client,
}

impl<'a> Messages<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Returns the Message Batches API service nested under Messages.
    pub fn batches(&self) -> Batches<'a> {
        Batches::new(self.client)
    }

    /// Creates a message.
    ///
    /// Executes a non-streaming Messages API request.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages"))]
    pub async fn create(&self, params: MessageCreateParams) -> Result<Message, Error> {
        self.create_with(params, RequestOptions::new()).await
    }

    /// Creates a message with per-request options.
    ///
    /// Executes a non-streaming Messages API request with per-request options.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages"))]
    pub async fn create_with(
        &self,
        params: MessageCreateParams,
        options: RequestOptions,
    ) -> Result<Message, Error> {
        self.create_with_response(params, options)
            .await
            .map(ApiResponse::into_data)
    }

    /// Creates a message with per-request options and response metadata.
    ///
    /// Executes a non-streaming Messages API request and returns the decoded
    /// message plus metadata such as the `request-id` response header.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages"))]
    pub async fn create_with_response(
        &self,
        params: MessageCreateParams,
        options: RequestOptions,
    ) -> Result<ApiResponse<Message>, Error> {
        let prepared = crate::request::build_messages_create(
            self.client.http_client(),
            self.client.config(),
            &params,
            &options,
        )?;
        crate::transport::execute_json_response(self.client.http_client(), prepared).await
    }

    /// Creates a message and parses its structured JSON output.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages"))]
    pub async fn create_and_parse<T>(&self, params: MessageCreateParams) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        self.create_and_parse_with(params, RequestOptions::new())
            .await
    }

    /// Creates a message with per-request options and parses its structured JSON output.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages"))]
    pub async fn create_and_parse_with<T>(
        &self,
        params: MessageCreateParams,
        options: RequestOptions,
    ) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        self.create_and_parse_with_response(params, options)
            .await
            .map(ApiResponse::into_data)
    }

    /// Creates a message, parses structured JSON output, and returns response metadata.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages"))]
    pub async fn create_and_parse_with_response<T>(
        &self,
        params: MessageCreateParams,
        options: RequestOptions,
    ) -> Result<ApiResponse<T>, Error>
    where
        T: DeserializeOwned,
    {
        let response = self.create_with_response(params, options).await?;
        let parsed = response
            .data
            .parse_json_output()
            .map_err(|source| Error::StructuredOutput { source })?;

        Ok(ApiResponse::new(parsed, response.request_id))
    }

    /// Creates a streaming message response.
    ///
    /// Executes a Messages API request with `stream` forced to `true` and
    /// returns the raw SSE event stream.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages"))]
    pub async fn create_stream(&self, params: MessageCreateParams) -> Result<MessageStream, Error> {
        self.create_stream_with(params, RequestOptions::new()).await
    }

    /// Creates a streaming message response with per-request options.
    ///
    /// Executes a Messages API request with `stream` forced to `true` and
    /// returns the raw SSE event stream.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages"))]
    pub async fn create_stream_with(
        &self,
        params: MessageCreateParams,
        options: RequestOptions,
    ) -> Result<MessageStream, Error> {
        let prepared = crate::request::build_messages_create_stream(
            self.client.http_client(),
            self.client.config(),
            &params,
            &options,
        )?;
        crate::transport::execute_stream_response(self.client.http_client(), prepared).await
    }

    /// Creates a streaming message response as text chunks.
    ///
    /// Executes a Messages API request with `stream` forced to `true` and
    /// returns a stream that yields only appended text deltas.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages"))]
    pub async fn create_streaming_text(
        &self,
        params: MessageCreateParams,
    ) -> Result<TextStream, Error> {
        self.create_streaming_text_with(params, RequestOptions::new())
            .await
    }

    /// Counts the input tokens for a Messages API request.
    ///
    /// Executes a non-streaming `count_tokens` request and returns the decoded
    /// token count.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/count_tokens"))]
    pub async fn count_tokens(
        &self,
        params: MessageCountTokensParams,
    ) -> Result<MessageTokensCount, Error> {
        self.count_tokens_with(params, RequestOptions::new()).await
    }

    /// Counts the input tokens with per-request options.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/count_tokens"))]
    pub async fn count_tokens_with(
        &self,
        params: MessageCountTokensParams,
        options: RequestOptions,
    ) -> Result<MessageTokensCount, Error> {
        self.count_tokens_with_response(params, options)
            .await
            .map(ApiResponse::into_data)
    }

    /// Counts the input tokens with per-request options and response metadata.
    ///
    /// Returns the decoded token count plus metadata such as the `request-id`
    /// response header.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages/count_tokens"))]
    pub async fn count_tokens_with_response(
        &self,
        params: MessageCountTokensParams,
        options: RequestOptions,
    ) -> Result<ApiResponse<MessageTokensCount>, Error> {
        let prepared = crate::request::build_messages_count_tokens(
            self.client.http_client(),
            self.client.config(),
            &params,
            &options,
        )?;
        crate::transport::execute_json_response(self.client.http_client(), prepared).await
    }

    /// Creates a streaming text response with per-request options.
    ///
    /// Executes a Messages API request with `stream` forced to `true` and
    /// returns a stream that yields only appended text deltas.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/messages"))]
    pub async fn create_streaming_text_with(
        &self,
        params: MessageCreateParams,
        options: RequestOptions,
    ) -> Result<TextStream, Error> {
        self.create_stream_with(params, options)
            .await
            .map(TextStream::new)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::{Duration, Instant},
    };

    use futures_util::StreamExt;
    use reqwest::StatusCode;
    use serde::Deserialize;

    use super::*;
    use crate::{ApiErrorKind, Client};
    use anthropic_types::{
        ApiErrorType, ContentBlock, ContentBlockDelta, MessageCountTokensParams, MessageParam,
        MessageStreamEvent, Model, StopReason,
    };

    fn params() -> Result<MessageCreateParams, anthropic_types::MessageCreateParamsError> {
        MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(128)
            .message(MessageParam::user("Hello"))
            .build()
    }

    fn client(base_url: &str) -> Result<Client, Error> {
        Client::builder()
            .api_key("sk-ant-test-safe")
            .base_url(base_url)?
            .build()
    }

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
            let requests = self
                .handle
                .join()
                .map_err(|_| std::io::Error::other("mock server thread panicked"))??;
            Ok(requests)
        }
    }

    enum MockAction {
        Http {
            status: u16,
            headers: Vec<(String, String)>,
            body: String,
        },
        CloseConnection,
    }

    impl MockAction {
        fn http(status: u16, headers: &[(&str, &str)], body: &str) -> Self {
            Self::Http {
                status,
                headers: headers
                    .iter()
                    .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
                    .collect(),
                body: body.to_owned(),
            }
        }
    }

    fn spawn_mock(
        status: u16,
        headers: &[(&str, &str)],
        body: &str,
    ) -> Result<MockServer, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        let base_url = format!("http://{addr}");
        let headers = headers
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect::<Vec<_>>();
        let body = body.to_owned();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let mut request = Vec::new();
            let mut buffer = [0_u8; 8192];

            loop {
                let read = stream.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }

            let mut response = format!(
                "HTTP/1.1 {status} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                reason_phrase(status),
                body.len()
            );
            for (name, value) in headers {
                response.push_str(&name);
                response.push_str(": ");
                response.push_str(&value);
                response.push_str("\r\n");
            }
            response.push_str("\r\n");
            response.push_str(&body);
            stream.write_all(response.as_bytes())?;

            Ok(String::from_utf8_lossy(&request).into_owned())
        });

        Ok(MockServer { base_url, handle })
    }

    fn spawn_sequence(
        actions: Vec<MockAction>,
    ) -> Result<MockSequenceServer, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let addr = listener.local_addr()?;
        let base_url = format!("http://{addr}");

        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            for action in actions {
                let (mut stream, _) = listener.accept()?;
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                requests.push(read_http_request(&mut stream)?);

                match action {
                    MockAction::Http {
                        status,
                        headers,
                        body,
                    } => write_http_response(&mut stream, status, &headers, &body)?,
                    MockAction::CloseConnection => {}
                }
            }

            Ok(requests)
        });

        Ok(MockSequenceServer { base_url, handle })
    }

    fn spawn_single_then_wait_for_optional_retry(
        action: MockAction,
        wait: Duration,
    ) -> Result<MockSequenceServer, Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let base_url = format!("http://{addr}");

        let handle = thread::spawn(move || {
            let mut requests = Vec::new();
            let (mut stream, _) = accept_until(&listener, Instant::now() + Duration::from_secs(5))?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            requests.push(read_http_request(&mut stream)?);
            match action {
                MockAction::Http {
                    status,
                    headers,
                    body,
                } => write_http_response(&mut stream, status, &headers, &body)?,
                MockAction::CloseConnection => {}
            }

            let deadline = Instant::now() + wait;
            if let Some(mut stream) = try_accept_until(&listener, deadline)? {
                stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                requests.push(read_http_request(&mut stream)?);
            }

            Ok(requests)
        });

        Ok(MockSequenceServer { base_url, handle })
    }

    fn accept_until(
        listener: &TcpListener,
        deadline: Instant,
    ) -> std::io::Result<(std::net::TcpStream, std::net::SocketAddr)> {
        loop {
            match listener.accept() {
                Ok(accepted) => return Ok(accepted),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "timed out waiting for mock request",
                        ));
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn try_accept_until(
        listener: &TcpListener,
        deadline: Instant,
    ) -> std::io::Result<Option<std::net::TcpStream>> {
        loop {
            match listener.accept() {
                Ok((stream, _)) => return Ok(Some(stream)),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Ok(None);
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> std::io::Result<String> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 8192];

        loop {
            let read = stream.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..read]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }

        Ok(String::from_utf8_lossy(&request).into_owned())
    }

    fn write_http_response(
        stream: &mut std::net::TcpStream,
        status: u16,
        headers: &[(String, String)],
        body: &str,
    ) -> std::io::Result<()> {
        let mut response = format!(
            "HTTP/1.1 {status} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
            reason_phrase(status),
            body.len()
        );
        for (name, value) in headers {
            response.push_str(name);
            response.push_str(": ");
            response.push_str(value);
            response.push_str("\r\n");
        }
        response.push_str("\r\n");
        response.push_str(body);
        stream.write_all(response.as_bytes())
    }

    fn reason_phrase(status: u16) -> &'static str {
        match status {
            200 => "OK",
            400 => "Bad Request",
            401 => "Unauthorized",
            403 => "Forbidden",
            404 => "Not Found",
            408 => "Request Timeout",
            409 => "Conflict",
            418 => "I'm a Teapot",
            422 => "Unprocessable Entity",
            429 => "Too Many Requests",
            500 => "Internal Server Error",
            529 => "Site Overloaded",
            _ => "Status",
        }
    }

    fn success_body() -> &'static str {
        r#"{
            "id": "msg_01",
            "type": "message",
            "role": "assistant",
            "model": "claude-sonnet-4-5",
            "content": [
                { "type": "text", "text": "Hi there" }
            ],
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {
                "input_tokens": 8,
                "output_tokens": 3
            }
        }"#
    }

    fn structured_output_body() -> &'static str {
        r#"{
            "id": "msg_01",
            "type": "message",
            "role": "assistant",
            "model": "claude-sonnet-4-5",
            "content": [
                { "type": "text", "text": "{\"answer\":4}" }
            ],
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {
                "input_tokens": 8,
                "output_tokens": 3
            }
        }"#
    }

    fn error_body(error_type: &str, message: &str) -> String {
        format!(r#"{{"error":{{"type":"{error_type}","message":"{message}"}}}}"#)
    }

    async fn assert_api_error_mapping(
        status: u16,
        body: &str,
        expected_kind: ApiErrorKind,
        expected_message: &str,
        expected_error_type: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(status, &[("request-id", "req_error")], body)?;
        let client = client(server.base_url())?;
        let options = RequestOptions::builder().max_retries(0).build()?;
        let result = client.messages().create_with(params()?, options).await;

        match result {
            Err(Error::Api(api_error)) => {
                assert_eq!(api_error.status.as_u16(), status);
                assert_eq!(api_error.request_id(), Some("req_error"));
                assert_eq!(api_error.kind, expected_kind);
                assert_eq!(api_error.message, expected_message);
                assert_eq!(
                    api_error
                        .body
                        .as_ref()
                        .map(|body| body.error.error_type.as_str()),
                    expected_error_type
                );
                assert_eq!(api_error.raw_body.as_deref(), Some(body));

                let rendered = format!("{api_error:?}");
                assert!(rendered.contains("ApiError"));
                assert!(rendered.contains("req_error"));
                assert!(rendered.contains("[redacted]"));
                assert!(!rendered.contains(expected_message));
                if !body.is_empty() {
                    assert!(!rendered.contains(body));
                }
            }
            other => {
                return Err(std::io::Error::other(format!("expected Api, got {other:?}")).into());
            }
        }

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn create_decodes_successful_message_response() -> Result<(), Box<dyn std::error::Error>>
    {
        let server = spawn_mock(200, &[("request-id", "req_success")], success_body())?;
        let client = client(server.base_url())?;
        let message = client.messages().create(params()?).await?;

        assert_eq!(message.id, "msg_01");
        assert_eq!(message.model, Model::ClaudeSonnet4_5);
        assert_eq!(message.stop_reason, Some(StopReason::EndTurn));
        assert_eq!(message.usage.input_tokens, 8);
        assert_eq!(message.usage.output_tokens, 3);
        assert_eq!(message.content, vec![ContentBlock::text("Hi there")]);

        let request = server.join()?;
        assert!(request.starts_with("POST /v1/messages HTTP/1.1"));
        assert!(request.contains("x-api-key: sk-ant-test-safe"));
        Ok(())
    }

    #[tokio::test]
    async fn create_with_response_returns_message_and_request_id()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(200, &[("request-id", "req_success")], success_body())?;
        let client = client(server.base_url())?;
        let response = client
            .messages()
            .create_with_response(params()?, RequestOptions::new())
            .await?;

        assert_eq!(response.request_id(), Some("req_success"));
        assert_eq!(response.data.id, "msg_01");
        assert_eq!(response.data.content, vec![ContentBlock::text("Hi there")]);

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn create_with_response_uses_none_for_missing_request_id()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(200, &[], success_body())?;
        let client = client(server.base_url())?;
        let response = client
            .messages()
            .create_with_response(params()?, RequestOptions::new())
            .await?;

        assert_eq!(response.request_id(), None);
        assert_eq!(response.request_id, None);
        assert_eq!(response.data.id, "msg_01");

        let _request = server.join()?;
        Ok(())
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct ParsedAnswer {
        answer: u32,
    }

    #[tokio::test]
    async fn create_and_parse_returns_structured_output_and_request_id()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(
            200,
            &[("request-id", "req_success")],
            structured_output_body(),
        )?;
        let client = client(server.base_url())?;

        let response: ApiResponse<ParsedAnswer> = client
            .messages()
            .create_and_parse_with_response(params()?, RequestOptions::new())
            .await?;

        assert_eq!(response.request_id(), Some("req_success"));
        assert_eq!(response.data, ParsedAnswer { answer: 4 });

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn create_still_returns_plain_message() -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(200, &[("request-id", "req_success")], success_body())?;
        let client = client(server.base_url())?;
        let message: Message = client.messages().create(params()?).await?;

        assert_eq!(message.id, "msg_01");
        assert_eq!(message.content, vec![ContentBlock::text("Hi there")]);

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn create_stream_decodes_sse_events() -> Result<(), Box<dyn std::error::Error>> {
        let body = concat!(
            "event: ping\n",
            "data: {\"type\":\"ping\"}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n",
        );
        let server = spawn_mock(
            200,
            &[
                ("request-id", "req_stream"),
                ("content-type", "text/event-stream"),
            ],
            body,
        )?;
        let client = client(server.base_url())?;
        let mut stream = client.messages().create_stream(params()?).await?;

        assert_eq!(stream.request_id(), Some("req_stream"));
        assert_eq!(
            stream
                .next()
                .await
                .ok_or_else(|| std::io::Error::other("expected ping event"))??,
            MessageStreamEvent::Ping
        );
        assert_eq!(
            stream
                .next()
                .await
                .ok_or_else(|| std::io::Error::other("expected delta event"))??,
            MessageStreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentBlockDelta::Text {
                    text: "Hi".to_owned()
                }
            }
        );
        assert_eq!(
            stream
                .next()
                .await
                .ok_or_else(|| std::io::Error::other("expected stop event"))??,
            MessageStreamEvent::MessageStop
        );
        assert!(stream.next().await.is_none());

        let request = server.join()?;
        assert!(request.starts_with("POST /v1/messages HTTP/1.1"));
        Ok(())
    }

    #[tokio::test]
    async fn create_streaming_text_yields_text_chunks() -> Result<(), Box<dyn std::error::Error>> {
        let body = concat!(
            "event: ping\n",
            "data: {\"type\":\"ping\"}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}\n\n",
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"lo\"}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n",
        );
        let server = spawn_mock(
            200,
            &[
                ("request-id", "req_text_stream"),
                ("content-type", "text/event-stream"),
            ],
            body,
        )?;
        let client = client(server.base_url())?;
        let mut stream = client.messages().create_streaming_text(params()?).await?;

        assert_eq!(stream.request_id(), Some("req_text_stream"));
        assert_eq!(stream.next().await.transpose()?, Some("Hel".to_owned()));
        assert_eq!(stream.next().await.transpose()?, Some("lo".to_owned()));
        assert!(stream.next().await.is_none());

        let request = server.join()?;
        assert!(request.starts_with("POST /v1/messages HTTP/1.1"));
        Ok(())
    }

    #[tokio::test]
    async fn create_streaming_text_with_applies_request_options()
    -> Result<(), Box<dyn std::error::Error>> {
        let body = concat!(
            "event: content_block_delta\n",
            "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n",
            "event: message_stop\n",
            "data: {\"type\":\"message_stop\"}\n\n",
        );
        let server = spawn_mock(
            200,
            &[
                ("request-id", "req_text_stream_options"),
                ("content-type", "text/event-stream"),
            ],
            body,
        )?;
        let client = client(server.base_url())?;
        let options = RequestOptions::builder()
            .header("anthropic-beta", "text-stream-test")
            .max_retries(0)
            .build()?;
        let mut stream = client
            .messages()
            .create_streaming_text_with(params()?, options)
            .await?;

        assert_eq!(stream.next().await.transpose()?, Some("ok".to_owned()));
        assert!(stream.next().await.is_none());

        let request = server.join()?;
        assert!(request.contains("anthropic-beta: text-stream-test"));
        Ok(())
    }

    #[tokio::test]
    async fn create_stream_maps_setup_api_errors() -> Result<(), Box<dyn std::error::Error>> {
        let body = error_body("rate_limit_error", "slow down");
        let server = spawn_mock(429, &[("request-id", "req_stream_setup")], &body)?;
        let client = client(server.base_url())?;
        let options = RequestOptions::builder().max_retries(0).build()?;
        let result = client
            .messages()
            .create_stream_with(params()?, options)
            .await;

        match result {
            Err(Error::Api(api_error)) => {
                assert_eq!(api_error.status, StatusCode::TOO_MANY_REQUESTS);
                assert_eq!(api_error.request_id(), Some("req_stream_setup"));
                assert_eq!(api_error.kind, ApiErrorKind::RateLimit);
            }
            other => {
                return Err(std::io::Error::other(format!("expected Api, got {other:?}")).into());
            }
        }

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn create_stream_maps_sse_error_events() -> Result<(), Box<dyn std::error::Error>> {
        let body = "event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n";
        let server = spawn_mock(
            200,
            &[
                ("request-id", "req_stream_error"),
                ("content-type", "text/event-stream"),
            ],
            body,
        )?;
        let client = client(server.base_url())?;
        let mut stream = client.messages().create_stream(params()?).await?;

        match stream
            .next()
            .await
            .ok_or_else(|| std::io::Error::other("expected stream error"))?
        {
            Err(Error::Api(api_error)) => {
                assert_eq!(api_error.status, StatusCode::OK);
                assert_eq!(api_error.request_id(), Some("req_stream_error"));
                assert_eq!(api_error.kind, ApiErrorKind::Overloaded);
                assert_eq!(api_error.message, "Overloaded");
            }
            other => {
                return Err(std::io::Error::other(format!(
                    "expected API stream error, got {other:?}"
                ))
                .into());
            }
        }
        assert!(stream.next().await.is_none());

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn create_retries_retryable_statuses_until_success()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_sequence(vec![
            MockAction::http(
                429,
                &[("request-id", "req_first"), ("retry-after-ms", "0")],
                &error_body("rate_limit_error", "slow down"),
            ),
            MockAction::http(
                500,
                &[("request-id", "req_second"), ("retry-after-ms", "0")],
                &error_body("api_error", "try again"),
            ),
            MockAction::http(200, &[("request-id", "req_success")], success_body()),
        ])?;
        let client = client(server.base_url())?;
        let message = client.messages().create(params()?).await?;

        assert_eq!(message.id, "msg_01");
        assert_eq!(message.content, vec![ContentBlock::text("Hi there")]);

        let requests = server.join()?;
        assert_eq!(requests.len(), 3);
        assert!(
            requests
                .iter()
                .all(|request| request.starts_with("POST /v1/messages HTTP/1.1"))
        );
        Ok(())
    }

    #[tokio::test]
    async fn create_with_response_returns_final_successful_request_id_after_retry()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_sequence(vec![
            MockAction::http(
                408,
                &[("request-id", "req_timeout"), ("retry-after-ms", "0")],
                &error_body("request_timeout_error", "timed out"),
            ),
            MockAction::http(200, &[("request-id", "req_final")], success_body()),
        ])?;
        let client = client(server.base_url())?;
        let options = RequestOptions::builder().max_retries(1).build()?;
        let response = client
            .messages()
            .create_with_response(params()?, options)
            .await?;

        assert_eq!(response.request_id(), Some("req_final"));
        assert_eq!(response.data.id, "msg_01");

        let requests = server.join()?;
        assert_eq!(requests.len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn max_retries_zero_performs_one_attempt_only() -> Result<(), Box<dyn std::error::Error>>
    {
        let body = error_body("rate_limit_error", "slow down");
        let server = spawn_mock(429, &[("request-id", "req_only")], &body)?;
        let client = client(server.base_url())?;
        let options = RequestOptions::builder().max_retries(0).build()?;
        let result = client.messages().create_with(params()?, options).await;

        match result {
            Err(Error::Api(api_error)) => {
                assert_eq!(api_error.status, StatusCode::TOO_MANY_REQUESTS);
                assert_eq!(api_error.kind, ApiErrorKind::RateLimit);
                assert_eq!(api_error.request_id(), Some("req_only"));
            }
            other => {
                return Err(std::io::Error::other(format!("expected Api, got {other:?}")).into());
            }
        }

        let request = server.join()?;
        assert!(request.starts_with("POST /v1/messages HTTP/1.1"));
        Ok(())
    }

    #[tokio::test]
    async fn final_non_success_after_retries_preserves_final_request_id()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_sequence(vec![
            MockAction::http(
                500,
                &[("request-id", "req_first"), ("retry-after-ms", "0")],
                &error_body("api_error", "try again"),
            ),
            MockAction::http(
                429,
                &[("request-id", "req_final")],
                &error_body("rate_limit_error", "still slow"),
            ),
        ])?;
        let client = client(server.base_url())?;
        let options = RequestOptions::builder().max_retries(1).build()?;
        let result = client.messages().create_with(params()?, options).await;

        match result {
            Err(Error::Api(api_error)) => {
                assert_eq!(api_error.status, StatusCode::TOO_MANY_REQUESTS);
                assert_eq!(api_error.kind, ApiErrorKind::RateLimit);
                assert_eq!(api_error.request_id(), Some("req_final"));
                let body = api_error
                    .body
                    .ok_or_else(|| std::io::Error::other("expected parsed API error body"))?;
                assert_eq!(body.error.error_type, ApiErrorType::RateLimit);
                assert_eq!(body.error.message, "still slow");
            }
            other => {
                return Err(std::io::Error::other(format!("expected Api, got {other:?}")).into());
            }
        }

        let requests = server.join()?;
        assert_eq!(requests.len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn malformed_success_json_is_not_retried() -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_single_then_wait_for_optional_retry(
            MockAction::http(200, &[], "{not json"),
            Duration::from_millis(100),
        )?;
        let client = client(server.base_url())?;
        let result = client.messages().create(params()?).await;

        match result {
            Err(Error::Json { .. }) => {}
            other => {
                return Err(std::io::Error::other(format!("expected Json, got {other:?}")).into());
            }
        }

        let requests = server.join()?;
        assert_eq!(requests.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn create_retries_transport_failure_before_valid_response()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_sequence(vec![
            MockAction::CloseConnection,
            MockAction::http(
                200,
                &[("request-id", "req_after_transport")],
                success_body(),
            ),
        ])?;
        let client = client(server.base_url())?;
        let options = RequestOptions::builder().max_retries(1).build()?;
        let response = client
            .messages()
            .create_with_response(params()?, options)
            .await?;

        assert_eq!(response.request_id(), Some("req_after_transport"));
        assert_eq!(response.data.id, "msg_01");

        let requests = server.join()?;
        assert_eq!(requests.len(), 2);
        Ok(())
    }

    #[tokio::test]
    async fn api_response_debug_redacts_decoded_data() -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(200, &[("request-id", "req_debug")], success_body())?;
        let client = client(server.base_url())?;
        let response = client
            .messages()
            .create_with_response(params()?, RequestOptions::new())
            .await?;

        let rendered = format!("{response:?}");

        assert!(rendered.contains("ApiResponse"));
        assert!(rendered.contains("req_debug"));
        assert!(rendered.contains("[redacted]"));
        assert!(!rendered.contains("Hi there"));
        assert!(!rendered.contains("Hello"));
        assert!(!rendered.contains("sk-ant-test-safe"));

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn create_maps_non_success_status_to_api_error() -> Result<(), Box<dyn std::error::Error>>
    {
        let body = error_body("invalid_request_error", "bad request");
        let server = spawn_mock(400, &[], &body)?;
        let client = client(server.base_url())?;
        let result = client.messages().create(params()?).await;

        match result {
            Err(Error::Api(api_error)) => {
                assert_eq!(api_error.status, StatusCode::BAD_REQUEST);
                assert_eq!(api_error.request_id, None);
                assert_eq!(api_error.kind, ApiErrorKind::InvalidRequest);
                assert_eq!(api_error.message, "bad request");
                let body = api_error
                    .body
                    .ok_or_else(|| std::io::Error::other("expected parsed API error body"))?;
                assert_eq!(body.error.error_type, ApiErrorType::InvalidRequest);
                assert_eq!(body.error.message, "bad request");
            }
            other => {
                return Err(std::io::Error::other(format!("expected Api, got {other:?}")).into());
            }
        }

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn api_error_maps_bad_request() -> Result<(), Box<dyn std::error::Error>> {
        assert_api_error_mapping(
            400,
            &error_body("invalid_request_error", "bad request"),
            ApiErrorKind::InvalidRequest,
            "bad request",
            Some("invalid_request_error"),
        )
        .await
    }

    #[tokio::test]
    async fn api_error_maps_authentication() -> Result<(), Box<dyn std::error::Error>> {
        assert_api_error_mapping(
            401,
            &error_body("authentication_error", "bad api key"),
            ApiErrorKind::Authentication,
            "bad api key",
            Some("authentication_error"),
        )
        .await
    }

    #[tokio::test]
    async fn api_error_maps_permission() -> Result<(), Box<dyn std::error::Error>> {
        assert_api_error_mapping(
            403,
            &error_body("permission_error", "not allowed"),
            ApiErrorKind::Permission,
            "not allowed",
            Some("permission_error"),
        )
        .await
    }

    #[tokio::test]
    async fn api_error_maps_not_found() -> Result<(), Box<dyn std::error::Error>> {
        assert_api_error_mapping(
            404,
            &error_body("not_found_error", "missing"),
            ApiErrorKind::NotFound,
            "missing",
            Some("not_found_error"),
        )
        .await
    }

    #[tokio::test]
    async fn api_error_maps_conflict_from_status() -> Result<(), Box<dyn std::error::Error>> {
        assert_api_error_mapping(
            409,
            "",
            ApiErrorKind::Conflict,
            "HTTP status 409 with empty error body",
            None,
        )
        .await
    }

    #[tokio::test]
    async fn api_error_maps_unprocessable_from_status() -> Result<(), Box<dyn std::error::Error>> {
        assert_api_error_mapping(
            422,
            r#"{"error":{"message":"unprocessable"}}"#,
            ApiErrorKind::UnprocessableEntity,
            "HTTP status 422 with unparseable error body",
            None,
        )
        .await
    }

    #[tokio::test]
    async fn api_error_maps_rate_limit() -> Result<(), Box<dyn std::error::Error>> {
        assert_api_error_mapping(
            429,
            &error_body("rate_limit_error", "slow down"),
            ApiErrorKind::RateLimit,
            "slow down",
            Some("rate_limit_error"),
        )
        .await
    }

    #[tokio::test]
    async fn api_error_maps_internal_server() -> Result<(), Box<dyn std::error::Error>> {
        assert_api_error_mapping(
            500,
            &error_body("api_error", "try again"),
            ApiErrorKind::InternalServer,
            "try again",
            Some("api_error"),
        )
        .await
    }

    #[tokio::test]
    async fn api_error_maps_overloaded() -> Result<(), Box<dyn std::error::Error>> {
        assert_api_error_mapping(
            529,
            &error_body("overloaded_error", "overloaded"),
            ApiErrorKind::Overloaded,
            "overloaded",
            Some("overloaded_error"),
        )
        .await
    }

    #[tokio::test]
    async fn api_error_preserves_unknown_error_type() -> Result<(), Box<dyn std::error::Error>> {
        assert_api_error_mapping(
            418,
            &error_body("teapot_error", "short and stout"),
            ApiErrorKind::Unknown("teapot_error".to_owned()),
            "short and stout",
            Some("teapot_error"),
        )
        .await
    }

    #[tokio::test]
    async fn malformed_error_body_still_returns_api_error() -> Result<(), Box<dyn std::error::Error>>
    {
        assert_api_error_mapping(
            400,
            "{not json",
            ApiErrorKind::InvalidRequest,
            "HTTP status 400 with unparseable error body",
            None,
        )
        .await
    }

    #[tokio::test]
    async fn create_preserves_request_id_on_api_errors() -> Result<(), Box<dyn std::error::Error>> {
        let body = error_body("rate_limit_error", "slow down");
        let server = spawn_mock(429, &[("request-id", "req_123")], &body)?;
        let client = client(server.base_url())?;
        let options = RequestOptions::builder().max_retries(0).build()?;
        let result = client.messages().create_with(params()?, options).await;

        match result {
            Err(Error::Api(api_error)) => {
                assert_eq!(api_error.request_id(), Some("req_123"));
            }
            other => {
                return Err(std::io::Error::other(format!("expected Api, got {other:?}")).into());
            }
        }

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn create_maps_malformed_success_json_to_json_error()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(200, &[], "{not json")?;
        let client = client(server.base_url())?;
        let result = client.messages().create(params()?).await;

        match result {
            Err(Error::Json { .. }) => {}
            other => {
                return Err(std::io::Error::other(format!("expected Json, got {other:?}")).into());
            }
        }

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn create_maps_transport_failure_to_transport_error()
    -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let base_url = format!("http://{}", listener.local_addr()?);
        drop(listener);

        let client = client(&base_url)?;
        let result = client.messages().create(params()?).await;

        match result {
            Err(Error::Transport { .. }) => Ok(()),
            other => {
                Err(std::io::Error::other(format!("expected Transport, got {other:?}")).into())
            }
        }
    }

    fn count_tokens_params() -> Result<MessageCountTokensParams, Box<dyn std::error::Error>> {
        Ok(MessageCountTokensParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .message(MessageParam::user("How many tokens?"))
            .build()?)
    }

    #[tokio::test]
    async fn count_tokens_decodes_input_tokens() -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(
            200,
            &[("request-id", "req_count")],
            r#"{"input_tokens": 42}"#,
        )?;
        let client = client(server.base_url())?;
        let count = client
            .messages()
            .count_tokens(count_tokens_params()?)
            .await?;

        assert_eq!(count.input_tokens, 42);

        let request = server.join()?;
        assert!(request.starts_with("POST /v1/messages/count_tokens HTTP/1.1"));
        assert!(request.contains("x-api-key: sk-ant-test-safe"));
        assert!(request.contains("anthropic-version: 2023-06-01"));
        Ok(())
    }

    #[tokio::test]
    async fn count_tokens_with_response_returns_request_id()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(
            200,
            &[("request-id", "req_count_meta")],
            r#"{"input_tokens": 7}"#,
        )?;
        let client = client(server.base_url())?;
        let response = client
            .messages()
            .count_tokens_with_response(count_tokens_params()?, RequestOptions::new())
            .await?;

        assert_eq!(response.request_id(), Some("req_count_meta"));
        assert_eq!(response.data.input_tokens, 7);

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn count_tokens_maps_api_errors() -> Result<(), Box<dyn std::error::Error>> {
        let body = error_body("rate_limit_error", "slow down");
        let server = spawn_mock(429, &[("request-id", "req_count_err")], &body)?;
        let client = client(server.base_url())?;
        let options = RequestOptions::builder().max_retries(0).build()?;
        let result = client
            .messages()
            .count_tokens_with(count_tokens_params()?, options)
            .await;

        match result {
            Err(Error::Api(api_error)) => {
                assert_eq!(api_error.status, StatusCode::TOO_MANY_REQUESTS);
                assert_eq!(api_error.kind, ApiErrorKind::RateLimit);
                assert_eq!(api_error.request_id(), Some("req_count_err"));
                let body = api_error
                    .body
                    .ok_or_else(|| std::io::Error::other("expected parsed API error body"))?;
                assert_eq!(body.error.error_type, ApiErrorType::RateLimit);
            }
            other => {
                return Err(std::io::Error::other(format!("expected Api, got {other:?}")).into());
            }
        }

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn count_tokens_maps_malformed_success_json() -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(200, &[], "{not json")?;
        let client = client(server.base_url())?;
        let result = client.messages().count_tokens(count_tokens_params()?).await;

        match result {
            Err(Error::Json { .. }) => {}
            other => {
                return Err(std::io::Error::other(format!("expected Json, got {other:?}")).into());
            }
        }

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn count_tokens_with_options_applies_headers_and_retries()
    -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(
            200,
            &[("request-id", "req_count_options")],
            r#"{"input_tokens": 3}"#,
        )?;
        let client = client(server.base_url())?;
        let options = RequestOptions::builder()
            .header("anthropic-beta", "count-tokens-test")
            .max_retries(0)
            .build()?;
        let count = client
            .messages()
            .count_tokens_with(count_tokens_params()?, options)
            .await?;

        assert_eq!(count.input_tokens, 3);

        let request = server.join()?;
        assert!(request.contains("anthropic-beta: count-tokens-test"));
        Ok(())
    }
}
