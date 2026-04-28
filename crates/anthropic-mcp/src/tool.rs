//! Conversions between MCP shapes and Anthropic tool blocks.
//!
//! Three directions, each behind both a trait and a free function so
//! callers can choose method-call or function-call style:
//!
//! - [`IntoAnthropicTool`] / [`into_anthropic_tool`] — convert an
//!   [`McpTool`] (with [`McpToolInputSchema`]) into an Anthropic
//!   [`anthropic_types::Tool`]. Validates the tool name through
//!   [`anthropic_types::ToolName`] and requires a non-empty object input
//!   schema; failures surface as [`McpToolConversionError`] /
//!   [`McpToolInputSchemaError`].
//! - [`IntoAnthropicToolResult`] / [`into_anthropic_tool_result`] —
//!   convert an [`McpToolResult`] back into a Claude `tool_result`
//!   [`anthropic_types::ContentBlockParam`]. Preserves text and supported
//!   image content in order, maps MCP `isError: true` to Anthropic
//!   `is_error: true`, and reports unsupported content variants
//!   (resource links, audio) with the failing index.
//! - [`IntoMcpCallToolRequest`] / [`into_mcp_call_tool_request`] —
//!   inverse helper that turns a Claude tool-use [`ContentBlock`] into an
//!   [`McpCallToolRequest`].
//!
//! All conversion errors implement `std::error::Error` and group into
//! [`McpConversionError`] for ergonomic `?` propagation.

use anthropic_types::{ContentBlock, ContentBlockParam, JsonSchema, Tool, ToolName, ToolNameError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// MCP tool definition shape returned by `tools/list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpTool {
    /// MCP tool name.
    pub name: String,
    /// Optional MCP tool description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema for the tool input.
    #[serde(rename = "inputSchema", alias = "input_schema")]
    pub input_schema: McpToolInputSchema,
}

impl McpTool {
    /// Creates an MCP tool value.
    ///
    /// The schema is validated when converting into an Anthropic tool so callers
    /// can deserialize MCP data first and handle conversion failures uniformly.
    pub fn new(name: impl Into<String>, input_schema: serde_json::Value) -> Self {
        Self {
            name: name.into(),
            description: None,
            input_schema: McpToolInputSchema::raw(input_schema),
        }
    }

    /// Sets the MCP tool description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// MCP input schema wrapper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct McpToolInputSchema(serde_json::Value);

impl McpToolInputSchema {
    /// Creates an input schema after validating the MCP shape.
    pub fn from_value(value: serde_json::Value) -> Result<Self, McpToolInputSchemaError> {
        validate_input_schema(&value)?;
        Ok(Self(value))
    }

    /// Creates an input schema without validation.
    ///
    /// Conversion helpers validate this value before producing Anthropic types.
    pub fn raw(value: serde_json::Value) -> Self {
        Self(value)
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

/// Errors produced while validating MCP input schemas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum McpToolInputSchemaError {
    /// MCP tool input schemas must be JSON objects.
    #[error("MCP tool input schema must be a JSON object")]
    NotObject,
    /// MCP tool input schemas must not be empty.
    #[error("MCP tool input schema must not be empty")]
    Empty,
}

/// MCP tool call result paired with the Claude tool-use ID it answers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpToolResult {
    /// Claude tool-use ID this result answers.
    pub tool_use_id: String,
    /// MCP content blocks returned by the tool.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub content: Vec<McpToolResultContent>,
    /// Optional structured content returned by MCP tools.
    #[serde(
        rename = "structuredContent",
        alias = "structured_content",
        skip_serializing_if = "Option::is_none"
    )]
    pub structured_content: Option<serde_json::Value>,
    /// Whether the MCP tool reported an error.
    #[serde(
        rename = "isError",
        alias = "is_error",
        skip_serializing_if = "Option::is_none"
    )]
    pub is_error: Option<bool>,
}

impl McpToolResult {
    /// Creates an MCP tool result.
    pub fn new(
        tool_use_id: impl Into<String>,
        content: impl IntoIterator<Item = McpToolResultContent>,
    ) -> Self {
        Self {
            tool_use_id: tool_use_id.into(),
            content: content.into_iter().collect(),
            structured_content: None,
            is_error: None,
        }
    }

