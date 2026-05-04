//! Request and response models for the Messages API.
//!
//! This is the largest module in the crate. It owns:
//!
//! - [`MessageCreateParams`] and its builder, the typed request body for
//!   `POST /v1/messages`. Required fields (`model`, `max_tokens`, at least
//!   one message) are enforced by the builder; optional fields are
//!   `Option<T>` with `skip_serializing_if` so they are omitted from the
//!   wire unless set.
//! - [`MessageCountTokensParams`] and its builder for
//!   `POST /v1/messages/count_tokens`, sharing system / message / tool
//!   shapes with `MessageCreateParams`.
//! - [`MessageParam`] (request turns), [`Role`], and [`SystemPrompt`] /
//!   [`SystemPromptBlock`] for plain or structured system prompts with
//!   cache markers.
//! - Top-level request knobs: [`ServiceTier`], [`ThinkingConfig`] (with
//!   [`ThinkingBudgetTokens`] / [`ThinkingDisplay`]),
//!   [`ContextManagementConfig`] / [`ContextManagementEdit`] /
//!   [`ContextManagementTrigger`], [`McpServer`] /
//!   [`McpServerToolConfiguration`], and the related validating newtypes
//!   ([`ContextTokenCount`], [`ToolUseCount`], [`ThinkingTurnCount`],
//!   [`ThinkingTurnsKeep`], [`ToolUsesKeep`], [`InputTokensThreshold`]).
//! - The response shape: [`Message`], [`StopReason`] (forward-compatible
//!   via `Other(String)`), [`Usage`], [`OutputConfig`] / [`OutputFormat`],
//!   and [`StructuredOutputError`] for `parse_json_output<T>()`.

use std::fmt;

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, DeserializeOwned, Visitor},
};
use thiserror::Error;

use crate::{
    CacheControl, CacheControlTtl, ContainerId, ContainerIdError, ContentBlock, ContentBlockParam,
    ContentBlockParamConversionError, InferenceGeo, InferenceGeoError, MaxTokens, MaxTokensError,
    McpAuthorizationToken, McpAuthorizationTokenError, McpServerName, McpServerNameError,
    McpServerUrl, McpServerUrlError, Model, Temperature, TemperatureError, Tool, ToolChoice,
    ToolName, ToolUse, TopK, TopKError, TopP, TopPError,
    tool::{JsonSchema, JsonSchemaError},
};

/// Parameters for counting input tokens for a Messages API request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageCountTokensParams {
    /// Model identifier whose tokenizer should be used.
    pub model: Model,
    /// Conversation messages whose tokens should be counted.
    pub messages: Vec<MessageParam>,
    /// Optional system prompt counted toward the input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemPrompt>,
    /// Top-level cache control applied to the last cacheable block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
    /// Tool definitions counted toward the input.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool>,
    /// Tool choice counted toward the input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
}

impl MessageCountTokensParams {
    /// Creates a builder for token-counting parameters.
    pub fn builder() -> MessageCountTokensParamsBuilder {
        MessageCountTokensParamsBuilder::default()
    }

    /// Creates minimal token-counting parameters.
    pub fn new(
        model: impl Into<Model>,
        messages: Vec<MessageParam>,
    ) -> Result<Self, MessageCountTokensParamsError> {
        if messages.is_empty() {
            return Err(MessageCountTokensParamsError::EmptyMessages);
        }

        Ok(Self {
            model: model.into(),
            messages,
            system: None,
            cache_control: None,
            tools: Vec::new(),
            tool_choice: None,
        })
    }
}

/// Builder for [`MessageCountTokensParams`].
#[derive(Debug, Default, Clone)]
pub struct MessageCountTokensParamsBuilder {
    model: Option<Model>,
    messages: Vec<MessageParam>,
    system: Option<SystemPrompt>,
    cache_control: Option<CacheControl>,
    tools: Vec<Tool>,
    tool_choice: Option<ToolChoice>,
}

impl MessageCountTokensParamsBuilder {
    /// Sets the model.
    pub fn model(mut self, model: impl Into<Model>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Adds one input message.
    pub fn message(mut self, message: MessageParam) -> Self {
        self.messages.push(message);
        self
    }

    /// Extends the input messages.
    pub fn messages(mut self, messages: impl IntoIterator<Item = MessageParam>) -> Self {
        self.messages.extend(messages);
        self
    }

    /// Sets the system prompt.
    pub fn system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(SystemPrompt::text(system));
        self
    }

    /// Sets the system prompt from a typed system-prompt value.
    pub fn system_prompt(mut self, system: SystemPrompt) -> Self {
        self.system = Some(system);
        self
    }

    /// Adds one structured text block to the system prompt.
    pub fn system_block(mut self, block: SystemPromptBlock) -> Self {
        self.system = Some(append_system_block(self.system, block));
        self
    }

    /// Sets the system prompt to structured text blocks.
    pub fn system_blocks(mut self, blocks: impl IntoIterator<Item = SystemPromptBlock>) -> Self {
        self.system = Some(SystemPrompt::blocks(blocks));
        self
    }

    /// Sets top-level cache control.
    pub fn cache_control(mut self, cache_control: CacheControl) -> Self {
        self.cache_control = Some(cache_control);
        self
    }

    /// Sets ephemeral top-level cache control using the API default TTL.
    pub fn cache_control_ephemeral(mut self) -> Self {
        self.cache_control = Some(CacheControl::ephemeral());
        self
    }

    /// Sets ephemeral top-level cache control with an explicit TTL.
    pub fn cache_control_ephemeral_with_ttl(mut self, ttl: CacheControlTtl) -> Self {
        self.cache_control = Some(CacheControl::ephemeral_with_ttl(ttl));
        self
    }

    /// Adds one tool definition.
    pub fn tool(mut self, tool: Tool) -> Self {
        self.tools.push(tool);
        self
    }

    /// Extends the tool definitions.
    pub fn tools(mut self, tools: impl IntoIterator<Item = Tool>) -> Self {
        self.tools.extend(tools);
        self
    }

    /// Sets the tool choice.
    pub fn tool_choice(mut self, tool_choice: ToolChoice) -> Self {
        self.tool_choice = Some(tool_choice);
        self
    }

    /// Builds validated token-counting parameters.
    pub fn build(self) -> Result<MessageCountTokensParams, MessageCountTokensParamsError> {
        let model = self
            .model
            .ok_or(MessageCountTokensParamsError::MissingModel)?;
        if self.messages.is_empty() {
            return Err(MessageCountTokensParamsError::EmptyMessages);
        }

        Ok(MessageCountTokensParams {
            model,
            messages: self.messages,
            system: self.system,
            cache_control: self.cache_control,
            tools: self.tools,
            tool_choice: self.tool_choice,
        })
    }
}

/// Errors produced while building [`MessageCountTokensParams`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MessageCountTokensParamsError {
    /// A model is required.
    #[error("count tokens params require a model")]
    MissingModel,
    /// At least one input message is required.
    #[error("count tokens params require at least one message")]
    EmptyMessages,
}

/// Token count returned by the count-tokens endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageTokensCount {
    /// Total input tokens across messages, system prompt, and tools.
    pub input_tokens: u32,
}

/// The role associated with a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// End-user message content.
    User,
    /// Assistant message content.
    Assistant,
}

/// Top-level system prompt supplied to a Messages API request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SystemPrompt {
    /// Plain string system prompt.
    Text(String),
    /// Structured text blocks, each of which may carry cache-control metadata.
    Blocks(Vec<SystemPromptBlock>),
}

impl SystemPrompt {
    /// Creates a plain string system prompt.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text(text.into())
    }

    /// Creates a structured system prompt from text blocks.
    pub fn blocks(blocks: impl IntoIterator<Item = SystemPromptBlock>) -> Self {
        Self::Blocks(blocks.into_iter().collect())
    }
}

impl From<String> for SystemPrompt {
    fn from(value: String) -> Self {
        Self::text(value)
    }
}

impl From<&str> for SystemPrompt {
    fn from(value: &str) -> Self {
        Self::text(value)
    }
}

/// Text block accepted in a structured system prompt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SystemPromptBlock {
    /// Plain text system prompt block.
    Text {
        /// The UTF-8 text payload.
        text: String,
        /// Optional cache control marker for this system prompt block.
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<CacheControl>,
    },
}

impl SystemPromptBlock {
    /// Creates a text system prompt block.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text {
            text: text.into(),
            cache_control: None,
        }
    }

    /// Creates a text system prompt block with a cache-control marker.
    pub fn text_with_cache_control(text: impl Into<String>, cache_control: CacheControl) -> Self {
        Self::Text {
            text: text.into(),
            cache_control: Some(cache_control),
        }
    }

    /// Sets cache control on this system prompt block.
    pub fn with_cache_control(self, cache_control: CacheControl) -> Self {
        match self {
            Self::Text { text, .. } => Self::Text {
                text,
                cache_control: Some(cache_control),
            },
        }
    }
}

fn append_system_block(system: Option<SystemPrompt>, block: SystemPromptBlock) -> SystemPrompt {
    match system {
        Some(SystemPrompt::Blocks(mut blocks)) => {
            blocks.push(block);
            SystemPrompt::Blocks(blocks)
        }
        Some(SystemPrompt::Text(text)) => {
            SystemPrompt::Blocks(vec![SystemPromptBlock::text(text), block])
        }
        None => SystemPrompt::Blocks(vec![block]),
    }
}

/// Parameters for creating a message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageCreateParams {
    /// Model identifier.
    pub model: Model,
    /// Maximum number of tokens to generate.
    pub max_tokens: MaxTokens,
    /// Conversation messages supplied to the model.
    pub messages: Vec<MessageParam>,
    /// Optional system prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<SystemPrompt>,
    /// Top-level cache control applied to the last cacheable block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
    /// Container ID to reuse across requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container: Option<ContainerId>,
    /// Geographic region for inference processing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inference_geo: Option<InferenceGeo>,
    /// Context management edits to apply to this request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_management: Option<ContextManagementConfig>,
    /// Whether the API should stream response events.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// Request-scoped MCP servers available to the model.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mcp_servers: Vec<McpServer>,
    /// Caller metadata. Use arbitrary JSON only because the API accepts it here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Custom stop sequences.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop_sequences: Vec<String>,
    /// Tool definitions available to the model.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool>,
    /// How the model should use supplied tools.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    /// Structured-output configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_config: Option<OutputConfig>,
    /// Configuration for extended thinking output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingConfig>,
    /// Service tier preference for this request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<ServiceTier>,
    /// Deprecated temperature sampling value retained for compatibility requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<Temperature>,
    /// Deprecated top-k sampling value retained for compatibility requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<TopK>,
    /// Deprecated top-p sampling value retained for compatibility requests.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<TopP>,
}

