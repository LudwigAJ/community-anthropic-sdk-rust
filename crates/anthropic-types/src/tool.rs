//! Tool definition models for the Messages API.
//!
//! [`Tool`] is the request-side union for custom tools and optional Anthropic
//! built-in/server tools. [`Tool::custom`] preserves the provider-neutral common
//! path: it serializes the same custom tool shape as earlier SDK versions and
//! does not include any built-in tool unless the caller explicitly chooses a
//! built-in variant.
//!
//! Built-in tools are provider- and model-dependent. Anthropic-compatible
//! gateways that do not support a selected built-in tool may reject the request,
//! but omitted built-ins never appear on the wire.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{CacheControl, CacheControlTtl, CitationsConfigParam, ToolName};

/// JSON Schema describing API input or output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct JsonSchema(serde_json::Value);

impl JsonSchema {
    /// Creates a JSON Schema from an object value.
    pub fn from_value(value: serde_json::Value) -> Result<Self, JsonSchemaError> {
        if !value.is_object() {
            return Err(JsonSchemaError::NotObject);
        }

        Ok(Self(value))
    }

    /// Returns the underlying JSON value.
    pub fn as_value(&self) -> &serde_json::Value {
        &self.0
    }

    /// Converts this schema into its underlying JSON value.
    pub fn into_value(self) -> serde_json::Value {
        self.0
    }
}

/// Errors produced while constructing [`JsonSchema`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum JsonSchemaError {
    /// The schema must be a JSON object.
    #[error("JSON Schema must be an object")]
    NotObject,
}

/// A tool definition supplied to the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Tool {
    /// A caller-defined custom tool.
    Custom(CustomTool),
    /// The bash server tool.
    Bash20250124(ToolBash20250124),
    /// The 2025-05-22 code execution server tool.
    CodeExecution20250522(CodeExecutionTool20250522),
    /// The 2025-08-25 code execution server tool.
    CodeExecution20250825(CodeExecutionTool20250825),
    /// The 2026-01-20 code execution server tool.
    CodeExecution20260120(CodeExecutionTool20260120),
    /// The memory server tool.
    Memory20250818(MemoryTool20250818),
    /// The 2025-01-24 text editor server tool.
    TextEditor20250124(ToolTextEditor20250124),
    /// The 2025-04-29 text editor server tool.
    TextEditor20250429(ToolTextEditor20250429),
    /// The 2025-07-28 text editor server tool.
    TextEditor20250728(ToolTextEditor20250728),
    /// The 2025-03-05 web search server tool.
    WebSearch20250305(WebSearchTool20250305),
    /// The 2026-02-09 web search server tool.
    WebSearch20260209(WebSearchTool20260209),
    /// The 2025-09-10 web fetch server tool.
    WebFetch20250910(WebFetchTool20250910),
    /// The 2026-02-09 web fetch server tool.
    WebFetch20260209(WebFetchTool20260209),
    /// The 2026-03-09 web fetch server tool.
    WebFetch20260309(WebFetchTool20260309),
    /// The BM25 tool-search server tool.
    SearchBm25_20251119(ToolSearchToolBm25_20251119),
    /// The regex tool-search server tool.
    SearchRegex20251119(ToolSearchToolRegex20251119),
}

impl Tool {
    /// Creates a custom tool definition with no description.
    pub fn custom(
        name: impl TryInto<ToolName, Error = crate::ToolNameError>,
        input_schema: JsonSchema,
    ) -> Result<Self, ToolError> {
        Ok(Self::Custom(CustomTool {
            name: name.try_into().map_err(ToolError::Name)?,
            description: None,
            input_schema,
            allowed_callers: Vec::new(),
            cache_control: None,
            defer_loading: None,
            eager_input_streaming: None,
            input_examples: Vec::new(),
            strict: None,
            tool_type: None,
        }))
    }

    /// Creates a custom tool definition from a raw JSON Schema value.
    pub fn new(
        name: impl TryInto<ToolName, Error = crate::ToolNameError>,
        input_schema: serde_json::Value,
    ) -> Result<Self, ToolError> {
        Self::custom(
            name,
            JsonSchema::from_value(input_schema).map_err(ToolError::InputSchema)?,
        )
    }

