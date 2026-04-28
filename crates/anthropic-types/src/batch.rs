//! Request and response models for the Message Batches API.
//!
//! Models the lifecycle of `/v1/messages/batches`:
//!
//! - [`BatchCreateParams`] / [`BatchCreateParamsBuilder`] / [`BatchCreateRequest`]
//!   construct the request body, rejecting empty batches, duplicate or empty
//!   `custom_id`s, and entries whose inner [`crate::MessageCreateParams`]
//!   set `stream = true` (not allowed in batch entries).
//! - [`MessageBatch`] is the lifecycle resource, keyed by [`MessageBatchId`]
//!   (a non-blank newtype validated at construction). [`BatchProcessingStatus`]
//!   reports the current phase, and [`MessageBatchRequestCounts`] mirrors the
//!   API counts surface.
//! - [`MessageBatchIndividualResponse`] is one decoded JSONL line from the
//!   `results_url` stream, carrying a [`MessageBatchResult`] tagged union of
//!   `Succeeded { message }`, `Errored { error }`, `Canceled`, and `Expired`.
//! - [`DeletedMessageBatch`] is the response from `DELETE
//!   /v1/messages/batches/{id}`.

use std::{collections::HashSet, fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ApiErrorBody, Message, MessageCreateParams};

/// Object type marker for a message batch response.
pub const MESSAGE_BATCH_OBJECT_TYPE: &str = "message_batch";

/// Identifier for a message batch.
///
/// The API does not guarantee a stable ID length or complete format over time,
/// so the SDK only validates the invariant callers can rely on locally: batch
/// IDs must not be blank.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct MessageBatchId(String);

impl MessageBatchId {
    /// Creates a message batch ID from a non-blank string.
    pub fn try_new(value: impl Into<String>) -> Result<Self, MessageBatchIdError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(MessageBatchIdError::Empty);
        }

        Ok(Self(value))
    }

    /// Returns the message batch ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Converts this message batch ID into its owned string.
    pub fn into_string(self) -> String {
        self.0
    }
}

impl AsRef<str> for MessageBatchId {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl TryFrom<String> for MessageBatchId {
    type Error = MessageBatchIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl TryFrom<&str> for MessageBatchId {
    type Error = MessageBatchIdError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl From<MessageBatchId> for String {
    fn from(value: MessageBatchId) -> Self {
        value.into_string()
    }
}

impl fmt::Display for MessageBatchId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for MessageBatchId {
    type Err = MessageBatchIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_new(value)
    }
}

/// Errors produced while constructing [`MessageBatchId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MessageBatchIdError {
    /// Message batch IDs must not be blank.
    #[error("message batch ID must not be blank")]
    Empty,
}

/// Processing state of a message batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchProcessingStatus {
    /// The batch is still processing requests.
    InProgress,
    /// Cancellation has been initiated for the batch.
    Canceling,
    /// All requests have reached a terminal state.
    Ended,
}

impl BatchProcessingStatus {
    /// Returns the wire-format string for this status.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InProgress => "in_progress",
            Self::Canceling => "canceling",
            Self::Ended => "ended",
        }
    }
}

impl fmt::Display for BatchProcessingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Tally of requests in a message batch by terminal status.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageBatchRequestCounts {
    /// Requests still being processed.
    pub processing: u32,
    /// Requests that completed successfully.
    pub succeeded: u32,
    /// Requests that returned an error.
    pub errored: u32,
    /// Requests that were canceled before completion.
    pub canceled: u32,
    /// Requests that expired before completion.
    pub expired: u32,
}

/// A message batch returned by the API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageBatch {
    /// Unique batch identifier.
    pub id: MessageBatchId,
    /// Object type. Always `"message_batch"`.
    #[serde(rename = "type")]
    pub object_type: String,
    /// Current processing status.
    pub processing_status: BatchProcessingStatus,
    /// Tally of requests by terminal status.
    pub request_counts: MessageBatchRequestCounts,
    /// RFC 3339 timestamp when the batch was created.
    pub created_at: String,
    /// RFC 3339 timestamp when the batch expires.
    pub expires_at: String,
    /// RFC 3339 timestamp when processing ended, when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    /// RFC 3339 timestamp when the batch was archived, when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    /// RFC 3339 timestamp when cancellation was initiated, when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel_initiated_at: Option<String>,
    /// URL of the JSONL results file, available once processing has ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub results_url: Option<String>,
}