impl MessageCreateParams {
    /// Creates a builder for message creation parameters.
    pub fn builder() -> MessageCreateParamsBuilder {
        MessageCreateParamsBuilder::default()
    }

    /// Creates minimal message creation parameters.
    pub fn new(
        model: impl Into<Model>,
        max_tokens: u32,
        messages: Vec<MessageParam>,
    ) -> Result<Self, MessageCreateParamsError> {
        let max_tokens =
            MaxTokens::try_new(max_tokens).map_err(MessageCreateParamsError::MaxTokens)?;
        if messages.is_empty() {
            return Err(MessageCreateParamsError::EmptyMessages);
        }

        Ok(Self {
            model: model.into(),
            max_tokens,
            messages,
            system: None,
            cache_control: None,
            container: None,
            inference_geo: None,
            context_management: None,
            stream: None,
            mcp_servers: Vec::new(),
            metadata: None,
            stop_sequences: Vec::new(),
            tools: Vec::new(),
            tool_choice: None,
            output_config: None,
            thinking: None,
            service_tier: None,
            temperature: None,
            top_k: None,
            top_p: None,
        })
    }
}

/// Output configuration for a message request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputConfig {
    /// Output format requested from the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<OutputFormat>,
}

impl OutputConfig {
    /// Creates output configuration that requests JSON matching a schema.
    pub fn json_schema(schema: JsonSchema) -> Self {
        Self {
            format: Some(OutputFormat::json_schema(schema)),
        }
    }
}

/// Structured-output format requested from the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputFormat {
    /// JSON output constrained by a JSON Schema object.
    JsonSchema {
        /// JSON Schema describing the expected model output.
        schema: JsonSchema,
    },
}

impl OutputFormat {
    /// Creates a JSON Schema output format.
    pub fn json_schema(schema: JsonSchema) -> Self {
        Self::JsonSchema { schema }
    }
}

/// Service tier preference for a message request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceTier {
    /// Let the API choose the appropriate tier.
    Auto,
    /// Use only the standard service tier.
    StandardOnly,
}

/// Context management configuration for a message request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextManagementConfig {
    /// Context management edits to apply before generation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edits: Vec<ContextManagementEdit>,
}

impl ContextManagementConfig {
    /// Creates context management configuration from edits.
    pub fn new(edits: Vec<ContextManagementEdit>) -> Self {
        Self { edits }
    }

    /// Creates an empty context management configuration.
    pub fn empty() -> Self {
        Self { edits: Vec::new() }
    }

    /// Adds one context management edit.
    pub fn edit(mut self, edit: ContextManagementEdit) -> Self {
        self.edits.push(edit);
        self
    }
}

/// Context management edit applied to a message request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContextManagementEdit {
    /// Clear older tool uses according to configured retention rules.
    #[serde(rename = "clear_tool_uses_20250919")]
    ClearToolUses {
        /// Minimum tokens that must be cleared before this edit applies.
        #[serde(skip_serializing_if = "Option::is_none")]
        clear_at_least: Option<InputTokensThreshold>,
        /// Whether to clear all tool inputs or only selected tool inputs.
        #[serde(skip_serializing_if = "Option::is_none")]
        clear_tool_inputs: Option<ClearToolInputs>,
        /// Tool names whose uses are excluded from clearing.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        exclude_tools: Vec<ToolName>,
        /// Number of tool uses to retain.
        #[serde(skip_serializing_if = "Option::is_none")]
        keep: Option<ToolUsesKeep>,
        /// Condition that triggers the edit.
        #[serde(skip_serializing_if = "Option::is_none")]
        trigger: Option<ContextManagementTrigger>,
    },
    /// Clear older extended-thinking blocks according to configured retention rules.
    #[serde(rename = "clear_thinking_20251015")]
    ClearThinking {
        /// Number of most recent assistant turns whose thinking should be retained.
        #[serde(skip_serializing_if = "Option::is_none")]
        keep: Option<ThinkingTurnsKeep>,
    },
    /// Compact older context when the configured trigger threshold is reached.
    #[serde(rename = "compact_20260112")]
    Compact {
        /// Additional instructions for summarization.
        #[serde(skip_serializing_if = "Option::is_none")]
        instructions: Option<String>,
        /// Whether to pause after compaction and return the compaction block.
        #[serde(skip_serializing_if = "Option::is_none")]
        pause_after_compaction: Option<bool>,
        /// Input-token threshold that triggers compaction.
        #[serde(skip_serializing_if = "Option::is_none")]
        trigger: Option<InputTokensThreshold>,
    },
}

impl ContextManagementEdit {
    /// Creates a tool-use clearing edit with no optional settings.
    pub fn clear_tool_uses() -> Self {
        Self::ClearToolUses {
            clear_at_least: None,
            clear_tool_inputs: None,
            exclude_tools: Vec::new(),
            keep: None,
            trigger: None,
        }
    }

    /// Creates a tool-use clearing edit that retains a number of tool uses.
    pub fn clear_tool_uses_keep(keep: ToolUsesKeep) -> Self {
        Self::ClearToolUses {
            clear_at_least: None,
            clear_tool_inputs: None,
            exclude_tools: Vec::new(),
            keep: Some(keep),
            trigger: None,
        }
    }

    /// Creates a thinking clearing edit with no optional settings.
    pub fn clear_thinking() -> Self {
        Self::ClearThinking { keep: None }
    }

    /// Creates a thinking clearing edit that retains a number of thinking turns.
    pub fn clear_thinking_keep(keep: ThinkingTurnsKeep) -> Self {
        Self::ClearThinking { keep: Some(keep) }
    }

    /// Creates a compaction edit.
    pub fn compact() -> Self {
        Self::Compact {
            instructions: None,
            pause_after_compaction: None,
            trigger: None,
        }
    }
}

/// Tool inputs selected for clearing by a context management edit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClearToolInputs {
    /// Clear or preserve all tool inputs.
    All(bool),
    /// Clear only the named tool inputs.
    Tools(Vec<ToolName>),
}

/// Input-token threshold used by context management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputTokensThreshold {
    /// Input-token threshold with a positive value.
    InputTokens {
        /// Positive input-token threshold.
        value: ContextTokenCount,
    },
}

impl InputTokensThreshold {
    /// Creates an input-token threshold.
    pub fn input_tokens(value: u32) -> Result<Self, ContextTokenCountError> {
        Ok(Self::InputTokens {
            value: ContextTokenCount::try_new(value)?,
        })
    }
}

/// Context management trigger condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContextManagementTrigger {
    /// Trigger after a positive number of input tokens.
    InputTokens {
        /// Positive input-token threshold.
        value: ContextTokenCount,
    },
    /// Trigger after a positive number of tool uses.
    ToolUses {
        /// Positive tool-use threshold.
        value: ToolUseCount,
    },
}

impl ContextManagementTrigger {
    /// Creates an input-token trigger.
    pub fn input_tokens(value: u32) -> Result<Self, ContextTokenCountError> {
        Ok(Self::InputTokens {
            value: ContextTokenCount::try_new(value)?,
        })
    }

    /// Creates a tool-use trigger.
    pub fn tool_uses(value: u32) -> Result<Self, ToolUseCountError> {
        Ok(Self::ToolUses {
            value: ToolUseCount::try_new(value)?,
        })
    }
}

/// Number of tool uses retained by a context management edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolUsesKeep {
    /// Retain a positive number of tool uses.
    ToolUses {
        /// Positive tool-use count.
        value: ToolUseCount,
    },
}

impl ToolUsesKeep {
    /// Creates a tool-use retention setting.
    pub fn tool_uses(value: u32) -> Result<Self, ToolUseCountError> {
        Ok(Self::ToolUses {
            value: ToolUseCount::try_new(value)?,
        })
    }
}

/// Number of thinking turns retained by a context management edit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThinkingTurnsKeep {
    /// Retain a positive number of thinking turns.
    ThinkingTurns {
        /// Positive thinking-turn count.
        value: ThinkingTurnCount,
    },
    /// Retain all thinking turns.
    All,
}

impl ThinkingTurnsKeep {
    /// Creates a thinking-turn retention setting.
    pub fn thinking_turns(value: u32) -> Result<Self, ThinkingTurnCountError> {
        Ok(Self::ThinkingTurns {
            value: ThinkingTurnCount::try_new(value)?,
        })
    }

    /// Retains all thinking turns.
    pub fn all() -> Self {
        Self::All
    }
}

/// Positive input-token count for context management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContextTokenCount(u32);

impl ContextTokenCount {
    /// Creates a positive context input-token count.
    pub fn try_new(value: u32) -> Result<Self, ContextTokenCountError> {
        if value == 0 {
            return Err(ContextTokenCountError::Zero);
        }

        Ok(Self(value))
    }

    /// Returns the raw token count.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl Serialize for ContextTokenCount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for ContextTokenCount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Self::try_new(value).map_err(de::Error::custom)
    }
}

/// Errors produced while constructing [`ContextTokenCount`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ContextTokenCountError {
    /// Context token counts must be greater than zero.
    #[error("context management token counts must be greater than zero")]
    Zero,
}

/// Positive tool-use count for context management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ToolUseCount(u32);

impl ToolUseCount {
    /// Creates a positive tool-use count.
    pub fn try_new(value: u32) -> Result<Self, ToolUseCountError> {
        if value == 0 {
            return Err(ToolUseCountError::Zero);
        }

        Ok(Self(value))
    }

    /// Returns the raw tool-use count.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl Serialize for ToolUseCount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for ToolUseCount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Self::try_new(value).map_err(de::Error::custom)
    }
}

/// Errors produced while constructing [`ToolUseCount`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ToolUseCountError {
    /// Tool-use counts must be greater than zero.
    #[error("tool-use counts must be greater than zero")]
    Zero,
}

/// Positive thinking-turn count for context management.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThinkingTurnCount(u32);