    /// Creates the `bash_20250124` built-in tool.
    pub fn bash_20250124() -> Self {
        Self::Bash20250124(ToolBash20250124::new())
    }

    /// Creates the `code_execution_20250522` built-in tool.
    pub fn code_execution_20250522() -> Self {
        Self::CodeExecution20250522(CodeExecutionTool20250522::new())
    }

    /// Creates the `code_execution_20250825` built-in tool.
    pub fn code_execution_20250825() -> Self {
        Self::CodeExecution20250825(CodeExecutionTool20250825::new())
    }

    /// Creates the `code_execution_20260120` built-in tool.
    pub fn code_execution_20260120() -> Self {
        Self::CodeExecution20260120(CodeExecutionTool20260120::new())
    }

    /// Creates the `memory_20250818` built-in tool.
    pub fn memory_20250818() -> Self {
        Self::Memory20250818(MemoryTool20250818::new())
    }

    /// Creates the `text_editor_20250124` built-in tool.
    pub fn text_editor_20250124() -> Self {
        Self::TextEditor20250124(ToolTextEditor20250124::new())
    }

    /// Creates the `text_editor_20250429` built-in tool.
    pub fn text_editor_20250429() -> Self {
        Self::TextEditor20250429(ToolTextEditor20250429::new())
    }

    /// Creates the `text_editor_20250728` built-in tool.
    pub fn text_editor_20250728() -> Self {
        Self::TextEditor20250728(ToolTextEditor20250728::new())
    }

    /// Creates the `web_search_20250305` built-in tool.
    pub fn web_search_20250305() -> Self {
        Self::WebSearch20250305(WebSearchTool20250305::new())
    }

    /// Creates the `web_search_20260209` built-in tool.
    pub fn web_search_20260209() -> Self {
        Self::WebSearch20260209(WebSearchTool20260209::new())
    }

    /// Creates the `web_fetch_20250910` built-in tool.
    pub fn web_fetch_20250910() -> Self {
        Self::WebFetch20250910(WebFetchTool20250910::new())
    }

    /// Creates the `web_fetch_20260209` built-in tool.
    pub fn web_fetch_20260209() -> Self {
        Self::WebFetch20260209(WebFetchTool20260209::new())
    }

    /// Creates the `web_fetch_20260309` built-in tool.
    pub fn web_fetch_20260309() -> Self {
        Self::WebFetch20260309(WebFetchTool20260309::new())
    }

    /// Creates the `tool_search_tool_bm25_20251119` built-in tool.
    pub fn tool_search_bm25_20251119() -> Self {
        Self::SearchBm25_20251119(ToolSearchToolBm25_20251119::new())
    }

    /// Creates the `tool_search_tool_regex_20251119` built-in tool.
    pub fn tool_search_regex_20251119() -> Self {
        Self::SearchRegex20251119(ToolSearchToolRegex20251119::new())
    }

    /// Sets the custom tool description.
    ///
    /// Built-in tools have fixed provider-defined descriptions, so this method
    /// leaves built-in variants unchanged.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        if let Self::Custom(tool) = &mut self {
            tool.description = Some(description.into());
        }
        self
    }

    /// Sets cache control on this tool definition.
    pub fn cache_control(mut self, cache_control: CacheControl) -> Self {
        match &mut self {
            Self::Custom(tool) => tool.cache_control = Some(cache_control),
            Self::Bash20250124(tool) => tool.common.cache_control = Some(cache_control),
            Self::CodeExecution20250522(tool) => tool.common.cache_control = Some(cache_control),
            Self::CodeExecution20250825(tool) => tool.common.cache_control = Some(cache_control),
            Self::CodeExecution20260120(tool) => tool.common.cache_control = Some(cache_control),
            Self::Memory20250818(tool) => tool.common.cache_control = Some(cache_control),
            Self::TextEditor20250124(tool) => tool.common.cache_control = Some(cache_control),
            Self::TextEditor20250429(tool) => tool.common.cache_control = Some(cache_control),
            Self::TextEditor20250728(tool) => tool.common.cache_control = Some(cache_control),
            Self::WebSearch20250305(tool) => tool.common.cache_control = Some(cache_control),
            Self::WebSearch20260209(tool) => tool.common.cache_control = Some(cache_control),
            Self::WebFetch20250910(tool) => tool.common.cache_control = Some(cache_control),
            Self::WebFetch20260209(tool) => tool.common.cache_control = Some(cache_control),
            Self::WebFetch20260309(tool) => tool.common.cache_control = Some(cache_control),
            Self::SearchBm25_20251119(tool) => tool.common.cache_control = Some(cache_control),
            Self::SearchRegex20251119(tool) => tool.common.cache_control = Some(cache_control),
        }
        self
    }

    /// Sets ephemeral cache control using the API default TTL.
    pub fn cache_control_ephemeral(self) -> Self {
        self.cache_control(CacheControl::ephemeral())
    }

    /// Sets ephemeral cache control with an explicit TTL.
    pub fn cache_control_ephemeral_with_ttl(self, ttl: CacheControlTtl) -> Self {
        self.cache_control(CacheControl::ephemeral_with_ttl(ttl))
    }
}