/// One line from the message batch JSONL results stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MessageBatchIndividualResponse {
    /// Caller-supplied identifier matching the request entry.
    pub custom_id: String,
    /// Per-request processing result.
    pub result: MessageBatchResult,
}

/// Per-request processing result inside a message batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MessageBatchResult {
    /// The request completed successfully.
    Succeeded {
        /// The message returned by the model.
        message: Message,
    },
    /// The request returned an API error.
    Errored {
        /// Error details returned for this request.
        error: ApiErrorBody,
    },
    /// The request was canceled before completion.
    Canceled,
    /// The request expired before completion.
    Expired,
}

/// One create entry in a message batch request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchCreateRequest {
    /// Caller-supplied identifier returned alongside the result.
    pub custom_id: String,
    /// Messages API parameters for this individual request.
    pub params: MessageCreateParams,
}

impl BatchCreateRequest {
    /// Creates a new batch entry.
    pub fn new(
        custom_id: impl Into<String>,
        params: MessageCreateParams,
    ) -> Result<Self, BatchCreateRequestError> {
        let custom_id = custom_id.into();
        if custom_id.is_empty() {
            return Err(BatchCreateRequestError::EmptyCustomId);
        }
        if params.stream == Some(true) {
            return Err(BatchCreateRequestError::StreamingNotAllowed);
        }

        Ok(Self { custom_id, params })
    }
}

/// Errors produced while building a [`BatchCreateRequest`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum BatchCreateRequestError {
    /// The custom identifier was empty.
    #[error("batch request custom_id must not be empty")]
    EmptyCustomId,
    /// Streaming is not supported within message batch entries.
    #[error("batch request params must not enable streaming")]
    StreamingNotAllowed,
}

/// Parameters for creating a message batch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BatchCreateParams {
    /// Individual Messages API requests to enqueue.
    pub requests: Vec<BatchCreateRequest>,
}

impl BatchCreateParams {
    /// Creates a builder for batch creation parameters.
    pub fn builder() -> BatchCreateParamsBuilder {
        BatchCreateParamsBuilder::default()
    }

    /// Creates batch parameters from a vector of validated requests.
    pub fn new(requests: Vec<BatchCreateRequest>) -> Result<Self, BatchCreateParamsError> {
        if requests.is_empty() {
            return Err(BatchCreateParamsError::EmptyRequests);
        }
        ensure_unique_custom_ids(&requests)?;
        Ok(Self { requests })
    }
}

fn ensure_unique_custom_ids(requests: &[BatchCreateRequest]) -> Result<(), BatchCreateParamsError> {
    let mut seen = HashSet::with_capacity(requests.len());
    for request in requests {
        if !seen.insert(request.custom_id.as_str()) {
            return Err(BatchCreateParamsError::DuplicateCustomId {
                custom_id: request.custom_id.clone(),
            });
        }
    }
    Ok(())
}

/// Builder for [`BatchCreateParams`].
#[derive(Debug, Default, Clone)]
pub struct BatchCreateParamsBuilder {
    requests: Vec<BatchCreateRequest>,
}

impl BatchCreateParamsBuilder {
    /// Adds one batch entry.
    pub fn request(mut self, request: BatchCreateRequest) -> Self {
        self.requests.push(request);
        self
    }

    /// Adds one batch entry from raw parts.
    pub fn add(
        mut self,
        custom_id: impl Into<String>,
        params: MessageCreateParams,
    ) -> Result<Self, BatchCreateParamsError> {
        let request =
            BatchCreateRequest::new(custom_id, params).map_err(BatchCreateParamsError::Request)?;
        self.requests.push(request);
        Ok(self)
    }