impl ThinkingTurnCount {
    /// Creates a positive thinking-turn count.
    pub fn try_new(value: u32) -> Result<Self, ThinkingTurnCountError> {
        if value == 0 {
            return Err(ThinkingTurnCountError::Zero);
        }

        Ok(Self(value))
    }

    /// Returns the raw thinking-turn count.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl Serialize for ThinkingTurnCount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for ThinkingTurnCount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Self::try_new(value).map_err(de::Error::custom)
    }
}

/// Errors produced while constructing [`ThinkingTurnCount`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ThinkingTurnCountError {
    /// Thinking-turn counts must be greater than zero.
    #[error("thinking-turn counts must be greater than zero")]
    Zero,
}

/// Request-scoped MCP server definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpServer {
    /// URL-backed MCP server.
    Url {
        /// Server name.
        name: McpServerName,
        /// Server URL.
        url: McpServerUrl,
        /// Optional authorization token sent to the MCP server.
        #[serde(skip_serializing_if = "Option::is_none")]
        authorization_token: Option<McpAuthorizationToken>,
        /// Optional tool filtering and enablement configuration.
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_configuration: Option<McpServerToolConfiguration>,
    },
}

impl McpServer {
    /// Creates a URL-backed MCP server definition.
    pub fn url(
        name: impl TryInto<McpServerName, Error = McpServerNameError>,
        url: impl TryInto<McpServerUrl, Error = McpServerUrlError>,
    ) -> Result<Self, McpServerError> {
        Ok(Self::Url {
            name: name.try_into().map_err(McpServerError::Name)?,
            url: url.try_into().map_err(McpServerError::Url)?,
            authorization_token: None,
            tool_configuration: None,
        })
    }

    /// Sets an authorization token on a URL-backed MCP server.
    pub fn authorization_token(
        mut self,
        authorization_token: impl TryInto<McpAuthorizationToken, Error = McpAuthorizationTokenError>,
    ) -> Result<Self, McpServerError> {
        match &mut self {
            Self::Url {
                authorization_token: value,
                ..
            } => {
                *value = Some(
                    authorization_token
                        .try_into()
                        .map_err(McpServerError::AuthorizationToken)?,
                );
            }
        }

        Ok(self)
    }

    /// Sets tool configuration on an MCP server.
    pub fn tool_configuration(mut self, tool_configuration: McpServerToolConfiguration) -> Self {
        match &mut self {
            Self::Url {
                tool_configuration: value,
                ..
            } => *value = Some(tool_configuration),
        }

        self
    }
}

/// Tool filtering and enablement configuration for an MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpServerToolConfiguration {
    /// Names of tools allowed from this MCP server.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_tools: Vec<ToolName>,
    /// Whether tools from this MCP server are enabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

impl McpServerToolConfiguration {
    /// Creates empty MCP server tool configuration.
    pub fn new() -> Self {
        Self {
            allowed_tools: Vec::new(),
            enabled: None,
        }
    }

    /// Adds one allowed tool name.
    pub fn allowed_tool(mut self, tool: ToolName) -> Self {
        self.allowed_tools.push(tool);
        self
    }

    /// Sets whether tools from this MCP server are enabled.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = Some(enabled);
        self
    }
}

impl Default for McpServerToolConfiguration {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors produced while constructing MCP server definitions.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum McpServerError {
    /// The MCP server name was invalid.
    #[error(transparent)]
    Name(#[from] McpServerNameError),
    /// The MCP server URL was invalid.
    #[error(transparent)]
    Url(#[from] McpServerUrlError),
    /// The MCP authorization token was invalid.
    #[error(transparent)]
    AuthorizationToken(#[from] McpAuthorizationTokenError),
}

/// Configuration for extended thinking output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThinkingConfig {
    /// Enable thinking with an explicit token budget.
    Enabled {
        /// Token budget for internal reasoning.
        budget_tokens: ThinkingBudgetTokens,
        /// Controls how thinking content appears in responses.
        #[serde(skip_serializing_if = "Option::is_none")]
        display: Option<ThinkingDisplay>,
    },
    /// Disable thinking explicitly.
    Disabled,
    /// Let the API choose an adaptive thinking budget.
    Adaptive {
        /// Controls how thinking content appears in responses.
        #[serde(skip_serializing_if = "Option::is_none")]
        display: Option<ThinkingDisplay>,
    },
}

impl ThinkingConfig {
    /// Creates an enabled thinking configuration.
    pub fn enabled(budget_tokens: u32) -> Result<Self, ThinkingBudgetTokensError> {
        Ok(Self::Enabled {
            budget_tokens: ThinkingBudgetTokens::try_new(budget_tokens)?,
            display: None,
        })
    }

    /// Creates an enabled thinking configuration with a display preference.
    pub fn enabled_with_display(
        budget_tokens: u32,
        display: ThinkingDisplay,
    ) -> Result<Self, ThinkingBudgetTokensError> {
        Ok(Self::Enabled {
            budget_tokens: ThinkingBudgetTokens::try_new(budget_tokens)?,
            display: Some(display),
        })
    }

    /// Creates a disabled thinking configuration.
    pub fn disabled() -> Self {
        Self::Disabled
    }

    /// Creates an adaptive thinking configuration.
    pub fn adaptive() -> Self {
        Self::Adaptive { display: None }
    }

    /// Creates an adaptive thinking configuration with a display preference.
    pub fn adaptive_with_display(display: ThinkingDisplay) -> Self {
        Self::Adaptive {
            display: Some(display),
        }
    }

    fn enabled_budget_tokens(&self) -> Option<ThinkingBudgetTokens> {
        match self {
            Self::Enabled { budget_tokens, .. } => Some(*budget_tokens),
            Self::Disabled | Self::Adaptive { .. } => None,
        }
    }
}

/// Controls how thinking content appears in a response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingDisplay {
    /// Return thinking content normally.
    Summarized,
    /// Redact thinking content while preserving signatures for continuity.
    Omitted,
}

/// Token budget for enabled thinking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThinkingBudgetTokens(u32);

impl ThinkingBudgetTokens {
    /// Minimum accepted thinking budget.
    pub const MIN: u32 = 1024;

    /// Creates a thinking token budget.
    pub fn try_new(value: u32) -> Result<Self, ThinkingBudgetTokensError> {
        if value < Self::MIN {
            return Err(ThinkingBudgetTokensError::TooSmall {
                min: Self::MIN,
                actual: value,
            });
        }

        Ok(Self(value))
    }

