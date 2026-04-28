//! Model identifiers and the `models.list` / `models.retrieve` response.
//!
//! [`Model`] enumerates well-known Claude model IDs as variants and uses
//! `Model::Other(String)` to round-trip identifiers that the SDK does not
//! recognize yet — this also lets callers target Anthropic-compatible
//! gateways with provider-specific model strings.
//!
//! [`ModelInfo`] is the response shape from `/v1/models`. Capability
//! metadata is preserved as `Option<serde_json::Value>` so newly added
//! capability flags never block on an SDK release; use
//! [`ModelInfo::model`] to obtain a typed [`Model`] from the raw `id`.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A Claude model identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Model {
    /// Claude Opus 4.6.
    ClaudeOpus4_6,
    /// Claude Sonnet 4.5.
    ClaudeSonnet4_5,
    /// Claude Haiku 4.5.
    ClaudeHaiku4_5,
    /// A model identifier not known to this crate yet.
    Other {
        /// Raw model identifier.
        id: String,
    },
}

impl Model {
    /// Creates a model identifier from a raw string.
    pub fn other(id: impl Into<String>) -> Self {
        Self::Other { id: id.into() }
    }

    /// Returns the API string for this model.
    pub fn as_str(&self) -> &str {
        match self {
            Self::ClaudeOpus4_6 => "claude-opus-4-6",
            Self::ClaudeSonnet4_5 => "claude-sonnet-4-5",
            Self::ClaudeHaiku4_5 => "claude-haiku-4-5",
            Self::Other { id } => id.as_str(),
        }
    }
}

impl From<String> for Model {
    fn from(value: String) -> Self {
        match value.as_str() {
            "claude-opus-4-6" => Self::ClaudeOpus4_6,
            "claude-sonnet-4-5" => Self::ClaudeSonnet4_5,
            "claude-haiku-4-5" => Self::ClaudeHaiku4_5,
            _ => Self::Other { id: value },
        }
    }
}

impl From<&str> for Model {
    fn from(value: &str) -> Self {
        Self::from(value.to_owned())
    }
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for Model {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Model {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from(value))
    }
}

/// Metadata returned by the Models API for a single model.
///
/// The `capabilities` field carries the API's evolving nested capability
/// description as raw JSON so that new capability flags do not require an SDK
/// release. Stable scalar metadata is decoded into typed fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Object type. Always `"model"` for the Models API.
    #[serde(rename = "type")]
    pub object_type: String,
    /// Unique model identifier.
    pub id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// RFC 3339 timestamp when the model was released.
    pub created_at: String,
    /// Maximum input context window size in tokens, when reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_input_tokens: Option<u32>,
    /// Maximum value of the `max_tokens` request parameter, when reported.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Capability metadata. Held as raw JSON because the API's capability
    /// schema is large and evolves across model releases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<serde_json::Value>,
}

impl ModelInfo {
    /// Returns the model identifier as a [`Model`].
    pub fn model(&self) -> Model {
        Model::from(self.id.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_model_serializes_to_api_identifier() -> Result<(), serde_json::Error> {
        let value = serde_json::to_string(&Model::ClaudeSonnet4_5)?;

        assert_eq!(value, "\"claude-sonnet-4-5\"");
        Ok(())
    }

    #[test]
    fn model_info_decodes_minimal_fields() -> Result<(), Box<dyn std::error::Error>> {
        let info: ModelInfo = serde_json::from_str(
            r#"{
                "type": "model",
                "id": "claude-sonnet-4-5",
                "display_name": "Claude Sonnet 4.5",
                "created_at": "2025-09-29T00:00:00Z"
            }"#,
        )?;

        assert_eq!(info.id, "claude-sonnet-4-5");
        assert_eq!(info.display_name, "Claude Sonnet 4.5");
        assert_eq!(info.created_at, "2025-09-29T00:00:00Z");
        assert_eq!(info.max_input_tokens, None);
        assert_eq!(info.max_tokens, None);
        assert_eq!(info.capabilities, None);
        assert_eq!(info.model(), Model::ClaudeSonnet4_5);
        Ok(())
    }

    #[test]
    fn model_info_preserves_capability_metadata() -> Result<(), Box<dyn std::error::Error>> {
        let info: ModelInfo = serde_json::from_str(
            r#"{
                "type": "model",
                "id": "claude-future",
                "display_name": "Claude Future",
                "created_at": "2026-01-01T00:00:00Z",
                "max_input_tokens": 200000,
                "max_tokens": 8192,
                "capabilities": { "batch": { "supported": true } }
            }"#,
        )?;

        assert_eq!(info.max_input_tokens, Some(200_000));
        assert_eq!(info.max_tokens, Some(8192));
        assert_eq!(
            info.capabilities,
            Some(serde_json::json!({ "batch": { "supported": true } }))
        );
        assert_eq!(info.model(), Model::other("claude-future"));
        Ok(())
    }

    #[test]
    fn unknown_model_round_trips() -> Result<(), serde_json::Error> {
        let model: Model = serde_json::from_str("\"claude-future-model\"")?;

        assert_eq!(model.as_str(), "claude-future-model");
        assert_eq!(serde_json::to_string(&model)?, "\"claude-future-model\"");
        Ok(())
    }
}
