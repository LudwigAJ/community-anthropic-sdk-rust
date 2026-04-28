#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Typed request and response models for the Anthropic Claude API.
//!
//! `anthropic-types` is the data layer of an independent community Rust SDK
//! for Anthropic's Messages API. It contains serde-driven request and response
//! structs, validating newtypes, and API-compatible enums. The crate has no
//! HTTP dependency; the async client lives in `anthropic-client`, which
//! re-exports every public item here.
//!
//! Most applications should depend on `anthropic-client` and import from there.
//! Use this crate directly if you only need to build or parse Anthropic
//! request/response payloads (for example, in a custom transport).
//!
//! # Highlights
//!
//! - [`MessageCreateParams`] and [`MessageCreateParamsBuilder`] for building
//!   `messages.create` requests with compile-time validation of common
//!   mistakes (empty conversations, conflicting tool choices, etc.).
//! - [`MessageParam`] and [`ContentBlockParam`] for typed turn construction,
//!   plus [`SystemPrompt`] / [`SystemPromptBlock`] for structured system
//!   prompts and cache markers.
//! - [`Message`] and [`ContentBlock`] response types with typed citations,
//!   tool-use replay, thinking blocks, and forward-compatible
//!   [`StopReason`] variants.
//! - [`MessageStreamEvent`] for SSE event modeling.
//! - [`MessageBatch`], [`BatchCreateParams`], and [`MessageBatchResult`] for
//!   the Message Batches API.
//! - [`Model`] for known and forward-compatible model IDs and [`ModelInfo`]
//!   for `models.list` / `models.retrieve` responses.
//! - [`Page`] and [`ListParams`] shared by cursor-paginated endpoints.
//! - Validating newtypes such as
//!   [`MaxTokens`], [`Temperature`], [`TopK`], [`TopP`], [`ToolName`],
//!   [`McpServerName`], [`McpServerUrl`], [`MessageBatchId`], and
//!   [`RequestId`].
//!
//! # Example: build a request
//!
//! ```
//! use anthropic_types::{MessageCreateParams, MessageParam, Model};
//!
//! let params = MessageCreateParams::builder()
//!     .model(Model::ClaudeSonnet4_5)
//!     .max_tokens(1024)
//!     .message(MessageParam::user("Hello, Claude"))
//!     .build()
//!     .expect("valid params");
//!
//! assert_eq!(params.messages.len(), 1);
//! ```
//!
//! # Design principles
//!
//! - Prefer typed structs and enums over [`serde_json::Value`]. Raw JSON is
//!   reserved for genuinely arbitrary data such as JSON Schema, tool inputs,
//!   metadata, and provider-specific blobs.
//! - Optional fields are modeled as `Option<T>` with `skip_serializing_if`,
//!   so omitted fields are never serialized as explicit `null`.
//! - Newtypes validate at construction (`parse, don't validate`) so invalid
//!   API requests are difficult to assemble.

pub mod batch;
mod cache;
pub mod content;
pub mod error;
pub mod message;
pub mod model;
pub mod pagination;
pub mod primitive;
pub mod stream;
pub mod tool;

pub use batch::{
    BatchCreateParams, BatchCreateParamsBuilder, BatchCreateParamsError, BatchCreateRequest,
    BatchCreateRequestError, BatchProcessingStatus, DeletedMessageBatch, MessageBatch,
    MessageBatchId, MessageBatchIdError, MessageBatchIndividualResponse, MessageBatchRequestCounts,
    MessageBatchResult,
};
pub use cache::{CacheControl, CacheControlTtl};
pub use content::{
    CitationsConfigParam, ContentBlock, ContentBlockParam, ContentBlockParamCacheControlError,
    ContentBlockParamConversionError, ContentBlockSourceContentBlockParam,
    ContentBlockSourceContentParam, DocumentMediaType, DocumentSourceParam, ImageSourceParam,
    SearchResultTextBlockParam, TextCitation, TextCitationParam, ToolInputDecodeError,
    ToolResultContent, ToolUse,
};
pub use error::{ApiErrorBody, ApiErrorDetail, ApiErrorType};
pub use message::{
    ClearToolInputs, ContextManagementConfig, ContextManagementEdit, ContextManagementTrigger,
    ContextTokenCount, ContextTokenCountError, InputTokensThreshold, McpServer, McpServerError,
    McpServerToolConfiguration, Message, MessageCountTokensParams, MessageCountTokensParamsBuilder,
    MessageCountTokensParamsError, MessageCreateParams, MessageCreateParamsBuilder,
    MessageCreateParamsError, MessageParam, MessageParamConversionError, MessageTokensCount,
    OutputConfig, OutputFormat, Role, ServiceTier, StopReason, StructuredOutputError, SystemPrompt,
    SystemPromptBlock, ThinkingBudgetTokens, ThinkingBudgetTokensError, ThinkingConfig,
    ThinkingDisplay, ThinkingTurnCount, ThinkingTurnCountError, ThinkingTurnsKeep, ToolUseCount,
    ToolUseCountError, ToolUsesKeep, Usage,
};
pub use model::{Model, ModelInfo};
pub use pagination::{ListParams, ListParamsBuilder, ListParamsError, Page};
pub use primitive::{
    ContainerId, ContainerIdError, InferenceGeo, InferenceGeoError, MaxTokens, MaxTokensError,
    McpAuthorizationToken, McpAuthorizationTokenError, McpServerName, McpServerNameError,
    McpServerUrl, McpServerUrlError, RequestId, RequestIdError, Temperature, TemperatureError,
    ToolName, ToolNameError, TopK, TopKError, TopP, TopPError,
};
pub use stream::{ContentBlockDelta, MessageDelta, MessageStreamEvent};
pub use tool::{JsonSchema, JsonSchemaError, Tool, ToolChoice, ToolError};