/// A caller-defined custom tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomTool {
    /// Tool name.
    pub name: ToolName,
    /// Optional tool description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema describing the tool input.
    pub input_schema: JsonSchema,
    /// Optional callers allowed to invoke this tool.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_callers: Vec<ToolCaller>,
    /// Optional cache control marker for this tool definition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
    /// Whether the tool may be deferred until referenced by tool search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defer_loading: Option<bool>,
    /// Whether tool input may be streamed eagerly.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eager_input_streaming: Option<bool>,
    /// Optional example input objects for this tool.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_examples: Vec<serde_json::Map<String, serde_json::Value>>,
    /// Whether strict tool schema validation is requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    /// Optional custom-tool discriminator.
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub tool_type: Option<CustomToolType>,
}

/// The optional discriminator for custom tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomToolType {
    /// A caller-defined custom tool.
    Custom,
}

/// A tool caller allowed to invoke a tool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCaller {
    /// The model may call the tool directly.
    Direct,
    /// The 2025-08-25 code execution tool may call the tool.
    #[serde(rename = "code_execution_20250825")]
    CodeExecution20250825,
    /// The 2026-01-20 code execution tool may call the tool.
    #[serde(rename = "code_execution_20260120")]
    CodeExecution20260120,
}

/// Shared optional request fields for built-in tools.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct BuiltInToolCommon {
    /// Optional callers allowed to invoke this tool.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_callers: Vec<ToolCaller>,
    /// Optional cache control marker for this tool definition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
    /// Whether the tool may be deferred until referenced by tool search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defer_loading: Option<bool>,
    /// Whether strict tool schema validation is requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

/// The `bash_20250124` built-in tool.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolBash20250124 {
    /// Tool name used in tool-use blocks.
    pub name: BashToolName,
    /// Tool version discriminator.
    #[serde(rename = "type")]
    pub tool_type: BashToolType,
    /// Shared optional built-in tool fields.
    #[serde(flatten)]
    pub common: BuiltInToolCommon,
    /// Optional example input objects for this tool.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_examples: Vec<serde_json::Map<String, serde_json::Value>>,
}

impl ToolBash20250124 {
    /// Creates a bash tool definition with no optional fields.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Fixed name for the bash built-in tool.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BashToolName {
    /// The `bash` tool name.
    #[default]
    Bash,
}

/// Wire discriminator for the bash built-in tool.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BashToolType {
    /// The `bash_20250124` tool type.
    #[default]
    #[serde(rename = "bash_20250124")]
    Bash20250124,
}