    /// Returns the raw token count.
    pub fn get(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for ThinkingBudgetTokens {
    type Error = ThinkingBudgetTokensError;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<ThinkingBudgetTokens> for u32 {
    fn from(value: ThinkingBudgetTokens) -> Self {
        value.get()
    }
}

impl Serialize for ThinkingBudgetTokens {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for ThinkingBudgetTokens {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u32::deserialize(deserializer)?;
        Self::try_new(value).map_err(de::Error::custom)
    }
}

/// Errors produced while constructing [`ThinkingBudgetTokens`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ThinkingBudgetTokensError {
    /// Thinking budgets must be at least [`ThinkingBudgetTokens::MIN`].
    #[error("thinking budget_tokens must be at least {min}; got {actual}")]
    TooSmall {
        /// Minimum accepted budget.
        min: u32,
        /// Caller-provided budget.
        actual: u32,
    },
}

/// Builder for [`MessageCreateParams`].
#[derive(Debug, Default, Clone)]
pub struct MessageCreateParamsBuilder {
    model: Option<Model>,
    max_tokens: Option<u32>,
    messages: Vec<MessageParam>,
    system: Option<SystemPrompt>,
    cache_control: Option<CacheControl>,
    container: Option<ContainerId>,
    inference_geo: Option<InferenceGeo>,
    context_management: Option<ContextManagementConfig>,
    stream: Option<bool>,
    mcp_servers: Vec<McpServer>,
    metadata: Option<serde_json::Value>,
    stop_sequences: Vec<String>,
    tools: Vec<Tool>,
    tool_choice: Option<ToolChoice>,
    output_config: Option<OutputConfig>,
    thinking: Option<ThinkingConfig>,
    service_tier: Option<ServiceTier>,
    temperature: Option<Temperature>,
    top_k: Option<TopK>,
    top_p: Option<TopP>,
}

impl MessageCreateParamsBuilder {
    /// Sets the model.
    pub fn model(mut self, model: impl Into<Model>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Sets the maximum number of tokens to generate.
    pub fn max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Adds one input message.
    pub fn message(mut self, message: MessageParam) -> Self {
        self.messages.push(message);
        self
    }

    /// Extends the input messages.
    pub fn messages(mut self, messages: impl IntoIterator<Item = MessageParam>) -> Self {
        self.messages.extend(messages);
        self
    }

    /// Sets the system prompt.
    pub fn system(mut self, system: impl Into<String>) -> Self {
        self.system = Some(SystemPrompt::text(system));
        self
    }

    /// Sets the system prompt from a typed system-prompt value.
    pub fn system_prompt(mut self, system: SystemPrompt) -> Self {
        self.system = Some(system);
        self
    }

    /// Adds one structured text block to the system prompt.
    pub fn system_block(mut self, block: SystemPromptBlock) -> Self {
        self.system = Some(append_system_block(self.system, block));
        self
    }

    /// Sets the system prompt to structured text blocks.
    pub fn system_blocks(mut self, blocks: impl IntoIterator<Item = SystemPromptBlock>) -> Self {
        self.system = Some(SystemPrompt::blocks(blocks));
        self
    }

    /// Sets top-level cache control.
    pub fn cache_control(mut self, cache_control: CacheControl) -> Self {
        self.cache_control = Some(cache_control);
        self
    }

    /// Sets ephemeral top-level cache control using the API default TTL.
    pub fn cache_control_ephemeral(mut self) -> Self {
        self.cache_control = Some(CacheControl::ephemeral());
        self
    }

    /// Sets ephemeral top-level cache control with an explicit TTL.
    pub fn cache_control_ephemeral_with_ttl(mut self, ttl: CacheControlTtl) -> Self {
        self.cache_control = Some(CacheControl::ephemeral_with_ttl(ttl));
        self
    }

    /// Sets the reusable container ID.
    pub fn container(
        mut self,
        container: impl TryInto<ContainerId, Error = ContainerIdError>,
    ) -> Result<Self, MessageCreateParamsError> {
        self.container = Some(
            container
                .try_into()
                .map_err(MessageCreateParamsError::ContainerId)?,
        );
        Ok(self)
    }

    /// Sets the inference processing region.
    pub fn inference_geo(
        mut self,
        inference_geo: impl TryInto<InferenceGeo, Error = InferenceGeoError>,
    ) -> Result<Self, MessageCreateParamsError> {
        self.inference_geo = Some(
            inference_geo
                .try_into()
                .map_err(MessageCreateParamsError::InferenceGeo)?,
        );
        Ok(self)
    }

    /// Sets context management configuration.
    pub fn context_management(mut self, context_management: ContextManagementConfig) -> Self {
        self.context_management = Some(context_management);
        self
    }

    /// Sets whether response streaming should be requested.
    pub fn stream(mut self, stream: bool) -> Self {
        self.stream = Some(stream);
        self
    }

    /// Adds one request-scoped MCP server.
    pub fn mcp_server(mut self, mcp_server: McpServer) -> Self {
        self.mcp_servers.push(mcp_server);
        self
    }

    /// Adds one URL-backed request-scoped MCP server.
    pub fn mcp_server_url(
        mut self,
        name: impl TryInto<McpServerName, Error = McpServerNameError>,
        url: impl TryInto<McpServerUrl, Error = McpServerUrlError>,
    ) -> Result<Self, MessageCreateParamsError> {
        self.mcp_servers
            .push(McpServer::url(name, url).map_err(MessageCreateParamsError::McpServer)?);
        Ok(self)
    }

    /// Extends request-scoped MCP servers.
    pub fn mcp_servers(mut self, mcp_servers: impl IntoIterator<Item = McpServer>) -> Self {
        self.mcp_servers.extend(mcp_servers);
        self
    }

    /// Sets arbitrary caller metadata accepted by the API.
    pub fn metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Adds one stop sequence.
    pub fn stop_sequence(mut self, stop_sequence: impl Into<String>) -> Self {
        self.stop_sequences.push(stop_sequence.into());
        self
    }

    /// Extends the stop sequences.
    pub fn stop_sequences(mut self, stop_sequences: impl IntoIterator<Item = String>) -> Self {
        self.stop_sequences.extend(stop_sequences);
        self
    }

    /// Adds one tool definition.
    pub fn tool(mut self, tool: Tool) -> Self {
        self.tools.push(tool);
        self
    }

    /// Extends the tool definitions.
    pub fn tools(mut self, tools: impl IntoIterator<Item = Tool>) -> Self {
        self.tools.extend(tools);
        self
    }

    /// Sets the tool choice.
    pub fn tool_choice(mut self, tool_choice: ToolChoice) -> Self {
        self.tool_choice = Some(tool_choice);
        self
    }

    /// Sets structured-output configuration.
    pub fn output_config(mut self, output_config: OutputConfig) -> Self {
        self.output_config = Some(output_config);
        self
    }

    /// Sets extended-thinking configuration.
    pub fn thinking(mut self, thinking: ThinkingConfig) -> Self {
        self.thinking = Some(thinking);
        self
    }

    /// Enables extended thinking with the provided token budget.
    pub fn thinking_enabled(
        mut self,
        budget_tokens: u32,
    ) -> Result<Self, MessageCreateParamsError> {
        self.thinking = Some(
            ThinkingConfig::enabled(budget_tokens)
                .map_err(MessageCreateParamsError::ThinkingBudget)?,
        );
        Ok(self)
    }

    /// Enables extended thinking with a token budget and display preference.
    pub fn thinking_enabled_with_display(
        mut self,
        budget_tokens: u32,
        display: ThinkingDisplay,
    ) -> Result<Self, MessageCreateParamsError> {
        self.thinking = Some(
            ThinkingConfig::enabled_with_display(budget_tokens, display)
                .map_err(MessageCreateParamsError::ThinkingBudget)?,
        );
        Ok(self)
    }

    /// Disables extended thinking explicitly.
    pub fn thinking_disabled(mut self) -> Self {
        self.thinking = Some(ThinkingConfig::disabled());
        self
    }

    /// Requests adaptive extended thinking.
    pub fn thinking_adaptive(mut self) -> Self {
        self.thinking = Some(ThinkingConfig::adaptive());
        self
    }

    /// Requests adaptive extended thinking with a display preference.
    pub fn thinking_adaptive_with_display(mut self, display: ThinkingDisplay) -> Self {
        self.thinking = Some(ThinkingConfig::adaptive_with_display(display));
        self
    }

    /// Sets the service tier preference.
    pub fn service_tier(mut self, service_tier: ServiceTier) -> Self {
        self.service_tier = Some(service_tier);
        self
    }

    /// Sets deprecated temperature sampling for compatibility requests.
    pub fn temperature(mut self, temperature: f64) -> Result<Self, MessageCreateParamsError> {
        self.temperature =
            Some(Temperature::try_new(temperature).map_err(MessageCreateParamsError::Temperature)?);
        Ok(self)
    }

    /// Sets deprecated top-k sampling for compatibility requests.
    pub fn top_k(mut self, top_k: u32) -> Result<Self, MessageCreateParamsError> {
        self.top_k = Some(TopK::try_new(top_k).map_err(MessageCreateParamsError::TopK)?);
        Ok(self)
    }

    /// Sets deprecated top-p sampling for compatibility requests.
    pub fn top_p(mut self, top_p: f64) -> Result<Self, MessageCreateParamsError> {
        self.top_p = Some(TopP::try_new(top_p).map_err(MessageCreateParamsError::TopP)?);
        Ok(self)
    }

    /// Requests JSON output matching the provided JSON Schema object.
    pub fn output_json_schema(
        mut self,
        schema: serde_json::Value,
    ) -> Result<Self, MessageCreateParamsError> {
        let schema =
            JsonSchema::from_value(schema).map_err(MessageCreateParamsError::OutputSchema)?;
        self.output_config = Some(OutputConfig::json_schema(schema));
        Ok(self)
    }

    /// Builds validated message creation parameters.
    pub fn build(self) -> Result<MessageCreateParams, MessageCreateParamsError> {
        let model = self.model.ok_or(MessageCreateParamsError::MissingModel)?;
        let max_tokens = self
            .max_tokens
            .ok_or(MessageCreateParamsError::MissingMaxTokens)
            .and_then(|value| {
                MaxTokens::try_new(value).map_err(MessageCreateParamsError::MaxTokens)
            })?;
        if self.messages.is_empty() {
            return Err(MessageCreateParamsError::EmptyMessages);
        }
        validate_thinking_budget_against_max_tokens(self.thinking.as_ref(), max_tokens)?;

        Ok(MessageCreateParams {
            model,
            max_tokens,
            messages: self.messages,
            system: self.system,
            cache_control: self.cache_control,
            container: self.container,
            inference_geo: self.inference_geo,
            context_management: self.context_management,
            stream: self.stream,
            mcp_servers: self.mcp_servers,
            metadata: self.metadata,
            stop_sequences: self.stop_sequences,
            tools: self.tools,
            tool_choice: self.tool_choice,
            output_config: self.output_config,
            thinking: self.thinking,
            service_tier: self.service_tier,
            temperature: self.temperature,
            top_k: self.top_k,
            top_p: self.top_p,
        })
    }
}

fn validate_thinking_budget_against_max_tokens(
    thinking: Option<&ThinkingConfig>,
    max_tokens: MaxTokens,
) -> Result<(), MessageCreateParamsError> {
    let Some(budget_tokens) = thinking.and_then(ThinkingConfig::enabled_budget_tokens) else {
        return Ok(());
    };

    if budget_tokens.get() >= max_tokens.get() {
        return Err(
            MessageCreateParamsError::ThinkingBudgetNotLessThanMaxTokens {
                budget_tokens: budget_tokens.get(),
                max_tokens: max_tokens.get(),
            },
        );
    }

    Ok(())
}

/// Errors produced while building [`MessageCreateParams`].
#[derive(Debug, Clone, PartialEq, Error)]
pub enum MessageCreateParamsError {
    /// A model is required.
    #[error("message create params require a model")]
    MissingModel,
    /// A max token value is required.
    #[error("message create params require max_tokens")]
    MissingMaxTokens,
    /// Max token validation failed.
    #[error(transparent)]
    MaxTokens(MaxTokensError),
    /// At least one input message is required.
    #[error("message create params require at least one message")]
    EmptyMessages,
    /// The structured-output JSON Schema was invalid.
    #[error(transparent)]
    OutputSchema(#[from] JsonSchemaError),
    /// The container ID was invalid.
    #[error(transparent)]
    ContainerId(#[from] ContainerIdError),
    /// The inference geo was invalid.
    #[error(transparent)]
    InferenceGeo(#[from] InferenceGeoError),
    /// An MCP server definition was invalid.
    #[error(transparent)]
    McpServer(#[from] McpServerError),
    /// Temperature validation failed.
    #[error(transparent)]
    Temperature(#[from] TemperatureError),
    /// Top-k validation failed.
    #[error(transparent)]
    TopK(#[from] TopKError),
    /// Top-p validation failed.
    #[error(transparent)]
    TopP(#[from] TopPError),
    /// The thinking token budget was invalid.
    #[error(transparent)]
    ThinkingBudget(#[from] ThinkingBudgetTokensError),
    /// Enabled thinking budget must be less than `max_tokens`.
    #[error("thinking budget_tokens ({budget_tokens}) must be less than max_tokens ({max_tokens})")]
    ThinkingBudgetNotLessThanMaxTokens {
        /// Enabled thinking budget.
        budget_tokens: u32,
        /// Request max token limit.
        max_tokens: u32,
    },
}

/// A message creation input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageParam {
    /// Role for this message.
    pub role: Role,
    /// Structured message content.
    pub content: Vec<ContentBlockParam>,
}

impl MessageParam {
    /// Creates a user message containing a single text block.
    pub fn user(text: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlockParam::text(text)],
        }
    }

    /// Creates an assistant message containing a single text block.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: vec![ContentBlockParam::text(text)],
        }
    }

    /// Creates a user message from structured content blocks.
    pub fn user_blocks(content: Vec<ContentBlockParam>) -> Self {
        Self {
            role: Role::User,
            content,
        }
    }

    /// Creates an assistant message from structured content blocks.
    pub fn assistant_blocks(content: Vec<ContentBlockParam>) -> Self {
        Self {
            role: Role::Assistant,
            content,
        }
    }

    /// Creates a user message containing a single text tool-result block.
    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::user_blocks(vec![ContentBlockParam::tool_result(tool_use_id, content)])
    }

    /// Creates a user message containing a single text tool-result error block.
    pub fn tool_result_error(tool_use_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self::user_blocks(vec![ContentBlockParam::tool_result_error(
            tool_use_id,
            content,
        )])
    }

    /// Creates a user message containing a single structured tool-result block.
    pub fn tool_result_blocks(
        tool_use_id: impl Into<String>,
        content: Vec<ContentBlockParam>,
    ) -> Self {
        Self::user_blocks(vec![ContentBlockParam::tool_result_blocks(
            tool_use_id,
            content,
        )])
    }

    /// Creates a user message containing a single structured tool-result block
    /// with explicit error status.
    pub fn tool_result_blocks_with_error(
        tool_use_id: impl Into<String>,
        content: Vec<ContentBlockParam>,
        is_error: bool,
    ) -> Self {
        Self::user_blocks(vec![ContentBlockParam::tool_result_blocks_with_error(
            tool_use_id,
            content,
            is_error,
        )])
    }
}

impl TryFrom<Message> for MessageParam {
    type Error = MessageParamConversionError;

    fn try_from(value: Message) -> Result<Self, Self::Error> {
        let content = value
            .content
            .into_iter()
            .enumerate()
            .map(|(index, block)| {
                ContentBlockParam::try_from(block)
                    .map_err(|source| MessageParamConversionError::ContentBlock { index, source })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            role: value.role,
            content,
        })
    }
}

impl TryFrom<&Message> for MessageParam {
    type Error = MessageParamConversionError;

    fn try_from(value: &Message) -> Result<Self, Self::Error> {
        let content = value
            .content
            .iter()
            .enumerate()
            .map(|(index, block)| {
                ContentBlockParam::try_from(block)
                    .map_err(|source| MessageParamConversionError::ContentBlock { index, source })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            role: value.role,
            content,
        })
    }
}

/// Errors produced while converting response messages into request messages.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MessageParamConversionError {
    /// One content block could not be converted.
    #[error("message content block at index {index} cannot be converted")]
    ContentBlock {
        /// Index of the content block in the response message.
        index: usize,
        /// The block conversion failure.
        #[source]
        source: ContentBlockParamConversionError,
    },
}

/// A message returned by the API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    /// Unique message identifier.
    pub id: String,
    /// Object type.
    #[serde(rename = "type")]
    pub object_type: String,
    /// Role for the returned message.
    pub role: Role,
    /// Model that generated the response.
    pub model: Model,
    /// Returned content blocks.
    pub content: Vec<ContentBlock>,
    /// Reason generation stopped.
    pub stop_reason: Option<StopReason>,
    /// Stop sequence that ended generation, when one matched.
    pub stop_sequence: Option<String>,
    /// Token usage information.
    pub usage: Usage,
}

impl Message {
    /// Converts this response message into a request message for conversation replay.
    ///
    /// The conversion preserves the role and all safely replayable content blocks.
    /// It returns an error if any response block has no conservative request-side
    /// equivalent.
    pub fn to_param(&self) -> Result<MessageParam, MessageParamConversionError> {
        MessageParam::try_from(self)
    }

    /// Iterates over returned tool-use blocks in content order.
    pub fn tool_uses(&self) -> impl Iterator<Item = ToolUse<'_>> + '_ {
        self.content.iter().filter_map(ContentBlock::as_tool_use)
    }

    /// Finds the first returned tool-use block with the given identifier.
    pub fn find_tool_use(&self, id: impl AsRef<str>) -> Option<ToolUse<'_>> {
        let id = id.as_ref();
        self.tool_uses().find(|tool_use| tool_use.id() == id)
    }

    /// Parses a structured JSON output response into a caller-provided type.
    ///
    /// This helper accepts exactly one text content block and rejects mixed or
    /// non-text response layouts so callers do not accidentally parse an
    /// ambiguous response.
    pub fn parse_json_output<T>(&self) -> Result<T, StructuredOutputError>
    where
        T: DeserializeOwned,
    {
        let text = self.json_output_text()?;
        let value = serde_json::from_str::<serde_json::Value>(text)
            .map_err(|source| StructuredOutputError::InvalidJson { source })?;

        serde_json::from_value(value)
            .map_err(|source| StructuredOutputError::Deserialize { source })
    }

    fn json_output_text(&self) -> Result<&str, StructuredOutputError> {
        let mut text = None;

        for (index, block) in self.content.iter().enumerate() {
            match block {
                ContentBlock::Text {
                    text: block_text, ..
                } => {
                    if text.is_some() {
                        return Err(StructuredOutputError::MultipleTextBlocks);
                    }
                    text = Some(block_text.as_str());
                }
                other => {
                    return Err(StructuredOutputError::UnsupportedContentBlock {
                        index,
                        block_type: content_block_type(other),
                    });
                }
            }
        }

        text.ok_or(StructuredOutputError::MissingTextOutput)
    }
}

impl ToolUse<'_> {
    /// Creates a user message containing a text tool-result block answering this tool use.
    pub fn tool_result_message(&self, content: impl Into<String>) -> MessageParam {
        MessageParam::tool_result(self.id(), content)
    }

    /// Creates a user message containing a text error tool-result block answering this tool use.
    pub fn tool_result_error_message(&self, content: impl Into<String>) -> MessageParam {
        MessageParam::tool_result_error(self.id(), content)
    }

    /// Creates a user message containing a structured tool-result block answering this tool use.
    pub fn tool_result_blocks_message(&self, content: Vec<ContentBlockParam>) -> MessageParam {
        MessageParam::tool_result_blocks(self.id(), content)
    }

    /// Creates a user message containing a structured tool-result block answering
    /// this tool use with explicit error status.
    pub fn tool_result_blocks_with_error_message(
        &self,
        content: Vec<ContentBlockParam>,
        is_error: bool,
    ) -> MessageParam {
        MessageParam::tool_result_blocks_with_error(self.id(), content, is_error)
    }
}

/// Errors produced while parsing structured JSON output from a message.
#[derive(Debug, Error)]
pub enum StructuredOutputError {
    /// The message contained no text content to parse.
    #[error("message has no text content to parse as structured output")]
    MissingTextOutput,
    /// The message contained more than one text content block.
    #[error("message has multiple text content blocks; structured output is ambiguous")]
    MultipleTextBlocks,
    /// The message contained content that is not safe to treat as JSON output.
    #[error(
        "message content block at index {index} has unsupported type `{block_type}` for structured output parsing"
    )]
    UnsupportedContentBlock {
        /// Index of the unsupported content block.
        index: usize,
        /// Wire `type` value of the unsupported content block.
        block_type: &'static str,
    },
    /// The text content was not valid JSON.
    #[error("message text content is not valid JSON")]
    InvalidJson {
        /// JSON parser source error.
        #[source]
        source: serde_json::Error,
    },
    /// The JSON output did not deserialize into the requested type.
    #[error("message JSON output did not match the requested type")]
    Deserialize {
        /// JSON deserialization source error.
        #[source]
        source: serde_json::Error,
    },
}

fn content_block_type(block: &ContentBlock) -> &'static str {
    match block {
        ContentBlock::Text { .. } => "text",
        ContentBlock::Thinking { .. } => "thinking",
        ContentBlock::RedactedThinking { .. } => "redacted_thinking",
        ContentBlock::ToolUse { .. } => "tool_use",
        ContentBlock::ToolResult { .. } => "tool_result",
    }
}

