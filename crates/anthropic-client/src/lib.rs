#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Async client for the Anthropic Claude API.
//!
//! `anthropic-client` is the HTTP transport and resource layer of an
//! independent community Rust SDK for Anthropic's Messages API. It re-exports
//! the typed request and response models from [`anthropic_types`], so most
//! applications only need this crate as a dependency.
//!
//! # What this crate provides
//!
//! - A configurable async [`Client`] built on `reqwest` and `tokio` with
//!   API-key redaction, retry policy, and per-request [`RequestOptions`].
//! - The [`Messages`] resource: non-streaming `create`, SSE `stream`, text
//!   stream convenience, final-message accumulation, and `count_tokens`.
//! - The [`Models`] resource and [`Batches`] resource with full lifecycle
//!   (create / retrieve / list / cancel / delete / streamed JSONL `results`).
//! - Typed cursor auto-pagination via [`AutoItemStream`] and [`AutoPageStream`].
//! - Strongly typed errors through [`Error`] and [`ApiError`], with request IDs
//!   preserved on both successful responses ([`ApiResponse`]) and API failures.
//!
//! # Quick start
//!
//! ```no_run
//! use anthropic_client::{Client, ContentBlock, MessageCreateParams, MessageParam, Model};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let client = Client::from_env()?;
//!
//! let params = MessageCreateParams::builder()
//!     .model(Model::ClaudeSonnet4_5)
//!     .max_tokens(1024)
//!     .message(MessageParam::user("Hello, Claude"))
//!     .build()?;
//!
//! let message = client.messages().create(params).await?;
//!
//! for block in message.content {
//!     if let ContentBlock::Text { text, .. } = block {
//!         println!("{text}");
//!     }
//! }
//! # Ok(()) }
//! ```
//!
//! # Configuration
//!
//! [`Client::from_env`] reads `ANTHROPIC_API_KEY` (and optionally
//! `ANTHROPIC_BASE_URL`). For Anthropic-compatible gateways or tests, build a
//! client explicitly with [`Client::builder`] / [`ClientBuilder`].
//!
//! API keys are wrapped in [`ApiKey`], a [`Debug`]-redacted newtype; the SDK
//! never logs prompts, response bodies, or sensitive headers by default.
//!
//! # Cancellation
//!
//! All async methods are cancel-safe via future-drop. There is no separate
//! `Context`; drop the future to cancel the request.

mod batches;
mod client;
mod config;
mod error;
mod messages;
mod models;
mod pagination;
mod request;
mod request_options;
mod response;
mod stream;
mod transport;

pub use batches::{
    BatchResultsStream, Batches, MessageBatchStream, MessageBatchesPage, MessageBatchesPageStream,
};
pub use client::{Client, ClientBuilder};
pub use config::{ApiKey, ApiKeyError, BaseUrl, BaseUrlError, ClientConfig, MaxRetries};
pub use error::{ApiError, ApiErrorKind, Error};
pub use messages::Messages;
pub use models::{ModelInfoStream, ModelInfosPage, ModelInfosPageStream, Models};
pub use pagination::{AutoItemStream, AutoPageStream};
pub use request_options::{RequestOptions, RequestOptionsBuildError, RequestOptionsBuilder};
pub use response::ApiResponse;
pub use stream::{MessageStream, TextStream};

pub use anthropic_types::{
    BatchCreateParams, BatchCreateParamsBuilder, BatchCreateParamsError, BatchCreateRequest,
    BatchCreateRequestError, BatchProcessingStatus, CacheControl, CacheControlTtl, CacheCreation,
    CitationsConfigParam, ClearToolInputs, ContainerId, ContainerIdError, ContentBlock,
    ContentBlockParam, ContentBlockParamCacheControlError, ContentBlockParamConversionError,
    ContentBlockSourceContentBlockParam, ContentBlockSourceContentParam, ContextManagementConfig,
    ContextManagementEdit, ContextManagementTrigger, ContextTokenCount, ContextTokenCountError,
    DeletedMessageBatch, DocumentMediaType, DocumentSourceParam, ImageSourceParam, InferenceGeo,
    InferenceGeoError, InputTokensThreshold, JsonSchema, ListParams, ListParamsBuilder,
    ListParamsError, MaxTokens, McpAuthorizationToken, McpAuthorizationTokenError, McpServer,
    McpServerError, McpServerName, McpServerNameError, McpServerToolConfiguration, McpServerUrl,
    McpServerUrlError, Message, MessageBatch, MessageBatchId, MessageBatchIdError,
    MessageBatchIndividualResponse, MessageBatchRequestCounts, MessageBatchResult,
    MessageCountTokensParams, MessageCountTokensParamsBuilder, MessageCountTokensParamsError,
    MessageCreateParams, MessageCreateParamsBuilder, MessageCreateParamsError, MessageDeltaUsage,
    MessageParam, MessageParamConversionError, MessageStreamEvent, MessageTokensCount, Model,
    ModelInfo, OutputConfig, OutputFormat, Page, RequestId, Role, SearchResultTextBlockParam,
    ServerToolUsage, ServiceTier, StopReason, StructuredOutputError, SystemPrompt,
    SystemPromptBlock, Temperature, TemperatureError, TextCitation, TextCitationParam,
    ThinkingBudgetTokens, ThinkingBudgetTokensError, ThinkingConfig, ThinkingDisplay,
    ThinkingTurnCount, ThinkingTurnCountError, ThinkingTurnsKeep, Tool, ToolChoice,
    ToolInputDecodeError, ToolName, ToolUse, ToolUseCount, ToolUseCountError, ToolUsesKeep, TopK,
    TopKError, TopP, TopPError, Usage, UsageServiceTier,
};