macro_rules! code_execution_tool {
    ($type_name:ident, $type_enum:ident, $variant:ident, $wire:literal) => {
        #[doc = concat!("The `", $wire, "` built-in code execution tool.")]
        #[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
        pub struct $type_name {
            /// Tool name used in tool-use blocks.
            pub name: CodeExecutionToolName,
            /// Tool version discriminator.
            #[serde(rename = "type")]
            pub tool_type: $type_enum,
            /// Shared optional built-in tool fields.
            #[serde(flatten)]
            pub common: BuiltInToolCommon,
        }

        impl $type_name {
            #[doc = concat!("Creates a `", $wire, "` tool definition with no optional fields.")]
            pub fn new() -> Self {
                Self::default()
            }
        }

        #[doc = concat!("Wire discriminator for the `", $wire, "` tool.")]
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $type_enum {
            #[doc = concat!("The `", $wire, "` tool type.")]
            #[default]
            #[serde(rename = $wire)]
            $variant,
        }
    };
}

code_execution_tool!(
    CodeExecutionTool20250522,
    CodeExecutionToolType20250522,
    CodeExecution20250522,
    "code_execution_20250522"
);
code_execution_tool!(
    CodeExecutionTool20250825,
    CodeExecutionToolType20250825,
    CodeExecution20250825,
    "code_execution_20250825"
);
code_execution_tool!(
    CodeExecutionTool20260120,
    CodeExecutionToolType20260120,
    CodeExecution20260120,
    "code_execution_20260120"
);

/// Fixed name for code execution built-in tools.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodeExecutionToolName {
    /// The `code_execution` tool name.
    #[default]
    CodeExecution,
}

/// The `memory_20250818` built-in tool.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemoryTool20250818 {
    /// Tool name used in tool-use blocks.
    pub name: MemoryToolName,
    /// Tool version discriminator.
    #[serde(rename = "type")]
    pub tool_type: MemoryToolType20250818,
    /// Shared optional built-in tool fields.
    #[serde(flatten)]
    pub common: BuiltInToolCommon,
    /// Optional example input objects for this tool.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_examples: Vec<serde_json::Map<String, serde_json::Value>>,
}

impl MemoryTool20250818 {
    /// Creates a memory tool definition with no optional fields.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Fixed name for the memory built-in tool.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryToolName {
    /// The `memory` tool name.
    #[default]
    Memory,
}

/// Wire discriminator for the memory built-in tool.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemoryToolType20250818 {
    /// The `memory_20250818` tool type.
    #[default]
    #[serde(rename = "memory_20250818")]
    Memory20250818,
}

macro_rules! text_editor_tool {
    ($type_name:ident, $type_enum:ident, $name_enum:ident, $name_variant:ident, $name_wire:literal, $type_variant:ident, $wire:literal) => {
        #[doc = concat!("The `", $wire, "` built-in text editor tool.")]
        #[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
        pub struct $type_name {
            /// Tool name used in tool-use blocks.
            pub name: $name_enum,
            /// Tool version discriminator.
            #[serde(rename = "type")]
            pub tool_type: $type_enum,
            /// Shared optional built-in tool fields.
            #[serde(flatten)]
            pub common: BuiltInToolCommon,
            /// Optional example input objects for this tool.
            #[serde(default, skip_serializing_if = "Vec::is_empty")]
            pub input_examples: Vec<serde_json::Map<String, serde_json::Value>>,
        }

        impl $type_name {
            #[doc = concat!("Creates a `", $wire, "` tool definition with no optional fields.")]
            pub fn new() -> Self {
                Self::default()
            }
        }

        #[doc = concat!("Fixed name for the `", $wire, "` text editor tool.")]
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name_enum {
            #[doc = concat!("The `", $name_wire, "` tool name.")]
            #[default]
            #[serde(rename = $name_wire)]
            $name_variant,
        }

        #[doc = concat!("Wire discriminator for the `", $wire, "` tool.")]
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $type_enum {
            #[doc = concat!("The `", $wire, "` tool type.")]
            #[default]
            #[serde(rename = $wire)]
            $type_variant,
        }
    };
}

text_editor_tool!(
    ToolTextEditor20250124,
    TextEditorToolType20250124,
    TextEditorToolName20250124,
    StrReplaceEditor,
    "str_replace_editor",
    TextEditor20250124,
    "text_editor_20250124"
);
text_editor_tool!(
    ToolTextEditor20250429,
    TextEditorToolType20250429,
    TextEditorToolName20250429,
    StrReplaceBasedEditTool,
    "str_replace_based_edit_tool",
    TextEditor20250429,
    "text_editor_20250429"
);

