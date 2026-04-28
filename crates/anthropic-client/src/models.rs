//! Models API service boundary.
//!
//! Hosts the [`Models`] resource returned by [`crate::Client::models`] for
//! every operation under `/v1/models`:
//!
//! - `list` / `list_with_params` / `list_with_response` — single-page reads
//!   returning [`Page<ModelInfo>`] (also re-exported as [`ModelInfosPage`]).
//! - `list_auto_paging_with(_params)` — typed [`crate::AutoItemStream`] of
//!   [`ModelInfo`] driven by the cursor in [`ListParams`].
//! - `list_pages_with` — page stream variant ([`ModelInfosPageStream`])
//!   that yields [`crate::ApiResponse<Page<ModelInfo>>`] so callers can
//!   read the per-page request ID.
//! - `retrieve` / `retrieve_with_response` — single model lookup.
//!
//! [`ModelInfo`] preserves capability metadata as
//! `Option<serde_json::Value>` so newly added flags do not require an SDK
//! release.

use anthropic_types::{ListParams, ModelInfo, Page};
use futures_util::future::BoxFuture;

use crate::{ApiResponse, AutoItemStream, AutoPageStream, Client, Error, RequestOptions};

/// Page of models returned by the Models API.
pub type ModelInfosPage = Page<ModelInfo>;
/// Stream of model pages returned by auto-pagination helpers.
pub type ModelInfosPageStream<'a> = AutoPageStream<'a, ModelInfo>;
/// Stream of model records returned by auto-pagination helpers.
pub type ModelInfoStream<'a> = AutoItemStream<'a, ModelInfo>;

/// Service for the Models API.
#[derive(Debug, Clone, Copy)]
pub struct Models<'a> {
    client: &'a Client,
}

impl<'a> Models<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Lists available models with default pagination.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/models"))]
    pub async fn list(&self) -> Result<ModelInfosPage, Error> {
        self.list_with_params(ListParams::new()).await
    }

    /// Lists available models with explicit pagination parameters.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/models"))]
    pub async fn list_with_params(&self, params: ListParams) -> Result<ModelInfosPage, Error> {
        self.list_with(params, RequestOptions::new()).await
    }

    /// Lists available models with pagination and per-request options.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/models"))]
    pub async fn list_with(
        &self,
        params: ListParams,
        options: RequestOptions,
    ) -> Result<ModelInfosPage, Error> {
        self.list_with_response(params, options)
            .await
            .map(ApiResponse::into_data)
    }

    /// Lists available models with pagination, options, and response metadata.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/models"))]
    pub async fn list_with_response(
        &self,
        params: ListParams,
        options: RequestOptions,
    ) -> Result<ApiResponse<ModelInfosPage>, Error> {
        let prepared = crate::request::build_models_list(
            self.client.http_client(),
            self.client.config(),
            &params,
            &options,
        )?;
        crate::transport::execute_json_response(self.client.http_client(), prepared).await
    }

    /// Returns a stream that auto-paginates model items with default parameters.
    pub fn list_auto_paging(&self) -> ModelInfoStream<'a> {
        self.list_auto_paging_with(ListParams::new(), RequestOptions::new())
    }

    /// Returns a stream that auto-paginates model items with explicit parameters.
    pub fn list_auto_paging_with_params(&self, params: ListParams) -> ModelInfoStream<'a> {
        self.list_auto_paging_with(params, RequestOptions::new())
    }

    /// Returns a stream that auto-paginates model items with parameters and options.
    ///
    /// Request options are applied to every page request.
    pub fn list_auto_paging_with(
        &self,
        params: ListParams,
        options: RequestOptions,
    ) -> ModelInfoStream<'a> {
        AutoItemStream::new(self.list_pages_with(params, options))
    }

    /// Returns a stream that auto-paginates model pages with default parameters.
    pub fn list_pages(&self) -> ModelInfosPageStream<'a> {
        self.list_pages_with(ListParams::new(), RequestOptions::new())
    }

    /// Returns a stream that auto-paginates model pages with explicit parameters.
    pub fn list_pages_with_params(&self, params: ListParams) -> ModelInfosPageStream<'a> {
        self.list_pages_with(params, RequestOptions::new())
    }

    /// Returns a stream that auto-paginates model pages with parameters and options.
    ///
    /// Each yielded page includes response metadata such as `request-id`.
    /// Request options are applied to every page request.
    pub fn list_pages_with(
        &self,
        params: ListParams,
        options: RequestOptions,
    ) -> ModelInfosPageStream<'a> {
        AutoPageStream::new(self.client, params, options, fetch_models_page)
    }

    /// Retrieves a specific model by identifier.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/models/{model}"))]
    pub async fn retrieve(&self, model_id: &str) -> Result<ModelInfo, Error> {
        self.retrieve_with(model_id, RequestOptions::new()).await
    }

    /// Retrieves a specific model with per-request options.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/models/{model}"))]
    pub async fn retrieve_with(
        &self,
        model_id: &str,
        options: RequestOptions,
    ) -> Result<ModelInfo, Error> {
        self.retrieve_with_response(model_id, options)
            .await
            .map(ApiResponse::into_data)
    }

    /// Retrieves a specific model with options and response metadata.
    #[tracing::instrument(skip_all, fields(endpoint = "/v1/models/{model}"))]
    pub async fn retrieve_with_response(
        &self,
        model_id: &str,
        options: RequestOptions,
    ) -> Result<ApiResponse<ModelInfo>, Error> {
        let prepared = crate::request::build_models_retrieve(
            self.client.http_client(),
            self.client.config(),
            model_id,
            &options,
        )?;
        crate::transport::execute_json_response(self.client.http_client(), prepared).await
    }
}

