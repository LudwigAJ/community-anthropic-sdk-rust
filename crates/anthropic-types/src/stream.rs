//! Server-Sent Event payloads for the Messages streaming API.
//!
//! [`MessageStreamEvent`] is the typed union of every event Anthropic emits
//! over the SSE channel: `message_start`, `content_block_start`,
//! `content_block_delta`, `content_block_stop`, `message_delta`,
//! `message_stop`, `ping`, `error`, and a forward-compatible `Other`
//! catch-all that preserves the original event name and raw JSON payload.
//!
//! [`ContentBlockDelta`] models the per-block delta payloads (text deltas,
//! input-JSON deltas for tool use, thinking deltas, signature deltas,
//! citation deltas). [`MessageDelta`] covers the top-level `stop_reason` /
//! `stop_sequence` updates that arrive late in the stream.
//!
//! These types are pure data; the async stream wrapper that decodes them
//! lives in `anthropic_client::stream`.

use serde::{Deserialize, Deserializer, Serialize, de};

use crate::{ApiErrorDetail, ContentBlock, Message, StopReason, TextCitation, Usage};

/// A streamed message event.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageStreamEvent {
    /// Message creation started.
    MessageStart {
        /// Initial message snapshot.
        message: Message,
    },
    /// A content block started.
    ContentBlockStart {
        /// Content block index.
        index: u32,
        /// Initial content block snapshot.
        content_block: ContentBlock,
    },
    /// A content block delta arrived.
    ContentBlockDelta {
        /// Content block index.
        index: u32,
        /// Incremental content delta.
        delta: ContentBlockDelta,
    },
    /// A content block completed.
    ContentBlockStop {
        /// Content block index.
        index: u32,
    },
    /// Message-level metadata changed.
    MessageDelta {
        /// Incremental message delta.
        delta: MessageDelta,
        /// Incremental usage data.
        #[serde(skip_serializing_if = "Option::is_none")]
        usage: Option<Usage>,
    },
    /// Message creation completed.
    MessageStop,
    /// Keepalive event.
    Ping,
    /// Stream-level API error.
    Error {
        /// Error details.
        error: ApiErrorDetail,
    },
    /// An unrecognized stream event preserved for forward compatibility.
    Other {
        /// SSE event name.
        event: String,
        /// Raw event data parsed as arbitrary JSON.
        data: serde_json::Value,
    },
}

impl<'de> Deserialize<'de> for MessageStreamEvent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        let event_type = value
            .get("type")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| de::Error::missing_field("type"))?
            .to_owned();

        match event_type.as_str() {
            "message_start"
            | "content_block_start"
            | "content_block_delta"
            | "content_block_stop"
            | "message_delta"
            | "message_stop"
            | "ping"
            | "error" => MessageStreamEventKnown::deserialize(value)
                .map(Into::into)
                .map_err(de::Error::custom),
            _ => Ok(Self::Other {
                event: event_type,
                data: value,
            }),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum MessageStreamEventKnown {
    MessageStart {
        message: Message,
    },
    ContentBlockStart {
        index: u32,
        content_block: ContentBlock,
    },
    ContentBlockDelta {
        index: u32,
        delta: ContentBlockDelta,
    },
    ContentBlockStop {
        index: u32,
    },
    MessageDelta {
        delta: MessageDelta,
        usage: Option<Usage>,
    },
    MessageStop,
    Ping,
    Error {
        error: ApiErrorDetail,
    },
}

impl From<MessageStreamEventKnown> for MessageStreamEvent {
    fn from(event: MessageStreamEventKnown) -> Self {
        match event {
            MessageStreamEventKnown::MessageStart { message } => Self::MessageStart { message },
            MessageStreamEventKnown::ContentBlockStart {
                index,
                content_block,
            } => Self::ContentBlockStart {
                index,
                content_block,
            },
            MessageStreamEventKnown::ContentBlockDelta { index, delta } => {
                Self::ContentBlockDelta { index, delta }
            }
            MessageStreamEventKnown::ContentBlockStop { index } => Self::ContentBlockStop { index },
            MessageStreamEventKnown::MessageDelta { delta, usage } => {
                Self::MessageDelta { delta, usage }
            }
            MessageStreamEventKnown::MessageStop => Self::MessageStop,
            MessageStreamEventKnown::Ping => Self::Ping,
            MessageStreamEventKnown::Error { error } => Self::Error { error },
        }
    }
}

/// A content block delta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlockDelta {
    /// Text was appended.
    #[serde(rename = "text_delta")]
    Text {
        /// Appended text.
        text: String,
    },
    /// A partial JSON fragment for tool input was appended.
    #[serde(rename = "input_json_delta")]
    InputJson {
        /// Partial JSON fragment.
        partial_json: String,
    },
    /// Thinking text was appended.
    #[serde(rename = "thinking_delta")]
    Thinking {
        /// Appended thinking text.
        thinking: String,
    },
    /// Thinking signature data was emitted.
    #[serde(rename = "signature_delta")]
    Signature {
        /// Signature data.
        signature: String,
    },
    /// A citation was appended to a text block.
    #[serde(rename = "citations_delta")]
    Citations {
        /// Citation appended to the text block.
        citation: TextCitation,
    },
}