/// The `text_editor_20250728` built-in text editor tool.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolTextEditor20250728 {
    /// Tool name used in tool-use blocks.
    pub name: TextEditorToolName20250728,
    /// Tool version discriminator.
    #[serde(rename = "type")]
    pub tool_type: TextEditorToolType20250728,
    /// Shared optional built-in tool fields.
    #[serde(flatten)]
    pub common: BuiltInToolCommon,
    /// Optional example input objects for this tool.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_examples: Vec<serde_json::Map<String, serde_json::Value>>,
    /// Maximum characters returned by a file view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_characters: Option<u32>,
}

impl ToolTextEditor20250728 {
    /// Creates a `text_editor_20250728` tool definition with no optional fields.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Fixed name for the `text_editor_20250728` text editor tool.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextEditorToolName20250728 {
    /// The `str_replace_based_edit_tool` tool name.
    #[default]
    #[serde(rename = "str_replace_based_edit_tool")]
    StrReplaceBasedEditTool,
}

/// Wire discriminator for the `text_editor_20250728` tool.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextEditorToolType20250728 {
    /// The `text_editor_20250728` tool type.
    #[default]
    #[serde(rename = "text_editor_20250728")]
    TextEditor20250728,
}

/// Parameters for approximate user location used by web search tools.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserLocation {
    /// Location discriminator.
    #[serde(rename = "type")]
    pub location_type: UserLocationType,
    /// City of the user, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    /// ISO 3166-1 alpha-2 country code of the user, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// Region of the user, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// IANA time zone of the user, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
}

impl UserLocation {
    /// Creates an approximate user location with no optional fields set.
    pub fn approximate() -> Self {
        Self::default()
    }
}

/// User-location discriminator.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserLocationType {
    /// Approximate user location.
    #[default]
    Approximate,
}

macro_rules! web_search_tool {
    ($type_name:ident, $type_enum:ident, $variant:ident, $wire:literal) => {
        #[doc = concat!("The `", $wire, "` built-in web search tool.")]
        #[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
        pub struct $type_name {
            /// Tool name used in tool-use blocks.
            pub name: WebSearchToolName,
            /// Tool version discriminator.
            #[serde(rename = "type")]
            pub tool_type: $type_enum,
            /// Shared optional built-in tool fields.
            #[serde(flatten)]
            pub common: BuiltInToolCommon,
            /// Domains allowed in web search results.
            #[serde(skip_serializing_if = "Option::is_none")]
            pub allowed_domains: Option<Vec<String>>,
            /// Domains blocked from web search results.
            #[serde(skip_serializing_if = "Option::is_none")]
            pub blocked_domains: Option<Vec<String>>,
            /// Maximum number of tool uses for this request.
            #[serde(skip_serializing_if = "Option::is_none")]
            pub max_uses: Option<u32>,
            /// Approximate user location for localized results.
            #[serde(skip_serializing_if = "Option::is_none")]
            pub user_location: Option<UserLocation>,
        }

        impl $type_name {
            #[doc = concat!("Creates a `", $wire, "` tool definition with no optional fields.")]
            pub fn new() -> Self {
                Self::default()
            }
        }

        #[doc = concat!("Wire discriminator for the `", $wire, "` tool.")]
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $type_enum {
            #[doc = concat!("The `", $wire, "` tool type.")]
            #[default]
            #[serde(rename = $wire)]
            $variant,
        }
    };
}

web_search_tool!(
    WebSearchTool20250305,
    WebSearchToolType20250305,
    WebSearch20250305,
    "web_search_20250305"
);
web_search_tool!(
    WebSearchTool20260209,
    WebSearchToolType20260209,
    WebSearch20260209,
    "web_search_20260209"
);

/// Fixed name for web search built-in tools.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebSearchToolName {
    /// The `web_search` tool name.
    #[default]
    WebSearch,
}