    /// Extends the batch with multiple entries.
    pub fn requests(mut self, requests: impl IntoIterator<Item = BatchCreateRequest>) -> Self {
        self.requests.extend(requests);
        self
    }

    /// Builds validated batch creation parameters.
    pub fn build(self) -> Result<BatchCreateParams, BatchCreateParamsError> {
        if self.requests.is_empty() {
            return Err(BatchCreateParamsError::EmptyRequests);
        }
        ensure_unique_custom_ids(&self.requests)?;
        Ok(BatchCreateParams {
            requests: self.requests,
        })
    }
}

/// Errors produced while building [`BatchCreateParams`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BatchCreateParamsError {
    /// At least one request entry is required.
    #[error("batch create params require at least one request")]
    EmptyRequests,
    /// A request entry was invalid.
    #[error(transparent)]
    Request(#[from] BatchCreateRequestError),
    /// Two or more entries share the same custom identifier.
    #[error("duplicate batch request custom_id `{custom_id}`")]
    DuplicateCustomId {
        /// The repeated identifier.
        custom_id: String,
    },
}

/// A deletion confirmation returned by the message batches delete endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletedMessageBatch {
    /// Identifier of the deleted batch.
    pub id: MessageBatchId,
    /// Object type. Always `"message_batch_deleted"`.
    #[serde(rename = "type")]
    pub object_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ApiErrorDetail, ApiErrorType, MessageParam, Model, Role, StopReason, Usage};

    fn message_create_params() -> Result<MessageCreateParams, Box<dyn std::error::Error>> {
        Ok(MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(128)
            .message(MessageParam::user("Hello"))
            .build()?)
    }

    #[test]
    fn batch_create_request_rejects_empty_custom_id() -> Result<(), Box<dyn std::error::Error>> {
        let result = BatchCreateRequest::new("", message_create_params()?);
        assert_eq!(result, Err(BatchCreateRequestError::EmptyCustomId));
        Ok(())
    }

    #[test]
    fn batch_create_request_rejects_streaming_params() -> Result<(), Box<dyn std::error::Error>> {
        let mut params = message_create_params()?;
        params.stream = Some(true);
        let result = BatchCreateRequest::new("a", params);
        assert_eq!(result, Err(BatchCreateRequestError::StreamingNotAllowed));
        Ok(())
    }

    #[test]
    fn batch_create_params_rejects_empty_and_duplicates() -> Result<(), Box<dyn std::error::Error>>
    {
        assert_eq!(
            BatchCreateParams::builder().build(),
            Err(BatchCreateParamsError::EmptyRequests)
        );

        let request = BatchCreateRequest::new("dup", message_create_params()?)?;
        let result = BatchCreateParams::builder()
            .request(request.clone())
            .request(request)
            .build();
        assert_eq!(
            result,
            Err(BatchCreateParamsError::DuplicateCustomId {
                custom_id: "dup".to_owned()
            })
        );
        Ok(())
    }

    #[test]
    fn batch_create_params_serialize_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        let params = BatchCreateParams::builder()
            .add("req-1", message_create_params()?)?
            .add("req-2", message_create_params()?)?
            .build()?;

        let value = serde_json::to_value(&params)?;
        assert_eq!(value["requests"][0]["custom_id"], "req-1");
        assert_eq!(value["requests"][1]["custom_id"], "req-2");
        assert_eq!(value["requests"][0]["params"]["model"], "claude-sonnet-4-5");
        assert_eq!(value["requests"][0]["params"]["max_tokens"], 128);

        let decoded: BatchCreateParams = serde_json::from_value(value)?;
        assert_eq!(decoded, params);
        Ok(())
    }

    #[test]
    fn message_batch_id_rejects_blank_values() {
        assert_eq!(MessageBatchId::try_new(""), Err(MessageBatchIdError::Empty));
        assert_eq!(
            MessageBatchId::try_new(" \t"),
            Err(MessageBatchIdError::Empty)
        );
    }

    #[test]
    fn message_batch_id_serializes_as_string() -> Result<(), Box<dyn std::error::Error>> {
        let id = MessageBatchId::try_new("msgbatch_01")?;
        let value = serde_json::to_value(&id)?;
        assert_eq!(value, "msgbatch_01");

        let decoded: MessageBatchId = serde_json::from_value(value)?;
        assert_eq!(decoded.as_str(), "msgbatch_01");
        Ok(())
    }

    #[test]
    fn message_batch_decodes_in_progress_response() -> Result<(), Box<dyn std::error::Error>> {
        let batch: MessageBatch = serde_json::from_str(
            r#"{
                "id": "msgbatch_01",
                "type": "message_batch",
                "processing_status": "in_progress",
                "request_counts": {
                    "processing": 5,
                    "succeeded": 0,
                    "errored": 0,
                    "canceled": 0,
                    "expired": 0
                },
                "created_at": "2026-04-27T00:00:00Z",
                "expires_at": "2026-04-28T00:00:00Z",
                "ended_at": null,
                "archived_at": null,
                "cancel_initiated_at": null,
                "results_url": null
            }"#,
        )?;

        assert_eq!(batch.id.as_str(), "msgbatch_01");
        assert_eq!(batch.processing_status, BatchProcessingStatus::InProgress);
        assert_eq!(batch.request_counts.processing, 5);
        assert!(batch.results_url.is_none());
        Ok(())
    }

    #[test]
    fn deleted_message_batch_decodes_response() -> Result<(), Box<dyn std::error::Error>> {
        let deleted: DeletedMessageBatch = serde_json::from_str(
            r#"{
                "id": "msgbatch_01",
                "type": "message_batch_deleted"
            }"#,
        )?;

        assert_eq!(deleted.id.as_str(), "msgbatch_01");
        assert_eq!(deleted.object_type, "message_batch_deleted");
        Ok(())
    }

    #[test]
    fn message_batch_result_decodes_all_variants() -> Result<(), Box<dyn std::error::Error>> {
        let succeeded: MessageBatchIndividualResponse = serde_json::from_str(
            r#"{
                "custom_id": "req-1",
                "result": {
                    "type": "succeeded",
                    "message": {
                        "id": "msg_01",
                        "type": "message",
                        "role": "assistant",
                        "model": "claude-sonnet-4-5",
                        "content": [{ "type": "text", "text": "ok" }],
                        "stop_reason": "end_turn",
                        "stop_sequence": null,
                        "usage": { "input_tokens": 1, "output_tokens": 1 }
                    }
                }
            }"#,
        )?;
        match succeeded.result {
            MessageBatchResult::Succeeded { message } => {
                assert_eq!(message.role, Role::Assistant);
                assert_eq!(message.stop_reason, Some(StopReason::EndTurn));
                assert_eq!(
                    message.usage,
                    Usage {
                        input_tokens: 1,
                        output_tokens: 1,
                    }
                );
            }
            other => panic!("expected Succeeded, got {other:?}"),
        }

        let errored: MessageBatchIndividualResponse = serde_json::from_str(
            r#"{
                "custom_id": "req-2",
                "result": {
                    "type": "errored",
                    "error": {
                        "error": {
                            "type": "invalid_request_error",
                            "message": "bad request"
                        }
                    }
                }
            }"#,
        )?;
        match errored.result {
            MessageBatchResult::Errored { error } => {
                assert_eq!(
                    error,
                    ApiErrorBody {
                        error: ApiErrorDetail {
                            error_type: ApiErrorType::InvalidRequest,
                            message: "bad request".to_owned(),
                        }
                    }
                );
            }
            other => panic!("expected Errored, got {other:?}"),
        }

        let canceled: MessageBatchIndividualResponse =
            serde_json::from_str(r#"{ "custom_id": "req-3", "result": { "type": "canceled" } }"#)?;
        assert!(matches!(canceled.result, MessageBatchResult::Canceled));

        let expired: MessageBatchIndividualResponse =
            serde_json::from_str(r#"{ "custom_id": "req-4", "result": { "type": "expired" } }"#)?;
        assert!(matches!(expired.result, MessageBatchResult::Expired));
        Ok(())
    }
}