    /// Marks this result as a tool error or successful result.
    pub fn with_is_error(mut self, is_error: bool) -> Self {
        self.is_error = Some(is_error);
        self
    }

    /// Adds structured content returned by an MCP tool.
    pub fn structured_content(mut self, structured_content: serde_json::Value) -> Self {
        self.structured_content = Some(structured_content);
        self
    }
}

/// MCP tool result content block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpToolResultContent {
    /// Text content.
    Text {
        /// Text payload.
        text: String,
    },
    /// Base64-encoded image content.
    Image {
        /// Base64-encoded image bytes.
        data: String,
        /// Image MIME type.
        #[serde(rename = "mimeType", alias = "mime_type")]
        mime_type: String,
    },
    /// Base64-encoded audio content.
    Audio {
        /// Base64-encoded audio bytes.
        data: String,
        /// Audio MIME type.
        #[serde(rename = "mimeType", alias = "mime_type")]
        mime_type: String,
    },
    /// Embedded resource content.
    Resource {
        /// Resource contents.
        resource: McpResourceContents,
    },
    /// Link to an MCP resource.
    ResourceLink {
        /// Resource URI.
        uri: String,
        /// Resource display name.
        name: String,
        /// Optional resource MIME type.
        #[serde(
            rename = "mimeType",
            alias = "mime_type",
            skip_serializing_if = "Option::is_none"
        )]
        mime_type: Option<String>,
    },
}

impl McpToolResultContent {
    /// Creates a text MCP result content block.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Creates an image MCP result content block.
    pub fn image(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::Image {
            data: data.into(),
            mime_type: mime_type.into(),
        }
    }

    fn content_type(&self) -> &'static str {
        match self {
            Self::Text { .. } => "text",
            Self::Image { .. } => "image",
            Self::Audio { .. } => "audio",
            Self::Resource { .. } => "resource",
            Self::ResourceLink { .. } => "resource_link",
        }
    }
}

/// MCP embedded resource contents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpResourceContents {
    /// Text resource contents.
    Text {
        /// Resource URI.
        uri: String,
        /// Optional resource MIME type.
        #[serde(
            rename = "mimeType",
            alias = "mime_type",
            skip_serializing_if = "Option::is_none"
        )]
        mime_type: Option<String>,
        /// Resource text.
        text: String,
    },
    /// Blob resource contents.
    Blob {
        /// Resource URI.
        uri: String,
        /// Optional resource MIME type.
        #[serde(
            rename = "mimeType",
            alias = "mime_type",
            skip_serializing_if = "Option::is_none"
        )]
        mime_type: Option<String>,
        /// Base64-encoded resource bytes.
        blob: String,
    },
}

/// MCP `tools/call` request shape derived from an Anthropic tool-use block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpCallToolRequest {
    /// MCP tool name to call.
    pub name: String,
    /// Tool arguments, when the Claude tool-use input was an object.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

/// Errors produced while converting MCP and Anthropic tool values.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum McpConversionError {
    /// The MCP tool name was invalid.
    #[error("MCP tool name is invalid")]
    InvalidToolName {
        /// Tool-name validation source error.
        #[source]
        source: ToolNameError,
    },
    /// The MCP tool input schema was invalid.
    #[error("MCP tool input schema is invalid")]
    InvalidToolInputSchema {
        /// Input-schema validation source error.
        #[source]
        source: McpToolInputSchemaError,
    },
    /// The MCP tool result content variant is not supported by this SDK slice.
    #[error("MCP tool result content at index {index} has unsupported type `{content_type}`")]
    UnsupportedToolResultContent {
        /// Index of the unsupported MCP content block.
        index: usize,
        /// MCP content `type` value.
        content_type: &'static str,
    },
    /// The MCP image MIME type is not accepted by the Anthropic Messages API.
    #[error("MCP image content at index {index} has unsupported MIME type `{mime_type}`")]
    UnsupportedImageMimeType {
        /// Index of the unsupported MCP image block.
        index: usize,
        /// Unsupported MIME type.
        mime_type: String,
    },
    /// The Claude tool-use block contained an invalid MCP tool name.
    #[error("Claude tool-use block contains an invalid MCP tool name")]
    InvalidToolUseName {
        /// Tool-name validation source error.
        #[source]
        source: ToolNameError,
    },
    /// The Claude tool-use input was not an object or null.
    #[error("Claude tool-use block input must be a JSON object or null to become MCP arguments")]
    InvalidToolUseInput,
    /// The Claude content block is not a tool-use block.
    #[error("Claude content block is not a tool-use block")]
    NotToolUse,
}