/// Why model generation stopped.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum StopReason {
    /// The model reached a natural end turn.
    EndTurn,
    /// The response reached the requested token limit.
    MaxTokens,
    /// A caller-provided stop sequence matched.
    StopSequence,
    /// The model requested a tool call.
    ToolUse,
    /// The API paused a long-running turn.
    PauseTurn,
    /// The model refused to produce requested content.
    Refusal,
    /// An unknown future or provider-specific stop reason.
    Other(String),
}

impl StopReason {
    /// Returns the API wire value for this stop reason.
    pub fn as_str(&self) -> &str {
        match self {
            Self::EndTurn => "end_turn",
            Self::MaxTokens => "max_tokens",
            Self::StopSequence => "stop_sequence",
            Self::ToolUse => "tool_use",
            Self::PauseTurn => "pause_turn",
            Self::Refusal => "refusal",
            Self::Other(value) => value.as_str(),
        }
    }
}

impl From<&str> for StopReason {
    fn from(value: &str) -> Self {
        match value {
            "end_turn" => Self::EndTurn,
            "max_tokens" => Self::MaxTokens,
            "stop_sequence" => Self::StopSequence,
            "tool_use" => Self::ToolUse,
            "pause_turn" => Self::PauseTurn,
            "refusal" => Self::Refusal,
            other => Self::Other(other.to_owned()),
        }
    }
}

impl From<String> for StopReason {
    fn from(value: String) -> Self {
        match value.as_str() {
            "end_turn" => Self::EndTurn,
            "max_tokens" => Self::MaxTokens,
            "stop_sequence" => Self::StopSequence,
            "tool_use" => Self::ToolUse,
            "pause_turn" => Self::PauseTurn,
            "refusal" => Self::Refusal,
            _ => Self::Other(value),
        }
    }
}

impl Serialize for StopReason {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for StopReason {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StopReasonVisitor;

        impl Visitor<'_> for StopReasonVisitor {
            type Value = StopReason;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a stop reason string")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StopReason::from(value))
            }
        }

        deserializer.deserialize_str(StopReasonVisitor)
    }
}

/// Token usage reported by the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Input tokens used to create a cache entry, when reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u32>,
    /// Input tokens read from the cache, when reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u32>,
    /// Input token count.
    pub input_tokens: u32,
    /// Output token count.
    pub output_tokens: u32,
}

impl Usage {
    /// Creates usage counters without cache token details.
    pub const fn new(input_tokens: u32, output_tokens: u32) -> Self {
        Self {
            cache_creation_input_tokens: None,
            cache_read_input_tokens: None,
            input_tokens,
            output_tokens,
        }
    }

