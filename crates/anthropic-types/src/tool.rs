//! Tool definition models for the Messages API.
//!
//! [`Tool`] is the request-side tool definition, validated through the
//! [`crate::ToolName`] newtype and a non-empty object [`JsonSchema`] for
//! `input_schema`. Builders carry an optional [`crate::CacheControl`] so
//! tool definitions participate in prompt caching.
//!
//! [`ToolChoice`] mirrors the four API shapes: `Auto`, `Any`,
//! `Tool { name }`, and `None`. Omit `tool_choice` from a request to leave
//! the API default in effect.
//!
//! [`JsonSchema`] preserves arbitrary JSON schemas as
//! [`serde_json::Value`] but enforces that the top level deserializes into
//! an object and is not empty. [`JsonSchemaError`] and [`ToolError`]
//! surface validation failures at construction.

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{CacheControl, CacheControlTtl, ToolName};

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
pub struct Tool {
    /// Tool name.
    pub name: ToolName,
    /// Optional tool description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// JSON Schema describing the tool input.
    pub input_schema: JsonSchema,
    /// Optional cache control marker for this tool definition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

impl Tool {
    /// Creates a tool definition with no description.
    pub fn custom(
        name: impl TryInto<ToolName, Error = crate::ToolNameError>,
        input_schema: JsonSchema,
    ) -> Result<Self, ToolError> {
        Ok(Self {
            name: name.try_into().map_err(ToolError::Name)?,
            description: None,
            input_schema,
            cache_control: None,
        })
    }

    /// Creates a tool definition from a raw JSON Schema value.
    pub fn new(
        name: impl TryInto<ToolName, Error = crate::ToolNameError>,
        input_schema: serde_json::Value,
    ) -> Result<Self, ToolError> {
        Self::custom(
            name,
            JsonSchema::from_value(input_schema).map_err(ToolError::InputSchema)?,
        )
    }

    /// Sets the tool description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Sets cache control on this tool definition.
    pub fn cache_control(mut self, cache_control: CacheControl) -> Self {
        self.cache_control = Some(cache_control);
        self
    }

    /// Sets ephemeral cache control using the API default TTL.
    pub fn cache_control_ephemeral(mut self) -> Self {
        self.cache_control = Some(CacheControl::ephemeral());
        self
    }

    /// Sets ephemeral cache control with an explicit TTL.
    pub fn cache_control_ephemeral_with_ttl(mut self, ttl: CacheControlTtl) -> Self {
        self.cache_control = Some(CacheControl::ephemeral_with_ttl(ttl));
        self
    }
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
    fn tool_omits_absent_cache_control() -> Result<(), Box<dyn std::error::Error>> {
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
        Ok(())
    }

    #[test]
    fn tool_serializes_cache_control() -> Result<(), Box<dyn std::error::Error>> {
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
}