pub use anthropic_types::{
    BashCodeExecutionOutputBlock, BashCodeExecutionOutputBlockType, BashCodeExecutionResultBlock,
    BashCodeExecutionResultBlockType, BashCodeExecutionToolResultContent,
    BashCodeExecutionToolResultError, BashCodeExecutionToolResultErrorType, BashToolName,
    BashToolType, BuiltInToolCommon, CodeExecutionOutputBlock, CodeExecutionOutputBlockType,
    CodeExecutionResultBlock, CodeExecutionResultBlockType, CodeExecutionTool20250522,
    CodeExecutionTool20250825, CodeExecutionTool20260120, CodeExecutionToolName,
    CodeExecutionToolResultContent, CodeExecutionToolResultError, CodeExecutionToolResultErrorType,
    CodeExecutionToolType20250522, CodeExecutionToolType20250825, CodeExecutionToolType20260120,
    ContainerUploadBlock, ContainerUploadBlockParam, ContainerUploadBlockType, CustomTool,
    CustomToolType, DocumentBlock, EncryptedCodeExecutionResultBlock,
    EncryptedCodeExecutionResultBlockType, MemoryTool20250818, MemoryToolName,
    MemoryToolType20250818, ServerToolCaller, TextEditorCodeExecutionCreateResultBlock,
    TextEditorCodeExecutionCreateResultBlockType, TextEditorCodeExecutionStrReplaceResultBlock,
    TextEditorCodeExecutionStrReplaceResultBlockType, TextEditorCodeExecutionToolResultContent,
    TextEditorCodeExecutionToolResultContentParam, TextEditorCodeExecutionToolResultError,
    TextEditorCodeExecutionToolResultErrorType, TextEditorCodeExecutionViewFileType,
    TextEditorCodeExecutionViewResultBlock, TextEditorCodeExecutionViewResultBlockType,
    TextEditorToolName20250124, TextEditorToolName20250429, TextEditorToolName20250728,
    TextEditorToolType20250124, TextEditorToolType20250429, TextEditorToolType20250728,
    ToolBash20250124, ToolCaller, ToolReferenceBlock, ToolReferenceBlockType,
    ToolSearchBm25ToolName, ToolSearchBm25ToolType20251119, ToolSearchRegexToolName,
    ToolSearchRegexToolType20251119, ToolSearchToolBm25_20251119, ToolSearchToolRegex20251119,
    ToolSearchToolResultContent, ToolSearchToolResultContentParam, ToolSearchToolResultError,
    ToolSearchToolResultErrorParam, ToolSearchToolResultErrorType, ToolSearchToolSearchResultBlock,
    ToolSearchToolSearchResultBlockType, ToolTextEditor20250124, ToolTextEditor20250429,
    ToolTextEditor20250728, UserLocation, UserLocationType, WebFetchBlock, WebFetchBlockParam,
    WebFetchBlockType, WebFetchTool20250910, WebFetchTool20260209, WebFetchTool20260309,
    WebFetchToolName, WebFetchToolResultContent, WebFetchToolResultContentParam,
    WebFetchToolResultErrorBlock, WebFetchToolResultErrorBlockType, WebFetchToolType20250910,
    WebFetchToolType20260209, WebFetchToolType20260309, WebSearchResultBlock,
    WebSearchResultBlockType, WebSearchTool20250305, WebSearchTool20260209, WebSearchToolName,
    WebSearchToolResultContent, WebSearchToolResultContentParam, WebSearchToolResultError,
    WebSearchToolResultErrorType, WebSearchToolType20250305, WebSearchToolType20260209,
};