    /// Returns the sum of uncached, cache-creation, and cache-read input tokens.
    pub fn total_input_tokens(&self) -> u32 {
        self.input_tokens
            .saturating_add(self.cache_creation_input_tokens.unwrap_or(0))
            .saturating_add(self.cache_read_input_tokens.unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assistant_message(content: Vec<ContentBlock>) -> Message {
        Message {
            id: "msg_01".to_owned(),
            object_type: "message".to_owned(),
            role: Role::Assistant,
            model: Model::ClaudeSonnet4_5,
            content,
            stop_reason: Some(StopReason::EndTurn),
            stop_sequence: None,
            usage: Usage::new(1, 1),
        }
    }

    #[test]
    fn message_create_params_omit_absent_optional_fields() -> Result<(), Box<dyn std::error::Error>>
    {
        let params = MessageCreateParams::new(
            Model::ClaudeSonnet4_5,
            128,
            vec![MessageParam::user("Hello")],
        )?;

        let value = serde_json::to_value(params)?;

        assert!(value.get("system").is_none());
        assert!(value.get("stream").is_none());
        assert!(value.get("metadata").is_none());
        assert!(value.get("stop_sequences").is_none());
        assert!(value.get("tools").is_none());
        assert!(value.get("tool_choice").is_none());
        assert!(value.get("output_config").is_none());
        assert!(value.get("cache_control").is_none());
        assert!(value.get("container").is_none());
        assert!(value.get("inference_geo").is_none());
        assert!(value.get("context_management").is_none());
        assert!(value.get("mcp_servers").is_none());
        assert!(value.get("thinking").is_none());
        assert!(value.get("service_tier").is_none());
        assert!(value.get("temperature").is_none());
        assert!(value.get("top_k").is_none());
        assert!(value.get("top_p").is_none());
        Ok(())
    }

    #[test]
    fn builder_requires_model_max_tokens_and_message() {
        let missing_model = MessageCreateParams::builder()
            .max_tokens(128)
            .message(MessageParam::user("Hello"))
            .build();
        let missing_max_tokens = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .message(MessageParam::user("Hello"))
            .build();
        let missing_messages = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(128)
            .build();
        let zero_max_tokens = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(0)
            .message(MessageParam::user("Hello"))
            .build();

        assert_eq!(missing_model, Err(MessageCreateParamsError::MissingModel));
        assert_eq!(
            missing_max_tokens,
            Err(MessageCreateParamsError::MissingMaxTokens)
        );
        assert_eq!(
            missing_messages,
            Err(MessageCreateParamsError::EmptyMessages)
        );
        assert_eq!(
            zero_max_tokens,
            Err(MessageCreateParamsError::MaxTokens(MaxTokensError::Zero))
        );
    }

    #[test]
    fn builder_serializes_common_path_and_tools() -> Result<(), Box<dyn std::error::Error>> {
        let tool = Tool::new(
            "get_weather",
            serde_json::json!({
                "type": "object",
                "properties": { "city": { "type": "string" } },
                "required": ["city"]
            }),
        )?
        .description("Get current weather.");

        let params = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(128)
            .message(MessageParam::user("Hello"))
            .system("Be brief.")
            .stop_sequence("\n\nHuman:")
            .tool(tool)
            .tool_choice(ToolChoice::auto())
            .build()?;

        let value = serde_json::to_value(params)?;

        assert_eq!(value["model"], "claude-sonnet-4-5");
        assert_eq!(value["max_tokens"], 128);
        assert_eq!(value["system"], "Be brief.");
        assert_eq!(value["stop_sequences"][0], "\n\nHuman:");
        assert_eq!(value["tools"][0]["name"], "get_weather");
        assert_eq!(value["tool_choice"]["type"], "auto");
        assert!(
            value["tool_choice"]
                .get("disable_parallel_tool_use")
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn builder_serializes_structured_system_prompt() -> Result<(), Box<dyn std::error::Error>> {
        let params = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(128)
            .system_block(SystemPromptBlock::text_with_cache_control(
                "You are concise.",
                CacheControl::ephemeral_with_ttl(CacheControlTtl::FiveMinutes),
            ))
            .system_block(SystemPromptBlock::text("Use JSON when asked."))
            .message(MessageParam::user("Hello"))
            .build()?;

        let value = serde_json::to_value(params)?;

        assert_eq!(
            value["system"],
            serde_json::json!([
                {
                    "type": "text",
                    "text": "You are concise.",
                    "cache_control": {
                        "type": "ephemeral",
                        "ttl": "5m"
                    }
                },
                {
                    "type": "text",
                    "text": "Use JSON when asked."
                }
            ])
        );
        Ok(())
    }

    #[test]
    fn builder_serializes_json_schema_output_config() -> Result<(), Box<dyn std::error::Error>> {
        let params = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(128)
            .message(MessageParam::user("What is 2 + 2?"))
            .output_json_schema(serde_json::json!({
                "type": "object",
                "properties": {
                    "answer": { "type": "integer" }
                },
                "required": ["answer"],
                "additionalProperties": false
            }))?
            .build()?;

        let value = serde_json::to_value(params)?;

        assert_eq!(
            value["output_config"],
            serde_json::json!({
                "format": {
                    "type": "json_schema",
                    "schema": {
                        "type": "object",
                        "properties": {
                            "answer": { "type": "integer" }
                        },
                        "required": ["answer"],
                        "additionalProperties": false
                    }
                }
            })
        );
        Ok(())
    }

    #[test]
    fn builder_serializes_thinking_and_service_tier() -> Result<(), Box<dyn std::error::Error>> {
        let params = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(4096)
            .message(MessageParam::user("Think carefully, then answer."))
            .thinking_enabled_with_display(2048, ThinkingDisplay::Omitted)?
            .service_tier(ServiceTier::StandardOnly)
            .build()?;

        let value = serde_json::to_value(params)?;

        assert_eq!(
            value["thinking"],
            serde_json::json!({
                "type": "enabled",
                "budget_tokens": 2048,
                "display": "omitted"
            })
        );
        assert_eq!(value["service_tier"], "standard_only");
        Ok(())
    }

    #[test]
    fn builder_serializes_adaptive_and_disabled_thinking() -> Result<(), Box<dyn std::error::Error>>
    {
        let adaptive = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(2048)
            .message(MessageParam::user("Use adaptive thinking."))
            .thinking_adaptive_with_display(ThinkingDisplay::Summarized)
            .service_tier(ServiceTier::Auto)
            .build()?;
        let disabled = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(128)
            .message(MessageParam::user("No thinking blocks."))
            .thinking_disabled()
            .build()?;

        let adaptive = serde_json::to_value(adaptive)?;
        let disabled = serde_json::to_value(disabled)?;

        assert_eq!(
            adaptive["thinking"],
            serde_json::json!({
                "type": "adaptive",
                "display": "summarized"
            })
        );
        assert_eq!(adaptive["service_tier"], "auto");
        assert_eq!(
            disabled["thinking"],
            serde_json::json!({ "type": "disabled" })
        );
        assert!(disabled.get("service_tier").is_none());
        Ok(())
    }

    #[test]
    fn builder_serializes_cache_control_inference_geo_temperature_and_top_p()
    -> Result<(), Box<dyn std::error::Error>> {
        let params = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(128)
            .message(MessageParam::user("Use compatibility request fields."))
            .cache_control_ephemeral_with_ttl(CacheControlTtl::OneHour)
            .inference_geo("eu")?
            .temperature(1.0)?
            .top_p(0.99)?
            .build()?;

        let value = serde_json::to_value(params)?;

        assert_eq!(
            value["cache_control"],
            serde_json::json!({
                "type": "ephemeral",
                "ttl": "1h"
            })
        );
        assert_eq!(value["inference_geo"], "eu");
        assert_eq!(value["temperature"], 1.0);
        assert_eq!(value["top_p"], 0.99);
        Ok(())
    }

    #[test]
    fn cache_control_ephemeral_omits_default_ttl() -> Result<(), Box<dyn std::error::Error>> {
        let value = serde_json::to_value(CacheControl::ephemeral())?;

        assert_eq!(
            value,
            serde_json::json!({
                "type": "ephemeral"
            })
        );
        Ok(())
    }

    #[test]
    fn builder_serializes_container_context_mcp_servers_and_top_k()
    -> Result<(), Box<dyn std::error::Error>> {
        let mcp_server = McpServer::url("docs", "https://mcp.example.com/sse")?
            .authorization_token("mcp-secret-token")?
            .tool_configuration(
                McpServerToolConfiguration::new()
                    .allowed_tool(ToolName::try_new("search_docs")?)
                    .enabled(true),
            );
        let context_management = ContextManagementConfig::new(vec![
            ContextManagementEdit::ClearToolUses {
                clear_at_least: Some(InputTokensThreshold::input_tokens(2048)?),
                clear_tool_inputs: Some(ClearToolInputs::Tools(vec![ToolName::try_new(
                    "search_docs",
                )?])),
                exclude_tools: vec![ToolName::try_new("audit_log")?],
                keep: Some(ToolUsesKeep::tool_uses(2)?),
                trigger: Some(ContextManagementTrigger::input_tokens(100_000)?),
            },
            ContextManagementEdit::clear_thinking_keep(ThinkingTurnsKeep::thinking_turns(1)?),
            ContextManagementEdit::Compact {
                instructions: Some("Keep decisions and open questions.".to_owned()),
                pause_after_compaction: Some(true),
                trigger: Some(InputTokensThreshold::input_tokens(150_000)?),
            },
        ]);

        let params = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(4096)
            .message(MessageParam::user("Use the docs server."))
            .container("container_01")?
            .context_management(context_management)
            .mcp_server(mcp_server)
            .top_k(50)?
            .build()?;

        let value = serde_json::to_value(params)?;

        assert_eq!(value["container"], "container_01");
        assert_eq!(value["top_k"], 50);
        assert_eq!(
            value["mcp_servers"][0],
            serde_json::json!({
                "type": "url",
                "name": "docs",
                "url": "https://mcp.example.com/sse",
                "authorization_token": "mcp-secret-token",
                "tool_configuration": {
                    "allowed_tools": ["search_docs"],
                    "enabled": true
                }
            })
        );
        assert_eq!(
            value["context_management"],
            serde_json::json!({
                "edits": [
                    {
                        "type": "clear_tool_uses_20250919",
                        "clear_at_least": {
                            "type": "input_tokens",
                            "value": 2048
                        },
                        "clear_tool_inputs": ["search_docs"],
                        "exclude_tools": ["audit_log"],
                        "keep": {
                            "type": "tool_uses",
                            "value": 2
                        },
                        "trigger": {
                            "type": "input_tokens",
                            "value": 100000
                        }
                    },
                    {
                        "type": "clear_thinking_20251015",
                        "keep": {
                            "type": "thinking_turns",
                            "value": 1
                        }
                    },
                    {
                        "type": "compact_20260112",
                        "instructions": "Keep decisions and open questions.",
                        "pause_after_compaction": true,
                        "trigger": {
                            "type": "input_tokens",
                            "value": 150000
                        }
                    }
                ]
            })
        );
        Ok(())
    }

    #[test]
    fn new_message_request_newtypes_reject_invalid_values() -> Result<(), Box<dyn std::error::Error>>
    {
        assert_eq!(ContainerId::try_new(" "), Err(ContainerIdError::Empty));
        assert_eq!(InferenceGeo::try_new(" "), Err(InferenceGeoError::Empty));
        assert_eq!(
            Temperature::try_new(-0.01),
            Err(TemperatureError::OutOfRange { actual: -0.01 })
        );
        assert_eq!(
            Temperature::try_new(1.01),
            Err(TemperatureError::OutOfRange { actual: 1.01 })
        );
        assert_eq!(
            Temperature::try_new(f64::NAN),
            Err(TemperatureError::NonFinite)
        );
        assert_eq!(
            Temperature::try_new(f64::INFINITY),
            Err(TemperatureError::NonFinite)
        );
        assert_eq!(TopK::try_new(0), Err(TopKError::Zero));
        assert_eq!(
            TopP::try_new(-0.01),
            Err(TopPError::OutOfRange { actual: -0.01 })
        );
        assert_eq!(
            TopP::try_new(1.01),
            Err(TopPError::OutOfRange { actual: 1.01 })
        );
        assert_eq!(TopP::try_new(f64::NAN), Err(TopPError::NonFinite));
        assert_eq!(TopP::try_new(f64::INFINITY), Err(TopPError::NonFinite));
        assert_eq!(McpServerName::try_new(""), Err(McpServerNameError::Empty));
        assert_eq!(McpServerUrl::try_new(" "), Err(McpServerUrlError::Empty));
        assert_eq!(
            McpAuthorizationToken::try_new("\t"),
            Err(McpAuthorizationTokenError::Empty)
        );
        assert_eq!(
            InputTokensThreshold::input_tokens(0),
            Err(ContextTokenCountError::Zero)
        );
        assert_eq!(
            ContextManagementTrigger::tool_uses(0),
            Err(ToolUseCountError::Zero)
        );
        assert_eq!(
            ThinkingTurnsKeep::thinking_turns(0),
            Err(ThinkingTurnCountError::Zero)
        );
        assert_eq!(
            McpServer::url("", "https://mcp.example.com"),
            Err(McpServerError::Name(McpServerNameError::Empty))
        );

        let auth_result =
            McpServer::url("docs", "https://mcp.example.com")?.authorization_token("");
        assert_eq!(
            auth_result,
            Err(McpServerError::AuthorizationToken(
                McpAuthorizationTokenError::Empty
            ))
        );
        Ok(())
    }

    #[test]
    fn builder_rejects_invalid_enabled_thinking_budget() {
        let too_small = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(4096)
            .message(MessageParam::user("Hello"))
            .thinking_enabled(1023);

        assert!(matches!(
            too_small,
            Err(MessageCreateParamsError::ThinkingBudget(
                ThinkingBudgetTokensError::TooSmall {
                    min: ThinkingBudgetTokens::MIN,
                    actual: 1023,
                }
            ))
        ));

        let too_large = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(2048)
            .message(MessageParam::user("Hello"))
            .thinking_enabled(2048)
            .and_then(MessageCreateParamsBuilder::build);

        assert_eq!(
            too_large,
            Err(
                MessageCreateParamsError::ThinkingBudgetNotLessThanMaxTokens {
                    budget_tokens: 2048,
                    max_tokens: 2048,
                }
            )
        );
    }

    #[test]
    fn builder_rejects_non_object_output_schema() {
        let result = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(128)
            .message(MessageParam::user("Hello"))
            .output_json_schema(serde_json::json!("not an object"));

        assert!(matches!(
            result,
            Err(MessageCreateParamsError::OutputSchema(
                JsonSchemaError::NotObject
            ))
        ));
    }

    #[test]
    fn builder_accepts_provider_specific_model_ids() -> Result<(), Box<dyn std::error::Error>> {
        let params = MessageCreateParams::builder()
            .model("MiniMax-M2.7")
            .max_tokens(1000)
            .system("You are a helpful assistant.")
            .message(MessageParam::user("Hi, how are you?"))
            .build()?;

        let value = serde_json::to_value(params)?;

        assert_eq!(value["model"], "MiniMax-M2.7");
        assert_eq!(value["max_tokens"], 1000);
        assert_eq!(value["system"], "You are a helpful assistant.");
        Ok(())
    }

    #[test]
    fn message_param_uses_text_content_block() -> Result<(), serde_json::Error> {
        let value = serde_json::to_value(MessageParam::user("Hello"))?;

        assert_eq!(value["role"], "user");
        assert_eq!(value["content"][0]["type"], "text");
        assert_eq!(value["content"][0]["text"], "Hello");
        Ok(())
    }

    #[test]
    fn custom_model_id_still_serializes_with_expanded_request_content()
    -> Result<(), Box<dyn std::error::Error>> {
        let params = MessageCreateParams::builder()
            .model(Model::other("gateway-model-1"))
            .max_tokens(128)
            .message(MessageParam::user_blocks(vec![
                ContentBlockParam::image_url("https://example.com/image.png"),
                ContentBlockParam::document_url("https://example.com/file.pdf"),
            ]))
            .build()?;

        let value = serde_json::to_value(params)?;

        assert_eq!(value["model"], "gateway-model-1");
        assert_eq!(value["messages"][0]["content"][0]["type"], "image");
        assert_eq!(value["messages"][0]["content"][1]["type"], "document");
        Ok(())
    }

    #[test]
    fn assistant_message_can_continue_with_thinking_blocks()
    -> Result<(), Box<dyn std::error::Error>> {
        let params = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(128)
            .message(MessageParam::user("Continue from the prior turn."))
            .message(MessageParam::assistant_blocks(vec![
                ContentBlockParam::thinking_with_signature(
                    "The user wants continuity from earlier reasoning.",
                    "ThinkingSignature",
                ),
                ContentBlockParam::redacted_thinking("RedactedPayload"),
                ContentBlockParam::text("I'll continue from there."),
            ]))
            .build()?;

        let value = serde_json::to_value(params)?;

        assert_eq!(value["messages"][1]["role"], "assistant");
        assert_eq!(value["messages"][1]["content"][0]["type"], "thinking");
        assert_eq!(
            value["messages"][1]["content"][0]["thinking"],
            "The user wants continuity from earlier reasoning."
        );
        assert_eq!(
            value["messages"][1]["content"][0]["signature"],
            "ThinkingSignature"
        );
        assert_eq!(
            value["messages"][1]["content"][1]["type"],
            "redacted_thinking"
        );
        assert_eq!(
            value["messages"][1]["content"][1]["data"],
            "RedactedPayload"
        );
        assert_eq!(value["messages"][1]["content"][2]["type"], "text");
        assert_eq!(
            value["messages"][1]["content"][2]["text"],
            "I'll continue from there."
        );
        Ok(())
    }

    #[test]
    fn tool_result_helper_serializes_text_content() -> Result<(), serde_json::Error> {
        let message = MessageParam::user_blocks(vec![ContentBlockParam::tool_result(
            "toolu_01",
            "18 C and partly cloudy",
        )]);

        let value = serde_json::to_value(message)?;

        assert_eq!(value["content"][0]["type"], "tool_result");
        assert_eq!(value["content"][0]["tool_use_id"], "toolu_01");
        assert_eq!(value["content"][0]["content"], "18 C and partly cloudy");
        Ok(())
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct WeatherInput {
        city: String,
        units: String,
    }

    #[test]
    fn tool_use_decode_input_deserializes_owned_type() -> Result<(), Box<dyn std::error::Error>> {
        let message = assistant_message(vec![ContentBlock::ToolUse {
            id: "toolu_01".to_owned(),
            name: "get_weather".to_owned(),
            input: serde_json::json!({
                "city": "Paris",
                "units": "celsius"
            }),
        }]);

        let Some(tool_use) = message.find_tool_use("toolu_01") else {
            return Err("missing tool use".into());
        };
        let input: WeatherInput = tool_use.decode_input()?;

        assert_eq!(
            input,
            WeatherInput {
                city: "Paris".to_owned(),
                units: "celsius".to_owned(),
            }
        );
        Ok(())
    }

    #[test]
    fn tool_use_decode_error_display_does_not_include_input_json()
    -> Result<(), Box<dyn std::error::Error>> {
        let message = assistant_message(vec![ContentBlock::ToolUse {
            id: "toolu_01".to_owned(),
            name: "get_weather".to_owned(),
            input: serde_json::json!({
                "city": "secret-city",
            }),
        }]);

        let Some(tool_use) = message.find_tool_use("toolu_01") else {
            return Err("missing tool use".into());
        };
        let Err(error) = tool_use.decode_input::<u32>() else {
            return Err("tool input unexpectedly decoded".into());
        };
        let display = error.to_string();

        assert!(display.contains("get_weather"));
        assert!(!display.contains("secret-city"));
        Ok(())
    }

    #[test]
    fn message_tool_uses_preserve_content_order() {
        let message = assistant_message(vec![
            ContentBlock::text("before"),
            ContentBlock::ToolUse {
                id: "toolu_01".to_owned(),
                name: "first_tool".to_owned(),
                input: serde_json::json!({ "first": true }),
            },
            ContentBlock::ToolUse {
                id: "toolu_02".to_owned(),
                name: "second_tool".to_owned(),
                input: serde_json::json!({ "second": true }),
            },
        ]);

        let ids = message
            .tool_uses()
            .map(|tool_use| tool_use.id().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["toolu_01", "toolu_02"]);
    }

    #[test]
    fn tool_use_follow_up_message_serializes_expected_json()
    -> Result<(), Box<dyn std::error::Error>> {
        let message = assistant_message(vec![ContentBlock::ToolUse {
            id: "toolu_01".to_owned(),
            name: "get_weather".to_owned(),
            input: serde_json::json!({ "city": "Paris" }),
        }]);
        let Some(tool_use) = message.find_tool_use("toolu_01") else {
            return Err("missing tool use".into());
        };

        let value = serde_json::to_value(tool_use.tool_result_message("18 C and clear"))?;

        assert_eq!(
            value,
            serde_json::json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": "toolu_01",
                    "content": "18 C and clear"
                }]
            })
        );
        Ok(())
    }

    #[test]
    fn returned_assistant_message_converts_to_assistant_param()
    -> Result<(), Box<dyn std::error::Error>> {
        let message = Message {
            id: "msg_01".to_owned(),
            object_type: "message".to_owned(),
            role: Role::Assistant,
            model: Model::ClaudeSonnet4_5,
            content: vec![
                ContentBlock::Thinking {
                    thinking: "I should preserve continuity.".to_owned(),
                    signature: Some("ThinkingSignature".to_owned()),
                },
                ContentBlock::redacted_thinking("RedactedPayload"),
                ContentBlock::text("I'll continue."),
                ContentBlock::ToolUse {
                    id: "toolu_01".to_owned(),
                    name: "get_weather".to_owned(),
                    input: serde_json::json!({ "city": "Paris" }),
                },
            ],
            stop_reason: Some(StopReason::ToolUse),
            stop_sequence: None,
            usage: Usage::new(7, 11),
        };

        let param = message.to_param()?;

        assert_eq!(param.role, Role::Assistant);
        assert_eq!(
            param.content,
            vec![
                ContentBlockParam::thinking_with_signature(
                    "I should preserve continuity.",
                    "ThinkingSignature"
                ),
                ContentBlockParam::redacted_thinking("RedactedPayload"),
                ContentBlockParam::text("I'll continue."),
                ContentBlockParam::tool_use(
                    "toolu_01",
                    "get_weather",
                    serde_json::json!({ "city": "Paris" })
                )?,
            ]
        );
        Ok(())
    }

    #[test]
    fn message_conversion_reports_unsupported_content_index() {
        let message = Message {
            id: "msg_01".to_owned(),
            object_type: "message".to_owned(),
            role: Role::Assistant,
            model: Model::ClaudeSonnet4_5,
            content: vec![
                ContentBlock::text("before"),
                ContentBlock::ToolResult {
                    tool_use_id: "toolu_01".to_owned(),
                    content: vec![ContentBlock::text("result")],
                },
            ],
            stop_reason: Some(StopReason::EndTurn),
            stop_sequence: None,
            usage: Usage::new(1, 1),
        };

        assert_eq!(
            message.to_param(),
            Err(MessageParamConversionError::ContentBlock {
                index: 1,
                source: ContentBlockParamConversionError::UnsupportedContentBlock {
                    block_type: "tool_result"
                },
            })
        );
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct ParsedAnswer {
        answer: u32,
    }

    #[test]
    fn message_parse_json_output_parses_object() -> Result<(), Box<dyn std::error::Error>> {
        let message = assistant_message(vec![ContentBlock::text(r#"{"answer":4}"#)]);

        let parsed: ParsedAnswer = message.parse_json_output()?;

        assert_eq!(parsed, ParsedAnswer { answer: 4 });
        Ok(())
    }

    #[test]
    fn message_parse_json_output_reports_invalid_json() {
        let message = assistant_message(vec![ContentBlock::text("not json")]);

        let error = message.parse_json_output::<ParsedAnswer>();

        assert!(matches!(
            error,
            Err(StructuredOutputError::InvalidJson { .. })
        ));
    }

    #[test]
    fn message_parse_json_output_reports_deserialize_error() {
        let message = assistant_message(vec![ContentBlock::text(r#"{"answer":"four"}"#)]);

        let error = message.parse_json_output::<ParsedAnswer>();

        assert!(matches!(
            error,
            Err(StructuredOutputError::Deserialize { .. })
        ));
    }

    #[test]
    fn message_parse_json_output_reports_missing_text() {
        let message = assistant_message(Vec::new());

        let error = message.parse_json_output::<ParsedAnswer>();

        assert!(matches!(
            error,
            Err(StructuredOutputError::MissingTextOutput)
        ));
    }

    #[test]
    fn message_parse_json_output_reports_multiple_text_blocks() {
        let message = assistant_message(vec![
            ContentBlock::text(r#"{"answer":4}"#),
            ContentBlock::text(r#"{"answer":5}"#),
        ]);

        let error = message.parse_json_output::<ParsedAnswer>();

        assert!(matches!(
            error,
            Err(StructuredOutputError::MultipleTextBlocks)
        ));
    }

    #[test]
    fn message_parse_json_output_rejects_non_text_content() {
        let message = assistant_message(vec![
            ContentBlock::text(r#"{"answer":4}"#),
            ContentBlock::ToolUse {
                id: "toolu_01".to_owned(),
                name: "get_weather".to_owned(),
                input: serde_json::json!({ "city": "London" }),
            },
        ]);

        let error = message.parse_json_output::<ParsedAnswer>();

        assert!(matches!(
            error,
            Err(StructuredOutputError::UnsupportedContentBlock {
                index: 1,
                block_type: "tool_use"
            })
        ));
    }

    #[test]
    fn count_tokens_builder_requires_model_and_messages() {
        let missing_model = MessageCountTokensParams::builder()
            .message(MessageParam::user("Hi"))
            .build();
        let missing_messages = MessageCountTokensParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .build();

        assert_eq!(
            missing_model,
            Err(MessageCountTokensParamsError::MissingModel)
        );
        assert_eq!(
            missing_messages,
            Err(MessageCountTokensParamsError::EmptyMessages)
        );
    }

    #[test]
    fn count_tokens_serialization_omits_absent_optional_fields()
    -> Result<(), Box<dyn std::error::Error>> {
        let params = MessageCountTokensParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .message(MessageParam::user("Count me"))
            .build()?;

        let value = serde_json::to_value(params)?;

        assert_eq!(value["model"], "claude-sonnet-4-5");
        assert_eq!(value["messages"][0]["role"], "user");
        assert!(value.get("system").is_none());
        assert!(value.get("cache_control").is_none());
        assert!(value.get("tools").is_none());
        assert!(value.get("tool_choice").is_none());
        assert!(value.get("max_tokens").is_none());
        assert!(value.get("stream").is_none());
        Ok(())
    }

    #[test]
    fn count_tokens_serializes_structured_system_prompt_and_cache_control()
    -> Result<(), Box<dyn std::error::Error>> {
        let params = MessageCountTokensParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .message(MessageParam::user("Count me"))
            .system_block(SystemPromptBlock::text_with_cache_control(
                "Cache the policy.",
                CacheControl::ephemeral(),
            ))
            .cache_control_ephemeral_with_ttl(CacheControlTtl::OneHour)
            .build()?;

        let value = serde_json::to_value(params)?;

        assert_eq!(
            value["system"],
            serde_json::json!([
                {
                    "type": "text",
                    "text": "Cache the policy.",
                    "cache_control": {
                        "type": "ephemeral"
                    }
                }
            ])
        );
        assert_eq!(
            value["cache_control"],
            serde_json::json!({
                "type": "ephemeral",
                "ttl": "1h"
            })
        );
        Ok(())
    }

    #[test]
    fn count_tokens_preserves_provider_specific_model_id() -> Result<(), Box<dyn std::error::Error>>
    {
        let params = MessageCountTokensParams::builder()
            .model("MiniMax-M2.7")
            .message(MessageParam::user("Hello"))
            .build()?;

        let value = serde_json::to_value(params)?;

        assert_eq!(value["model"], "MiniMax-M2.7");
        Ok(())
    }

    #[test]
    fn stop_reason_deserializes_current_and_future_values() -> Result<(), Box<dyn std::error::Error>>
    {
        let pause = serde_json::from_str::<StopReason>(r#""pause_turn""#)?;
        let refusal = serde_json::from_str::<StopReason>(r#""refusal""#)?;
        let future = serde_json::from_str::<StopReason>(r#""provider_custom""#)?;

        assert_eq!(pause, StopReason::PauseTurn);
        assert_eq!(refusal, StopReason::Refusal);
        assert_eq!(future, StopReason::Other("provider_custom".to_owned()));
        assert_eq!(serde_json::to_value(future)?, "provider_custom");
        Ok(())
    }

    #[test]
    fn message_deserializes_new_stop_reasons() -> Result<(), Box<dyn std::error::Error>> {
        let message = serde_json::from_str::<Message>(
            r#"{
                "id": "msg_01",
                "type": "message",
                "role": "assistant",
                "model": "claude-sonnet-4-5",
                "content": [],
                "stop_reason": "refusal",
                "stop_sequence": null,
                "usage": {
                    "input_tokens": 1,
                    "output_tokens": 1
                }
            }"#,
        )?;

        assert_eq!(message.stop_reason, Some(StopReason::Refusal));
        Ok(())
    }

    #[test]
    fn usage_decodes_cache_token_counts() -> Result<(), Box<dyn std::error::Error>> {
        let usage = serde_json::from_str::<Usage>(
            r#"{
                "cache_creation_input_tokens": 5,
                "cache_read_input_tokens": 13,
                "input_tokens": 17,
                "output_tokens": 3
            }"#,
        )?;

        assert_eq!(usage.cache_creation_input_tokens, Some(5));
        assert_eq!(usage.cache_read_input_tokens, Some(13));
        assert_eq!(usage.input_tokens, 17);
        assert_eq!(usage.output_tokens, 3);
        assert_eq!(usage.total_input_tokens(), 35);
        Ok(())
    }

    #[test]
    fn usage_keeps_cache_token_counts_optional() -> Result<(), Box<dyn std::error::Error>> {
        let usage = serde_json::from_str::<Usage>(
            r#"{
                "input_tokens": 17,
                "output_tokens": 3
            }"#,
        )?;

        assert_eq!(usage, Usage::new(17, 3));
        assert_eq!(
            serde_json::to_value(usage)?,
            serde_json::json!({
                "input_tokens": 17,
                "output_tokens": 3
            })
        );
        Ok(())
    }
}