macro_rules! web_fetch_tool {
    ($type_name:ident, $type_enum:ident, $variant:ident, $wire:literal) => {
        #[doc = concat!("The `", $wire, "` built-in web fetch tool.")]
        #[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
        pub struct $type_name {
            /// Tool name used in tool-use blocks.
            pub name: WebFetchToolName,
            /// Tool version discriminator.
            #[serde(rename = "type")]
            pub tool_type: $type_enum,
            /// Shared optional built-in tool fields.
            #[serde(flatten)]
            pub common: BuiltInToolCommon,
            /// Domains allowed for web fetches.
            #[serde(skip_serializing_if = "Option::is_none")]
            pub allowed_domains: Option<Vec<String>>,
            /// Domains blocked for web fetches.
            #[serde(skip_serializing_if = "Option::is_none")]
            pub blocked_domains: Option<Vec<String>>,
            /// Citation configuration for fetched documents.
            #[serde(skip_serializing_if = "Option::is_none")]
            pub citations: Option<CitationsConfigParam>,
            /// Maximum content tokens included in context.
            #[serde(skip_serializing_if = "Option::is_none")]
            pub max_content_tokens: Option<u32>,
            /// Maximum number of tool uses for this request.
            #[serde(skip_serializing_if = "Option::is_none")]
            pub max_uses: Option<u32>,
            /// Whether cached content may be used.
            #[serde(skip_serializing_if = "Option::is_none")]
            pub use_cache: Option<bool>,
        }

        impl $type_name {
            #[doc = concat!("Creates a `", $wire, "` tool definition with no optional fields.")]
            pub fn new() -> Self {
                Self::default()
            }
        }

        #[doc = concat!("Wire discriminator for the `", $wire, "` tool.")]
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $type_enum {
            #[doc = concat!("The `", $wire, "` tool type.")]
            #[default]
            #[serde(rename = $wire)]
            $variant,
        }
    };
}

web_fetch_tool!(
    WebFetchTool20250910,
    WebFetchToolType20250910,
    WebFetch20250910,
    "web_fetch_20250910"
);
web_fetch_tool!(
    WebFetchTool20260209,
    WebFetchToolType20260209,
    WebFetch20260209,
    "web_fetch_20260209"
);
web_fetch_tool!(
    WebFetchTool20260309,
    WebFetchToolType20260309,
    WebFetch20260309,
    "web_fetch_20260309"
);

/// Fixed name for web fetch built-in tools.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebFetchToolName {
    /// The `web_fetch` tool name.
    #[default]
    WebFetch,
}

/// The `tool_search_tool_bm25_20251119` built-in tool.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSearchToolBm25_20251119 {
    /// Tool name used in tool-use blocks.
    pub name: ToolSearchBm25ToolName,
    /// Tool version discriminator.
    #[serde(rename = "type")]
    pub tool_type: ToolSearchBm25ToolType20251119,
    /// Shared optional built-in tool fields.
    #[serde(flatten)]
    pub common: BuiltInToolCommon,
}

impl ToolSearchToolBm25_20251119 {
    /// Creates a BM25 tool-search definition with no optional fields.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Fixed name for the BM25 tool-search built-in tool.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSearchBm25ToolName {
    /// The `tool_search_tool_bm25` tool name.
    #[default]
    ToolSearchToolBm25,
}

/// Wire discriminator for the BM25 tool-search built-in tool.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolSearchBm25ToolType20251119 {
    /// The `tool_search_tool_bm25_20251119` tool type.
    #[default]
    #[serde(rename = "tool_search_tool_bm25_20251119")]
    ToolSearchToolBm25_20251119,
}

/// The `tool_search_tool_regex_20251119` built-in tool.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolSearchToolRegex20251119 {
    /// Tool name used in tool-use blocks.
    pub name: ToolSearchRegexToolName,
    /// Tool version discriminator.
    #[serde(rename = "type")]
    pub tool_type: ToolSearchRegexToolType20251119,
    /// Shared optional built-in tool fields.
    #[serde(flatten)]
    pub common: BuiltInToolCommon,
}

impl ToolSearchToolRegex20251119 {
    /// Creates a regex tool-search definition with no optional fields.
    pub fn new() -> Self {
        Self::default()
    }
}

/// Fixed name for the regex tool-search built-in tool.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolSearchRegexToolName {
    /// The `tool_search_tool_regex` tool name.
    #[default]
    ToolSearchToolRegex,
}