impl ContentBlockDelta {
    /// Returns appended text for text deltas.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            _ => None,
        }
    }
}

/// Message-level streaming delta data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageDelta {
    /// Updated stop reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<StopReason>,
    /// Updated stop sequence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequence: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_message_delta_usage_from_top_level() -> Result<(), Box<dyn std::error::Error>> {
        let event = serde_json::from_str::<MessageStreamEvent>(
            r#"{
                "type": "message_delta",
                "delta": {
                    "stop_reason": "end_turn",
                    "stop_sequence": null
                },
                "usage": {
                    "input_tokens": 0,
                    "output_tokens": 7
                }
            }"#,
        )?;

        assert_eq!(
            event,
            MessageStreamEvent::MessageDelta {
                delta: MessageDelta {
                    stop_reason: Some(StopReason::EndTurn),
                    stop_sequence: None,
                },
                usage: Some(Usage {
                    input_tokens: 0,
                    output_tokens: 7,
                }),
            }
        );
        Ok(())
    }

    #[test]
    fn deserializes_message_delta_new_and_unknown_stop_reasons()
    -> Result<(), Box<dyn std::error::Error>> {
        let pause_turn = serde_json::from_str::<MessageStreamEvent>(
            r#"{
                "type": "message_delta",
                "delta": {
                    "stop_reason": "pause_turn",
                    "stop_sequence": null
                },
                "usage": {
                    "input_tokens": 0,
                    "output_tokens": 7
                }
            }"#,
        )?;
        let future = serde_json::from_str::<MessageStreamEvent>(
            r#"{
                "type": "message_delta",
                "delta": {
                    "stop_reason": "provider_custom",
                    "stop_sequence": null
                },
                "usage": null
            }"#,
        )?;

        assert!(matches!(
            pause_turn,
            MessageStreamEvent::MessageDelta {
                delta: MessageDelta {
                    stop_reason: Some(StopReason::PauseTurn),
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            future,
            MessageStreamEvent::MessageDelta {
                delta: MessageDelta {
                    stop_reason: Some(StopReason::Other(ref value)),
                    ..
                },
                ..
            } if value == "provider_custom"
        ));
        Ok(())
    }

    #[test]
    fn deserializes_unknown_stream_events_as_other() -> Result<(), Box<dyn std::error::Error>> {
        let event =
            serde_json::from_str::<MessageStreamEvent>(r#"{"type":"provider_custom","value":42}"#)?;

        match event {
            MessageStreamEvent::Other { event, data } => {
                assert_eq!(event, "provider_custom");
                assert_eq!(data["value"], 42);
            }
            other => {
                return Err(std::io::Error::other(format!("expected Other, got {other:?}")).into());
            }
        }

        Ok(())
    }
}