/// Backwards-compatible alias for earlier tool-only conversion errors.
pub type McpToolConversionError = McpConversionError;

/// Converts MCP tool definitions into Anthropic SDK tool definitions.
pub trait IntoAnthropicTool {
    /// Converts this value into an Anthropic tool definition.
    fn into_anthropic_tool(self) -> Result<Tool, McpConversionError>;
}

impl IntoAnthropicTool for McpTool {
    fn into_anthropic_tool(self) -> Result<Tool, McpConversionError> {
        into_anthropic_tool(self)
    }
}

impl IntoAnthropicTool for &McpTool {
    fn into_anthropic_tool(self) -> Result<Tool, McpConversionError> {
        into_anthropic_tool(self.clone())
    }
}

/// Converts MCP tool results into Anthropic SDK tool-result blocks.
pub trait IntoAnthropicToolResult {
    /// Converts this value into an Anthropic tool-result content block.
    fn into_anthropic_tool_result(self) -> Result<ContentBlockParam, McpConversionError>;
}

impl IntoAnthropicToolResult for McpToolResult {
    fn into_anthropic_tool_result(self) -> Result<ContentBlockParam, McpConversionError> {
        into_anthropic_tool_result(self)
    }
}

impl IntoAnthropicToolResult for &McpToolResult {
    fn into_anthropic_tool_result(self) -> Result<ContentBlockParam, McpConversionError> {
        into_anthropic_tool_result(self.clone())
    }
}

/// Converts Anthropic tool-use blocks into MCP call-tool requests.
pub trait IntoMcpCallToolRequest {
    /// Converts this value into an MCP call-tool request.
    fn into_mcp_call_tool_request(self) -> Result<McpCallToolRequest, McpConversionError>;
}

impl IntoMcpCallToolRequest for ContentBlock {
    fn into_mcp_call_tool_request(self) -> Result<McpCallToolRequest, McpConversionError> {
        into_mcp_call_tool_request(self)
    }
}

impl IntoMcpCallToolRequest for &ContentBlock {
    fn into_mcp_call_tool_request(self) -> Result<McpCallToolRequest, McpConversionError> {
        into_mcp_call_tool_request(self.clone())
    }
}

/// Converts an MCP tool into an Anthropic tool definition.
pub fn into_anthropic_tool(tool: McpTool) -> Result<Tool, McpConversionError> {
    let name = ToolName::try_new(tool.name)
        .map_err(|source| McpConversionError::InvalidToolName { source })?;
    validate_input_schema(tool.input_schema.as_value())
        .map_err(|source| McpConversionError::InvalidToolInputSchema { source })?;

    let input_schema =
        JsonSchema::from_value(tool.input_schema.into_value()).map_err(|source| {
            McpConversionError::InvalidToolInputSchema {
                source: match source {
                    anthropic_types::JsonSchemaError::NotObject => {
                        McpToolInputSchemaError::NotObject
                    }
                },
            }
        })?;

    Ok(Tool {
        name,
        description: tool.description,
        input_schema,
        cache_control: None,
    })
}

/// Converts an MCP tool result into an Anthropic tool-result content block.
pub fn into_anthropic_tool_result(
    result: McpToolResult,
) -> Result<ContentBlockParam, McpConversionError> {
    let mut content = result
        .content
        .into_iter()
        .enumerate()
        .map(|(index, content)| convert_tool_result_content(index, content))
        .collect::<Result<Vec<_>, _>>()?;

    if content.is_empty()
        && let Some(structured_content) = result.structured_content
        && structured_content.is_object()
    {
        content.push(ContentBlockParam::text(structured_content.to_string()));
    }

    Ok(ContentBlockParam::ToolResult {
        tool_use_id: result.tool_use_id,
        is_error: result.is_error.filter(|is_error| *is_error),
        content: anthropic_types::ToolResultContent::blocks(content),
        cache_control: None,
    })
}