/// Wire discriminator for the regex tool-search built-in tool.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolSearchRegexToolType20251119 {
    /// The `tool_search_tool_regex_20251119` tool type.
    #[default]
    #[serde(rename = "tool_search_tool_regex_20251119")]
    ToolSearchToolRegex20251119,
}

/// How the model should use supplied tools.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    /// Let the model decide whether to use tools.
    Auto {
        /// Whether to prevent parallel tool use.
        #[serde(default, skip_serializing_if = "is_false")]
        disable_parallel_tool_use: bool,
    },
    /// Require use of any available tool.
    Any {
        /// Whether to prevent parallel tool use.
        #[serde(default, skip_serializing_if = "is_false")]
        disable_parallel_tool_use: bool,
    },
    /// Require use of a specific tool.
    Tool {
        /// Name of the tool to require.
        name: ToolName,
        /// Whether to prevent parallel tool use.
        #[serde(default, skip_serializing_if = "is_false")]
        disable_parallel_tool_use: bool,
    },
    /// Disallow tool use.
    None,
}

impl ToolChoice {
    /// Creates automatic tool choice.
    pub fn auto() -> Self {
        Self::Auto {
            disable_parallel_tool_use: false,
        }
    }

    /// Creates automatic tool choice with parallel tool use disabled.
    pub fn auto_disable_parallel_tool_use() -> Self {
        Self::Auto {
            disable_parallel_tool_use: true,
        }
    }

    /// Creates any-tool choice.
    pub fn any() -> Self {
        Self::Any {
            disable_parallel_tool_use: false,
        }
    }

    /// Creates a specific-tool choice.
    pub fn tool(
        name: impl TryInto<ToolName, Error = crate::ToolNameError>,
    ) -> Result<Self, ToolError> {
        Ok(Self::Tool {
            name: name.try_into().map_err(ToolError::Name)?,
            disable_parallel_tool_use: false,
        })
    }

    /// Creates no-tool choice.
    pub fn none() -> Self {
        Self::None
    }
}

/// Errors produced while constructing tool values.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ToolError {
    /// The tool name was invalid.
    #[error(transparent)]
    Name(#[from] crate::ToolNameError),
    /// The input schema was invalid.
    #[error(transparent)]
    InputSchema(#[from] JsonSchemaError),
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_tool_omits_absent_cache_control() -> Result<(), Box<dyn std::error::Error>> {
        let tool = Tool::new(
            "get_weather",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                }
            }),
        )?;

        let value = serde_json::to_value(tool)?;

        assert!(value.get("cache_control").is_none());
        assert!(value.get("type").is_none());
        Ok(())
    }

    #[test]
    fn custom_tool_serializes_cache_control() -> Result<(), Box<dyn std::error::Error>> {
        let tool = Tool::new(
            "get_weather",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "city": { "type": "string" }
                }
            }),
        )?
        .cache_control_ephemeral_with_ttl(CacheControlTtl::OneHour);

        let value = serde_json::to_value(tool)?;

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
    fn built_in_tool_serializes_exact_wire_shape() -> Result<(), Box<dyn std::error::Error>> {
        let mut tool = WebSearchTool20250305::new();
        tool.max_uses = Some(3);
        tool.user_location = Some(UserLocation {
            city: Some("London".to_owned()),
            country: Some("GB".to_owned()),
            ..UserLocation::approximate()
        });

        let value = serde_json::to_value(Tool::WebSearch20250305(tool))?;

        assert_eq!(
            value,
            serde_json::json!({
                "name": "web_search",
                "type": "web_search_20250305",
                "max_uses": 3,
                "user_location": {
                    "type": "approximate",
                    "city": "London",
                    "country": "GB"
                }
            })
        );
        Ok(())
    }

    #[test]
    fn built_in_tool_omits_absent_optional_fields() -> Result<(), Box<dyn std::error::Error>> {
        let value = serde_json::to_value(Tool::web_fetch_20260309())?;

        assert_eq!(
            value,
            serde_json::json!({
                "name": "web_fetch",
                "type": "web_fetch_20260309"
            })
        );
        Ok(())
    }
}