fn fetch_models_page<'a>(
    client: &'a Client,
    params: ListParams,
    options: RequestOptions,
) -> BoxFuture<'a, Result<ApiResponse<ModelInfosPage>, Error>> {
    Box::pin(async move {
        let prepared = crate::request::build_models_list(
            client.http_client(),
            client.config(),
            &params,
            &options,
        )?;
        crate::transport::execute_json_response(client.http_client(), prepared).await
    })
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::Duration,
    };

    use super::*;
    use crate::Client;
    use futures_util::StreamExt;

    struct MockServer {
        base_url: String,
        handle: thread::JoinHandle<std::io::Result<String>>,
    }

    struct MockSequenceServer {
        base_url: String,
        handle: thread::JoinHandle<std::io::Result<Vec<String>>>,
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
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }

        Ok(String::from_utf8_lossy(&request).into_owned())
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
            let request = read_request(&mut stream)?;
            write_response(&mut stream, status, &headers, body.as_bytes())?;

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
                    .map(|(name, value)| (name.to_owned(), value.to_owned()))
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

    fn page_body() -> &'static str {
        r#"{
            "data": [
                {
                    "type": "model",
                    "id": "claude-sonnet-4-5",
                    "display_name": "Claude Sonnet 4.5",
                    "created_at": "2025-09-29T00:00:00Z"
                },
                {
                    "type": "model",
                    "id": "claude-haiku-4-5",
                    "display_name": "Claude Haiku 4.5",
                    "created_at": "2025-10-01T00:00:00Z"
                }
            ],
            "first_id": "claude-sonnet-4-5",
            "last_id": "claude-haiku-4-5",
            "has_more": true
        }"#
    }

    fn single_model_page_body(
        id: &str,
        display_name: &str,
        has_more: bool,
        first_id: &str,
        last_id: &str,
    ) -> String {
        format!(
            r#"{{
                "data": [
                    {{
                        "type": "model",
                        "id": "{id}",
                        "display_name": "{display_name}",
                        "created_at": "2026-01-01T00:00:00Z"
                    }}
                ],
                "first_id": "{first_id}",
                "last_id": "{last_id}",
                "has_more": {has_more}
            }}"#
        )
    }

    #[tokio::test]
    async fn list_decodes_page_and_targets_endpoint() -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(200, &[("request-id", "req_models")], page_body())?;
        let client = make_client(server.base_url())?;
        let page = client.models().list().await?;

        assert_eq!(page.data.len(), 2);
        assert_eq!(page.data[0].id, "claude-sonnet-4-5");
        assert_eq!(page.first_id.as_deref(), Some("claude-sonnet-4-5"));
        assert_eq!(page.last_id.as_deref(), Some("claude-haiku-4-5"));
        assert!(page.has_more);

        let request = server.join()?;
        assert!(request.starts_with("GET /v1/models HTTP/1.1"));
        assert!(request.contains("x-api-key: sk-ant-test-safe"));
        assert!(request.contains("anthropic-version: 2023-06-01"));
        Ok(())
    }

    #[tokio::test]
    async fn list_with_params_appends_query_string() -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(200, &[("request-id", "req_models_query")], page_body())?;
        let client = make_client(server.base_url())?;
        let params = ListParams::builder()
            .limit(20)
            .after_id("claude-sonnet-4-5")
            .build()?;
        let _page = client.models().list_with_params(params).await?;

        let request = server.join()?;
        let request_line = request
            .lines()
            .next()
            .ok_or("request had no lines")?
            .to_owned();
        assert!(request_line.starts_with("GET /v1/models?"));
        assert!(request_line.contains("limit=20"));
        assert!(request_line.contains("after_id=claude-sonnet-4-5"));
        assert!(!request_line.contains("before_id="));
        Ok(())
    }

    #[tokio::test]
    async fn list_with_response_returns_request_id() -> Result<(), Box<dyn std::error::Error>> {
        let server = spawn_mock(200, &[("request-id", "req_models_meta")], page_body())?;
        let client = make_client(server.base_url())?;
        let response = client
            .models()
            .list_with_response(ListParams::new(), RequestOptions::new())
            .await?;

        assert_eq!(response.request_id(), Some("req_models_meta"));
        assert_eq!(response.data.data.len(), 2);
        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn list_pagination_walks_using_next_page_params() -> Result<(), Box<dyn std::error::Error>>
    {
        let first_page_server = spawn_mock(200, &[], page_body())?;
        let client = make_client(first_page_server.base_url())?;
        let page = client.models().list().await?;
        let _first_request = first_page_server.join()?;

        let next_params = page
            .next_page_params(Some(20))
            .ok_or("expected next page params")?;
        assert_eq!(next_params.after_id.as_deref(), Some("claude-haiku-4-5"));
        assert_eq!(next_params.limit, Some(20));

        let second_body = r#"{
            "data": [
                {
                    "type": "model",
                    "id": "claude-opus-4-6",
                    "display_name": "Claude Opus 4.6",
                    "created_at": "2025-11-01T00:00:00Z"
                }
            ],
            "first_id": "claude-opus-4-6",
            "last_id": "claude-opus-4-6",
            "has_more": false
        }"#;
        let second_page_server = spawn_mock(200, &[], second_body)?;
        let client = make_client(second_page_server.base_url())?;
        let second = client.models().list_with_params(next_params).await?;

        assert_eq!(second.data.len(), 1);
        assert!(!second.has_more);
        assert!(second.next_page_params(None).is_none());

        let request = second_page_server.join()?;
        let request_line = request.lines().next().ok_or("no request line")?;
        assert!(request_line.contains("after_id=claude-haiku-4-5"));
        assert!(request_line.contains("limit=20"));
        Ok(())
    }

    #[tokio::test]
    async fn retrieve_targets_path_with_url_encoded_id() -> Result<(), Box<dyn std::error::Error>> {
        let body = r#"{
            "type": "model",
            "id": "vendor/model id",
            "display_name": "Vendor Model",
            "created_at": "2026-01-01T00:00:00Z"
        }"#;
        let server = spawn_mock(200, &[("request-id", "req_model_one")], body)?;
        let client = make_client(server.base_url())?;
        let info = client.models().retrieve("vendor/model id").await?;

        assert_eq!(info.id, "vendor/model id");
        assert_eq!(info.display_name, "Vendor Model");

        let request = server.join()?;
        let request_line = request.lines().next().ok_or("no request line")?;
        assert!(
            request_line.starts_with("GET /v1/models/vendor%2Fmodel+id HTTP/1.1"),
            "unexpected request line: {request_line}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn retrieve_with_response_returns_request_id() -> Result<(), Box<dyn std::error::Error>> {
        let body = r#"{
            "type": "model",
            "id": "claude-sonnet-4-5",
            "display_name": "Claude Sonnet 4.5",
            "created_at": "2025-09-29T00:00:00Z"
        }"#;
        let server = spawn_mock(200, &[("request-id", "req_model_meta")], body)?;
        let client = make_client(server.base_url())?;
        let response = client
            .models()
            .retrieve_with_response("claude-sonnet-4-5", RequestOptions::new())
            .await?;

        assert_eq!(response.request_id(), Some("req_model_meta"));
        assert_eq!(response.data.id, "claude-sonnet-4-5");
        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn auto_paging_fetches_multiple_model_pages_with_limit_and_options()
    -> Result<(), Box<dyn std::error::Error>> {
        let first = single_model_page_body(
            "claude-sonnet-4-5",
            "Claude Sonnet 4.5",
            true,
            "claude-sonnet-4-5",
            "claude-sonnet-4-5",
        );
        let second = single_model_page_body(
            "claude-haiku-4-5",
            "Claude Haiku 4.5",
            false,
            "claude-haiku-4-5",
            "claude-haiku-4-5",
        );
        let server = spawn_sequence(vec![
            (
                200,
                vec![("request-id", "req_models_page_1")],
                first.into_bytes(),
            ),
            (
                200,
                vec![("request-id", "req_models_page_2")],
                second.into_bytes(),
            ),
        ])?;
        let client = make_client(server.base_url())?;
        let params = ListParams::builder().limit(1).build()?;
        let options = RequestOptions::builder()
            .header("x-pagination-test", "models")
            .build()?;
        let mut stream = client.models().list_auto_paging_with(params, options);

        let first = stream.next().await.ok_or("expected first model")??;
        let second = stream.next().await.ok_or("expected second model")??;
        assert_eq!(first.id, "claude-sonnet-4-5");
        assert_eq!(second.id, "claude-haiku-4-5");
        assert!(stream.next().await.is_none());
        assert_eq!(stream.last_request_id(), Some("req_models_page_2"));

        let requests = server.join()?;
        assert_eq!(requests.len(), 2);
        let first_line = requests[0].lines().next().ok_or("missing first line")?;
        let second_line = requests[1].lines().next().ok_or("missing second line")?;
        assert!(first_line.starts_with("GET /v1/models?"));
        assert!(first_line.contains("limit=1"));
        assert!(!first_line.contains("after_id="));
        assert!(second_line.starts_with("GET /v1/models?"));
        assert!(second_line.contains("limit=1"));
        assert!(second_line.contains("after_id=claude-sonnet-4-5"));
        assert!(requests[0].contains("x-pagination-test: models"));
        assert!(requests[1].contains("x-pagination-test: models"));
        Ok(())
    }

    #[tokio::test]
    async fn page_auto_paging_preserves_per_page_response_metadata()
    -> Result<(), Box<dyn std::error::Error>> {
        let first = single_model_page_body(
            "claude-sonnet-4-5",
            "Claude Sonnet 4.5",
            false,
            "claude-sonnet-4-5",
            "claude-sonnet-4-5",
        );
        let server = spawn_sequence(vec![(
            200,
            vec![("request-id", "req_models_page_meta")],
            first.into_bytes(),
        )])?;
        let client = make_client(server.base_url())?;
        let mut pages = client.models().list_pages();

        let page = pages.next().await.ok_or("expected page")??;
        assert_eq!(page.request_id(), Some("req_models_page_meta"));
        assert_eq!(page.data.data.len(), 1);
        assert!(pages.next().await.is_none());

        let _requests = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn retrieve_maps_api_errors() -> Result<(), Box<dyn std::error::Error>> {
        let body = r#"{"error":{"type":"not_found_error","message":"missing"}}"#;
        let server = spawn_mock(404, &[("request-id", "req_missing")], body)?;
        let client = make_client(server.base_url())?;
        let options = RequestOptions::builder().max_retries(0).build()?;
        let result = client
            .models()
            .retrieve_with("claude-unknown", options)
            .await;

        match result {
            Err(Error::Api(api_error)) => {
                assert_eq!(api_error.status.as_u16(), 404);
                assert_eq!(api_error.kind, crate::ApiErrorKind::NotFound);
                assert_eq!(api_error.request_id(), Some("req_missing"));
            }
            other => {
                return Err(std::io::Error::other(format!("expected Api, got {other:?}")).into());
            }
        }

        let _request = server.join()?;
        Ok(())
    }

    #[tokio::test]
    async fn auto_paging_propagates_model_api_errors_with_request_ids()
    -> Result<(), Box<dyn std::error::Error>> {
        let first = single_model_page_body(
            "claude-sonnet-4-5",
            "Claude Sonnet 4.5",
            true,
            "claude-sonnet-4-5",
            "claude-sonnet-4-5",
        );
        let error_body = br#"{"error":{"type":"rate_limit_error","message":"slow down"}}"#;
        let server = spawn_sequence(vec![
            (200, vec![], first.into_bytes()),
            (
                429,
                vec![("request-id", "req_models_rate_limited")],
                error_body.to_vec(),
            ),
        ])?;
        let client = make_client(server.base_url())?;
        let options = RequestOptions::builder().max_retries(0).build()?;
        let mut stream = client
            .models()
            .list_auto_paging_with(ListParams::new(), options);

        let first = stream.next().await.ok_or("expected first model")??;
        assert_eq!(first.id, "claude-sonnet-4-5");

        match stream.next().await {
            Some(Err(Error::Api(api_error))) => {
                assert_eq!(api_error.kind, crate::ApiErrorKind::RateLimit);
                assert_eq!(api_error.request_id(), Some("req_models_rate_limited"));
                assert_eq!(api_error.message, "slow down");
            }
            other => return Err(format!("expected API error, got {other:?}").into()),
        }
        assert!(stream.next().await.is_none());

        let requests = server.join()?;
        assert_eq!(requests.len(), 2);
        Ok(())
    }
}