/// Converts a Claude tool-use block into an MCP call-tool request.
pub fn into_mcp_call_tool_request(
    block: ContentBlock,
) -> Result<McpCallToolRequest, McpConversionError> {
    let ContentBlock::ToolUse { name, input, .. } = block else {
        return Err(McpConversionError::NotToolUse);
    };

    let name = ToolName::try_new(name)
        .map_err(|source| McpConversionError::InvalidToolUseName { source })?
        .into_string();
    let arguments = if input.is_null() {
        None
    } else if input.is_object() {
        Some(input)
    } else {
        return Err(McpConversionError::InvalidToolUseInput);
    };

    Ok(McpCallToolRequest { name, arguments })
}

fn validate_input_schema(value: &serde_json::Value) -> Result<(), McpToolInputSchemaError> {
    let object = value
        .as_object()
        .ok_or(McpToolInputSchemaError::NotObject)?;
    if object.is_empty() {
        return Err(McpToolInputSchemaError::Empty);
    }

    Ok(())
}

fn convert_tool_result_content(
    index: usize,
    content: McpToolResultContent,
) -> Result<ContentBlockParam, McpConversionError> {
    match content {
        McpToolResultContent::Text { text } => Ok(ContentBlockParam::text(text)),
        McpToolResultContent::Image { data, mime_type } => {
            if !is_supported_image_type(&mime_type) {
                return Err(McpConversionError::UnsupportedImageMimeType { index, mime_type });
            }

            Ok(ContentBlockParam::image_base64(mime_type, data))
        }
        other => Err(McpConversionError::UnsupportedToolResultContent {
            index,
            content_type: other.content_type(),
        }),
    }
}

