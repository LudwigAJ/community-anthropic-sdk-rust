#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! Optional MCP conversion helpers.
//!
//! This crate exists so MCP-specific types and conversion logic can grow without
//! coupling the core client crate to an MCP runtime.
//!
//! # Example
//!
//! ```no_run
//! # use anthropic_mcp::{IntoAnthropicTool, McpTool};
//! # use anthropic_types::{MessageCreateParams, MessageParam, Model};
//! # fn build_params(mcp_tools: Vec<McpTool>) -> Result<MessageCreateParams, Box<dyn std::error::Error>> {
//! let tools = mcp_tools
//!     .into_iter()
//!     .map(IntoAnthropicTool::into_anthropic_tool)
//!     .collect::<Result<Vec<_>, _>>()?;
//!
//! let params = MessageCreateParams::builder()
//!     .model(Model::ClaudeSonnet4_5)
//!     .max_tokens(1024)
//!     .message(MessageParam::user("Use the available tools."))
//!     .tools(tools)
//!     .build()?;
//! # Ok(params)
//! # }
//! ```

mod tool;

pub use tool::{
    IntoAnthropicTool, IntoAnthropicToolResult, IntoMcpCallToolRequest, McpCallToolRequest,
    McpConversionError, McpResourceContents, McpTool, McpToolConversionError, McpToolInputSchema,
    McpToolInputSchemaError, McpToolResult, McpToolResultContent, into_anthropic_tool,
    into_anthropic_tool_result, into_mcp_call_tool_request,
};
