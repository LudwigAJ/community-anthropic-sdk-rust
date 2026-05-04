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
    BashCodeExecutionOutputBlock, BashCodeExecutionOutputBlockType, BashCodeExecutionResultBlock,
    BashCodeExecutionResultBlockType, BashCodeExecutionToolResultContent,
    BashCodeExecutionToolResultError, BashCodeExecutionToolResultErrorType, CitationsConfigParam,
    CodeExecutionOutputBlock, CodeExecutionOutputBlockType, CodeExecutionResultBlock,
    CodeExecutionResultBlockType, CodeExecutionToolResultContent, CodeExecutionToolResultError,
    CodeExecutionToolResultErrorType, ContainerUploadBlock, ContainerUploadBlockParam,
    ContainerUploadBlockType, ContentBlock, ContentBlockParam, ContentBlockParamCacheControlError,
    ContentBlockParamConversionError, ContentBlockSourceContentBlockParam,
    ContentBlockSourceContentParam, DocumentBlock, DocumentMediaType, DocumentSourceParam,
    EncryptedCodeExecutionResultBlock, EncryptedCodeExecutionResultBlockType, ImageSourceParam,
    SearchResultTextBlockParam, ServerToolCaller, TextCitation, TextCitationParam,
    TextEditorCodeExecutionCreateResultBlock, TextEditorCodeExecutionCreateResultBlockType,
    TextEditorCodeExecutionStrReplaceResultBlock, TextEditorCodeExecutionStrReplaceResultBlockType,
    TextEditorCodeExecutionToolResultContent, TextEditorCodeExecutionToolResultContentParam,
    TextEditorCodeExecutionToolResultError, TextEditorCodeExecutionToolResultErrorType,
    TextEditorCodeExecutionViewFileType, TextEditorCodeExecutionViewResultBlock,
    TextEditorCodeExecutionViewResultBlockType, ToolInputDecodeError, ToolReferenceBlock,
    ToolReferenceBlockType, ToolResultContent, ToolSearchToolResultContent,
    ToolSearchToolResultContentParam, ToolSearchToolResultError, ToolSearchToolResultErrorParam,
    ToolSearchToolResultErrorType, ToolSearchToolSearchResultBlock,
    ToolSearchToolSearchResultBlockType, ToolUse, WebFetchBlock, WebFetchBlockParam,
    WebFetchBlockType, WebFetchToolResultContent, WebFetchToolResultContentParam,
    WebFetchToolResultErrorBlock, WebFetchToolResultErrorBlockType, WebSearchResultBlock,
    WebSearchResultBlockType, WebSearchToolResultContent, WebSearchToolResultContentParam,
    WebSearchToolResultError, WebSearchToolResultErrorType,
};
pub use error::{ApiErrorBody, ApiErrorDetail, ApiErrorType};
pub use message::{
    CacheCreation, ClearToolInputs, ContextManagementConfig, ContextManagementEdit,
    ContextManagementTrigger, ContextTokenCount, ContextTokenCountError, InputTokensThreshold,
    McpServer, McpServerError, McpServerToolConfiguration, Message, MessageCountTokensParams,
    MessageCountTokensParamsBuilder, MessageCountTokensParamsError, MessageCreateParams,
    MessageCreateParamsBuilder, MessageCreateParamsError, MessageParam,
    MessageParamConversionError, MessageTokensCount, OutputConfig, OutputFormat, Role,
    ServerToolUsage, ServiceTier, StopReason, StructuredOutputError, SystemPrompt,
    SystemPromptBlock, ThinkingBudgetTokens, ThinkingBudgetTokensError, ThinkingConfig,
    ThinkingDisplay, ThinkingTurnCount, ThinkingTurnCountError, ThinkingTurnsKeep, ToolUseCount,
    ToolUseCountError, ToolUsesKeep, Usage, UsageServiceTier,
};
pub use model::{Model, ModelInfo};
pub use pagination::{ListParams, ListParamsBuilder, ListParamsError, Page};
pub use primitive::{
    ContainerId, ContainerIdError, InferenceGeo, InferenceGeoError, MaxTokens, MaxTokensError,
    McpAuthorizationToken, McpAuthorizationTokenError, McpServerName, McpServerNameError,
    McpServerUrl, McpServerUrlError, RequestId, RequestIdError, Temperature, TemperatureError,
    ToolName, ToolNameError, TopK, TopKError, TopP, TopPError,
};
pub use stream::{ContentBlockDelta, MessageDelta, MessageDeltaUsage, MessageStreamEvent};
pub use tool::{
    BashToolName, BashToolType, BuiltInToolCommon, CodeExecutionTool20250522,
    CodeExecutionTool20250825, CodeExecutionTool20260120, CodeExecutionToolName,
    CodeExecutionToolType20250522, CodeExecutionToolType20250825, CodeExecutionToolType20260120,
    CustomTool, CustomToolType, JsonSchema, JsonSchemaError, MemoryTool20250818, MemoryToolName,
    MemoryToolType20250818, TextEditorToolName20250124, TextEditorToolName20250429,
    TextEditorToolName20250728, TextEditorToolType20250124, TextEditorToolType20250429,
    TextEditorToolType20250728, Tool, ToolBash20250124, ToolCaller, ToolChoice, ToolError,
    ToolSearchBm25ToolName, ToolSearchBm25ToolType20251119, ToolSearchRegexToolName,
    ToolSearchRegexToolType20251119, ToolSearchToolBm25_20251119, ToolSearchToolRegex20251119,
    ToolTextEditor20250124, ToolTextEditor20250429, ToolTextEditor20250728, UserLocation,
    UserLocationType, WebFetchTool20250910, WebFetchTool20260209, WebFetchTool20260309,
    WebFetchToolName, WebFetchToolType20250910, WebFetchToolType20260209, WebFetchToolType20260309,
    WebSearchTool20250305, WebSearchTool20260209, WebSearchToolName, WebSearchToolType20250305,
    WebSearchToolType20260209,
};