fn is_supported_image_type(mime_type: &str) -> bool {
    matches!(
        mime_type,
        "image/jpeg" | "image/png" | "image/gif" | "image/webp"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use anthropic_types::{ToolNameError, ToolResultContent};

    #[test]
    fn mcp_tool_converts_to_anthropic_tool() -> Result<(), Box<dyn std::error::Error>> {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "city": { "type": "string" }
            },
            "required": ["city"],
            "x-provider": { "kept": true }
        });
        let tool = McpTool::new("get_weather", schema.clone())
            .description("Get the current weather for a city.");

        let converted = tool.into_anthropic_tool()?;

        assert_eq!(converted.name.as_str(), "get_weather");
        assert_eq!(
            converted.description.as_deref(),
            Some("Get the current weather for a city.")
        );
        assert_eq!(converted.input_schema.as_value(), &schema);
        Ok(())
    }

    #[test]
    fn mcp_tool_rejects_empty_schema() {
        let tool = McpTool::new("get_weather", serde_json::json!({}));

        let result = tool.into_anthropic_tool();

        assert_eq!(
            result,
            Err(McpConversionError::InvalidToolInputSchema {
                source: McpToolInputSchemaError::Empty
            })
        );
    }

    #[test]
    fn mcp_tool_rejects_non_object_schema() {
        let tool = McpTool::new("get_weather", serde_json::json!(true));

        let result = tool.into_anthropic_tool();

        assert_eq!(
            result,
            Err(McpConversionError::InvalidToolInputSchema {
                source: McpToolInputSchemaError::NotObject
            })
        );
    }

    #[test]
    fn mcp_tool_rejects_invalid_name_with_tool_name_validation() {
        let tool = McpTool::new(" ", serde_json::json!({ "type": "object" }));

        let result = tool.into_anthropic_tool();

        assert_eq!(
            result,
            Err(McpConversionError::InvalidToolName {
                source: ToolNameError::Empty
            })
        );
    }

    #[test]
    fn text_only_mcp_tool_result_converts_to_tool_result_block() {
        let result = McpToolResult::new(
            "toolu_01",
            [McpToolResultContent::text("18 C and partly cloudy")],
        );

        let converted = result.into_anthropic_tool_result();

        assert_eq!(
            converted,
            Ok(ContentBlockParam::ToolResult {
                tool_use_id: "toolu_01".to_owned(),
                is_error: None,
                content: ToolResultContent::blocks(vec![ContentBlockParam::text(
                    "18 C and partly cloudy"
                )]),
                cache_control: None,
            })
        );
    }

    #[test]
    fn multi_block_mcp_tool_result_preserves_order() {
        let result = McpToolResult::new(
            "toolu_01",
            [
                McpToolResultContent::text("first"),
                McpToolResultContent::image("base64-image", "image/png"),
                McpToolResultContent::text("third"),
            ],
        );

        let converted = result.into_anthropic_tool_result();

        assert_eq!(
            converted,
            Ok(ContentBlockParam::ToolResult {
                tool_use_id: "toolu_01".to_owned(),
                is_error: None,
                content: ToolResultContent::blocks(vec![
                    ContentBlockParam::text("first"),
                    ContentBlockParam::image_base64("image/png", "base64-image"),
                    ContentBlockParam::text("third"),
                ]),
                cache_control: None,
            })
        );
    }

    #[test]
    fn unsupported_mcp_tool_result_content_reports_index() {
        let result = McpToolResult::new(
            "toolu_01",
            [
                McpToolResultContent::text("first"),
                McpToolResultContent::ResourceLink {
                    uri: "file:///tmp/weather.txt".to_owned(),
                    name: "weather.txt".to_owned(),
                    mime_type: Some("text/plain".to_owned()),
                },
            ],
        );

        let converted = result.into_anthropic_tool_result();

        assert_eq!(
            converted,
            Err(McpConversionError::UnsupportedToolResultContent {
                index: 1,
                content_type: "resource_link"
            })
        );
    }

    #[test]
    fn unsupported_mcp_image_mime_type_reports_index() {
        let result = McpToolResult::new(
            "toolu_01",
            [
                McpToolResultContent::text("first"),
                McpToolResultContent::image("base64-image", "image/svg+xml"),
            ],
        );

        let converted = result.into_anthropic_tool_result();

        assert_eq!(
            converted,
            Err(McpConversionError::UnsupportedImageMimeType {
                index: 1,
                mime_type: "image/svg+xml".to_owned()
            })
        );
    }

    #[test]
    fn error_mcp_tool_result_sets_anthropic_is_error_true() -> Result<(), Box<dyn std::error::Error>>
    {
        let result = McpToolResult::new("toolu_01", [McpToolResultContent::text("tool failed")])
            .with_is_error(true);

        let converted = result.into_anthropic_tool_result()?;
        let value = serde_json::to_value(converted)?;

        assert_eq!(value["type"], "tool_result");
        assert_eq!(value["tool_use_id"], "toolu_01");
        assert_eq!(value["is_error"], true);
        assert_eq!(value["content"][0]["text"], "tool failed");
        Ok(())
    }

    #[test]
    fn false_mcp_tool_result_error_flag_is_omitted() -> Result<(), Box<dyn std::error::Error>> {
        let result =
            McpToolResult::new("toolu_01", [McpToolResultContent::text("ok")]).with_is_error(false);

        let converted = result.into_anthropic_tool_result()?;
        let value = serde_json::to_value(converted)?;

        assert!(value.get("is_error").is_none());
        Ok(())
    }

    #[test]
    fn tool_use_block_converts_to_mcp_call_tool_request() -> Result<(), Box<dyn std::error::Error>>
    {
        let block = ContentBlock::ToolUse {
            id: "toolu_01".to_owned(),
            name: "get_weather".to_owned(),
            input: serde_json::json!({ "city": "London" }),
        };

        let request = block.into_mcp_call_tool_request()?;

        assert_eq!(
            request,
            McpCallToolRequest {
                name: "get_weather".to_owned(),
                arguments: Some(serde_json::json!({ "city": "London" })),
            }
        );
        Ok(())
    }
}
