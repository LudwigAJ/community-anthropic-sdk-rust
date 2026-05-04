//! Server-Sent Events streaming for the Messages API.
//!
//! [`MessageStream`] is the typed SSE stream returned by
//! [`crate::Messages::create_stream`]; it implements
//! `futures_core::Stream<Item = Result<anthropic_types::MessageStreamEvent, crate::Error>>`
//! and tolerates partial chunks split across byte boundaries, `ping` events,
//! API `error` events, malformed JSON, and early termination. Cancellation
//! is by drop.
//!
//! [`TextStream`] is the text-only convenience returned by
//! [`crate::Messages::create_streaming_text`] for terminal/web-server use.
//!
//! [`MessageStream::final_message`] consumes the stream into a fully
//! accumulated [`anthropic_types::Message`], applying every
//! `message_start`, `content_block_start`, text delta, citation delta,
//! thinking delta, signature delta, redacted-thinking block, tool-use
//! `input_json_delta`, and `message_delta` stop/usage event. Malformed or
//! incomplete tool input JSON surfaces as a stream error at content-block
//! completion rather than producing a silently broken `Message`.

use std::{
    collections::{HashMap, VecDeque},
    pin::Pin,
    task::{Context, Poll},
};

use anthropic_types::{
    ApiErrorBody, ContentBlock, ContentBlockDelta, Message, MessageStreamEvent, RequestId,
};
use bytes::Bytes;
use futures_util::{Stream, StreamExt};
use reqwest::StatusCode;

use crate::{ApiError, Error};

type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>;

/// Stream of Messages API SSE events.
///
/// Dropping this value cancels the underlying HTTP response body.
pub struct MessageStream {
    inner: ByteStream,
    buffer: Vec<u8>,
    pending: VecDeque<Result<MessageStreamEvent, Error>>,
    finished: bool,
    request_id: Option<RequestId>,
    status: StatusCode,
}

impl MessageStream {
    pub(crate) fn from_response(response: reqwest::Response) -> Self {
        let status = response.status();
        let request_id = crate::transport::response_request_id(&response);
        Self {
            inner: Box::pin(response.bytes_stream()),
            buffer: Vec::new(),
            pending: VecDeque::new(),
            finished: false,
            request_id,
            status,
        }
    }

    /// Returns the request ID from the stream setup response, when present.
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_ref().map(RequestId::as_str)
    }

    /// Consumes the stream and returns the final accumulated message.
    ///
    /// This helper accumulates `message_start`, `content_block_start`,
    /// text, thinking, thinking signature, and tool-use input JSON
    /// `content_block_delta`, redacted thinking `content_block_start`, and
    /// `message_delta` stop and usage fields.
    /// Unsupported delta types are reported as stream errors instead of being
    /// guessed into an incomplete final message.
    pub async fn final_message(mut self) -> Result<Message, Error> {
        let mut accumulator = MessageAccumulator::default();

        while let Some(event) = self.next().await {
            let event = event?;
            let is_stop = matches!(event, MessageStreamEvent::MessageStop);
            accumulator.push(event)?;
            if is_stop {
                return accumulator.finish();
            }
        }

        accumulator.finish_after_eof()
    }

    fn drain_complete_frames(&mut self) {
        while let Some((frame_end, boundary_len)) = find_frame_boundary(&self.buffer) {
            let mut frame = self
                .buffer
                .drain(..frame_end + boundary_len)
                .collect::<Vec<_>>();
            frame.truncate(frame_end);
            self.push_frame(frame);
            if self.finished {
                break;
            }
        }
    }

    fn drain_final_frame(&mut self) {
        if self.buffer.is_empty() {
            return;
        }

        let frame = std::mem::take(&mut self.buffer);
        self.push_frame(frame);
    }

    fn push_frame(&mut self, frame: Vec<u8>) {
        let parsed = match parse_sse_frame(&frame) {
            Ok(Some(event)) => self.decode_sse_event(event),
            Ok(None) => Ok(None),
            Err(error) => Err(error),
        };

        match parsed {
            Ok(Some(event)) => self.pending.push_back(Ok(event)),
            Ok(None) => {}
            Err(error) => {
                self.pending.push_back(Err(error));
                self.buffer.clear();
                self.finished = true;
            }
        }
    }

    fn decode_sse_event(&self, event: SseEvent) -> Result<Option<MessageStreamEvent>, Error> {
        let event_name = event.event.unwrap_or_else(|| "message".to_owned());
        if event_name == "error" {
            return Err(self.api_error_from_stream_data(&event.data));
        }

        if event.data.trim() == "[DONE]" {
            return Ok(Some(MessageStreamEvent::MessageStop));
        }

        if event.data.trim().is_empty() {
            if event_name == "ping" {
                return Ok(Some(MessageStreamEvent::Ping));
            }
            return Ok(None);
        }

        let mut data = serde_json::from_str::<serde_json::Value>(&event.data)
            .map_err(|source| Error::Json { source })?;
        if data.get("type").is_none() && is_known_stream_event_name(&event_name) {
            if let Some(object) = data.as_object_mut() {
                object.insert(
                    "type".to_owned(),
                    serde_json::Value::String(event_name.clone()),
                );
            }
        }
        match serde_json::from_value::<MessageStreamEvent>(data.clone()) {
            Ok(MessageStreamEvent::Error { error }) => {
                let body = ApiErrorBody { error };
                Err(ApiError::from_response_parts(
                    self.status,
                    self.request_id.clone(),
                    Some(body),
                    Some(event.data),
                )
                .into())
            }
            Ok(event) => Ok(Some(event)),
            Err(_) => Ok(Some(MessageStreamEvent::Other {
                event: event_name,
                data,
            })),
        }
    }

    fn api_error_from_stream_data(&self, data: &str) -> Error {
        let body = serde_json::from_str::<ApiErrorBody>(data).ok();
        ApiError::from_response_parts(
            self.status,
            self.request_id.clone(),
            body,
            Some(data.to_owned()),
        )
        .into()
    }
}

#[derive(Default)]
struct MessageAccumulator {
    message: Option<Message>,
    tool_input_json: HashMap<usize, String>,
}

impl MessageAccumulator {
    fn push(&mut self, event: MessageStreamEvent) -> Result<(), Error> {
        match event {
            MessageStreamEvent::MessageStart { message } => {
                if self.message.is_some() {
                    return Err(stream_error(
                        "unexpected message_start before previous message_stop",
                    ));
                }
                self.message = Some(message);
                Ok(())
            }
            MessageStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                let message = self.message_mut("content_block_start")?;
                let index = content_index(index)?;
                if index != message.content.len() {
                    return Err(stream_error(format!(
                        "unexpected content_block_start index {index}; next content block index is {}",
                        message.content.len()
                    )));
                }

                message.content.push(content_block);
                Ok(())
            }
            MessageStreamEvent::ContentBlockDelta { index, delta } => {
                let message = self
                    .message
                    .as_mut()
                    .ok_or_else(|| missing_message_start_error("content_block_delta"))?;
                let index = content_index(index)?;
                let content_block = message.content.get_mut(index).ok_or_else(|| {
                    stream_error(format!(
                        "received content_block_delta for missing content block index {index}"
                    ))
                })?;

                accumulate_content_delta(content_block, index, delta, &mut self.tool_input_json)
            }
            MessageStreamEvent::ContentBlockStop { index } => {
                let message = self
                    .message
                    .as_mut()
                    .ok_or_else(|| missing_message_start_error("content_block_stop"))?;
                let index = content_index(index)?;
                let content_block = message.content.get_mut(index).ok_or_else(|| {
                    stream_error(format!(
                        "received content_block_stop for missing content block index {index}"
                    ))
                })?;

                finish_content_block(content_block, index, &mut self.tool_input_json)?;
                Ok(())
            }
            MessageStreamEvent::MessageDelta { delta, usage } => {
                let message = self.message_mut("message_delta")?;
                message.stop_reason = delta.stop_reason;
                message.stop_sequence = delta.stop_sequence;
                if let Some(usage) = usage {
                    usage.apply_to(&mut message.usage);
                }
                Ok(())
            }
            MessageStreamEvent::MessageStop => {
                self.message_mut("message_stop")?;
                Ok(())
            }
            MessageStreamEvent::Ping | MessageStreamEvent::Other { .. } => Ok(()),
            MessageStreamEvent::Error { error } => Err(stream_error(format!(
                "stream emitted API error event: {}",
                error.message
            ))),
        }
    }

    fn message_mut(&mut self, event_type: &'static str) -> Result<&mut Message, Error> {
        self.message
            .as_mut()
            .ok_or_else(|| missing_message_start_error(event_type))
    }

    fn finish(self) -> Result<Message, Error> {
        self.message
            .ok_or_else(|| stream_error("stream ended without producing a message_start event"))
    }

    fn finish_after_eof(self) -> Result<Message, Error> {
        if self.message.is_some() {
            Err(stream_error(
                "stream ended before receiving a message_stop event",
            ))
        } else {
            Err(stream_error(
                "stream ended without producing a message_start event",
            ))
        }
    }
}

fn accumulate_content_delta(
    content_block: &mut ContentBlock,
    index: usize,
    delta: ContentBlockDelta,
    tool_input_json: &mut HashMap<usize, String>,
) -> Result<(), Error> {
    match delta {
        ContentBlockDelta::Text { text: delta_text } => match content_block {
            ContentBlock::Text { text, .. } => {
                text.push_str(&delta_text);
                Ok(())
            }
            _ => Err(stream_error(
                "received text_delta for a non-text content block",
            )),
        },
        ContentBlockDelta::InputJson { partial_json } => match content_block {
            ContentBlock::ToolUse { input, .. } => {
                if partial_json.is_empty() {
                    return Ok(());
                }

                let json = tool_input_json.entry(index).or_default();
                json.push_str(&partial_json);

                if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
                    *input = value;
                }
                Ok(())
            }
            _ => Err(stream_error(
                "received input_json_delta for a non-tool-use content block",
            )),
        },
        ContentBlockDelta::Thinking {
            thinking: delta_thinking,
        } => match content_block {
            ContentBlock::Thinking { thinking, .. } => {
                thinking.push_str(&delta_thinking);
                Ok(())
            }
            _ => Err(stream_error(
                "received thinking_delta for a non-thinking content block",
            )),
        },
        ContentBlockDelta::Signature {
            signature: delta_signature,
        } => match content_block {
            ContentBlock::Thinking { signature, .. } => {
                *signature = Some(delta_signature);
                Ok(())
            }
            _ => Err(stream_error(
                "received signature_delta for a non-thinking content block",
            )),
        },
        ContentBlockDelta::Citations { citation } => match content_block {
            ContentBlock::Text { citations, .. } => {
                citations.get_or_insert_with(Vec::new).push(citation);
                Ok(())
            }
            _ => Err(stream_error(
                "received citations_delta for a non-text content block",
            )),
        },
    }
}

fn finish_content_block(
    content_block: &mut ContentBlock,
    index: usize,
    tool_input_json: &mut HashMap<usize, String>,
) -> Result<(), Error> {
    let Some(json) = tool_input_json.remove(&index) else {
        return Ok(());
    };

    match content_block {
        ContentBlock::ToolUse { input, .. } => {
            let value = serde_json::from_str::<serde_json::Value>(&json).map_err(|source| {
                stream_error(format!(
                    "tool-use input_json_delta for content block index {index} did not produce valid JSON: {source}"
                ))
            })?;
            *input = value;
            Ok(())
        }
        _ => Err(stream_error(
            "received accumulated tool input JSON for a non-tool-use content block",
        )),
    }
}

fn content_index(index: u32) -> Result<usize, Error> {
    usize::try_from(index)
        .map_err(|_| stream_error(format!("content block index {index} is too large")))
}

fn stream_error(message: impl Into<String>) -> Error {
    Error::Stream {
        message: message.into(),
    }
}

fn missing_message_start_error(event_type: &'static str) -> Error {
    stream_error(format!(
        "unexpected {event_type} before receiving message_start"
    ))
}

fn is_known_stream_event_name(event_name: &str) -> bool {
    matches!(
        event_name,
        "message_start"
            | "content_block_start"
            | "content_block_delta"
            | "content_block_stop"
            | "message_delta"
            | "message_stop"
            | "ping"
            | "error"
    )
}

impl std::fmt::Debug for MessageStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MessageStream")
            .field("request_id", &self.request_id)
            .field("status", &self.status)
            .field("buffered_bytes", &self.buffer.len())
            .field("pending_events", &self.pending.len())
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl Stream for MessageStream {
    type Item = Result<MessageStreamEvent, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            if let Some(item) = this.pending.pop_front() {
                return Poll::Ready(Some(item));
            }

            if this.finished {
                return Poll::Ready(None);
            }

            match this.inner.as_mut().poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(chunk))) => {
                    this.buffer.extend_from_slice(&chunk);
                    this.drain_complete_frames();
                }
                Poll::Ready(Some(Err(source))) => {
                    this.finished = true;
                    return Poll::Ready(Some(Err(Error::Transport { source })));
                }
                Poll::Ready(None) => {
                    this.finished = true;
                    this.drain_final_frame();
                }
            }
        }
    }
}

/// Text-only stream of appended Messages API text deltas.
///
/// This stream wraps [`MessageStream`], yields only `text_delta` content as
/// owned string chunks, ignores non-text events, propagates stream errors, and
/// ends when the underlying message emits `message_stop`.
pub struct TextStream {
    inner: MessageStream,
    finished: bool,
}

impl TextStream {
    pub(crate) fn new(inner: MessageStream) -> Self {
        Self {
            inner,
            finished: false,
        }
    }

    /// Returns the request ID from the stream setup response, when present.
    pub fn request_id(&self) -> Option<&str> {
        self.inner.request_id()
    }
}

impl std::fmt::Debug for TextStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextStream")
            .field("request_id", &self.request_id())
            .field("finished", &self.finished)
            .finish_non_exhaustive()
    }
}

impl Stream for TextStream {
    type Item = Result<String, Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        if this.finished {
            return Poll::Ready(None);
        }

        loop {
            match Pin::new(&mut this.inner).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(MessageStreamEvent::ContentBlockDelta { delta, .. }))) => {
                    if let Some(text) = delta.text() {
                        return Poll::Ready(Some(Ok(text.to_owned())));
                    }
                }
                Poll::Ready(Some(Ok(MessageStreamEvent::MessageStop))) | Poll::Ready(None) => {
                    this.finished = true;
                    return Poll::Ready(None);
                }
                Poll::Ready(Some(Ok(_))) => {}
                Poll::Ready(Some(Err(error))) => return Poll::Ready(Some(Err(error))),
            }
        }
    }
}

struct SseEvent {
    event: Option<String>,
    data: String,
}

fn parse_sse_frame(frame: &[u8]) -> Result<Option<SseEvent>, Error> {
    let text = std::str::from_utf8(frame).map_err(|source| Error::Stream {
        message: format!("stream event was not valid UTF-8: {source}"),
    })?;

    let mut event = None;
    let mut data = Vec::new();

    for line in text.lines().map(|line| line.trim_end_matches('\r')) {
        if line.is_empty() || line.starts_with(':') {
            continue;
        }

        let (field, value) = line
            .split_once(':')
            .map_or((line, ""), |(field, value)| (field, value));
        let value = value.strip_prefix(' ').unwrap_or(value);

        match field {
            "event" => event = Some(value.to_owned()),
            "data" => data.push(value.to_owned()),
            _ => {}
        }
    }

    if event.is_none() && data.is_empty() {
        return Ok(None);
    }

    Ok(Some(SseEvent {
        event,
        data: data.join("\n"),
    }))
}

fn find_frame_boundary(buffer: &[u8]) -> Option<(usize, usize)> {
    [
        b"\r\n\r\n".as_slice(),
        b"\n\n".as_slice(),
        b"\r\r".as_slice(),
    ]
    .into_iter()
    .filter_map(|pattern| {
        buffer
            .windows(pattern.len())
            .position(|window| window == pattern)
            .map(|position| (position, pattern.len()))
    })
    .min_by_key(|(position, _)| *position)
}

#[cfg(test)]
mod tests {
    use futures_util::{StreamExt, stream};

    use super::*;
    use crate::ApiErrorKind;
    use anthropic_types::{ApiErrorType, ContentBlockDelta, TextCitation};

    fn stream_from_chunks(chunks: impl IntoIterator<Item = &'static [u8]>) -> MessageStream {
        let chunks = chunks
            .into_iter()
            .map(|chunk| Ok::<Bytes, reqwest::Error>(Bytes::from_static(chunk)))
            .collect::<Vec<_>>();
        MessageStream {
            inner: Box::pin(stream::iter(chunks)),
            buffer: Vec::new(),
            pending: VecDeque::new(),
            finished: false,
            request_id: RequestId::try_new("req_stream").ok(),
            status: StatusCode::OK,
        }
    }

    fn stream_from_owned_chunks(chunks: impl IntoIterator<Item = Vec<u8>>) -> MessageStream {
        let chunks = chunks
            .into_iter()
            .map(|chunk| Ok::<Bytes, reqwest::Error>(Bytes::from(chunk)))
            .collect::<Vec<_>>();
        MessageStream {
            inner: Box::pin(stream::iter(chunks)),
            buffer: Vec::new(),
            pending: VecDeque::new(),
            finished: false,
            request_id: RequestId::try_new("req_stream").ok(),
            status: StatusCode::OK,
        }
    }

    fn sse_frame(event: &str, data: serde_json::Value) -> Vec<u8> {
        format!("event: {event}\ndata: {data}\n\n").into_bytes()
    }

    fn message_start_frame() -> &'static [u8] {
        b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_01\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude-sonnet-4-5\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"input_tokens\":3,\"output_tokens\":0}}}\n\n"
    }

    fn text_block_start_frame() -> &'static [u8] {
        b"event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n"
    }

    fn message_stop_frame() -> &'static [u8] {
        b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n"
    }

    fn tool_block_start_frame(index: u32) -> Vec<u8> {
        sse_frame(
            "content_block_start",
            serde_json::json!({
                "type": "content_block_start",
                "index": index,
                "content_block": {
                    "type": "tool_use",
                    "id": "toolu_01",
                    "name": "get_weather",
                    "input": {}
                }
            }),
        )
    }

    fn thinking_block_start_frame(index: u32, thinking: &str) -> Vec<u8> {
        sse_frame(
            "content_block_start",
            serde_json::json!({
                "type": "content_block_start",
                "index": index,
                "content_block": {
                    "type": "thinking",
                    "thinking": thinking
                }
            }),
        )
    }

    fn redacted_thinking_block_start_frame(index: u32, data: &str) -> Vec<u8> {
        sse_frame(
            "content_block_start",
            serde_json::json!({
                "type": "content_block_start",
                "index": index,
                "content_block": {
                    "type": "redacted_thinking",
                    "data": data
                }
            }),
        )
    }

    fn thinking_delta_frame(index: u32, thinking: &str) -> Vec<u8> {
        sse_frame(
            "content_block_delta",
            serde_json::json!({
                "type": "content_block_delta",
                "index": index,
                "delta": {
                    "type": "thinking_delta",
                    "thinking": thinking
                }
            }),
        )
    }

    fn signature_delta_frame(index: u32, signature: &str) -> Vec<u8> {
        sse_frame(
            "content_block_delta",
            serde_json::json!({
                "type": "content_block_delta",
                "index": index,
                "delta": {
                    "type": "signature_delta",
                    "signature": signature
                }
            }),
        )
    }

    fn citations_delta_frame(index: u32) -> Vec<u8> {
        sse_frame(
            "content_block_delta",
            serde_json::json!({
                "type": "content_block_delta",
                "index": index,
                "delta": {
                    "type": "citations_delta",
                    "citation": {
                        "type": "page_location",
                        "cited_text": "Revenue increased.",
                        "document_index": 0,
                        "document_title": "Quarterly report",
                        "start_page_number": 2,
                        "end_page_number": 3,
                        "file_id": null
                    }
                }
            }),
        )
    }

    fn input_json_delta_frame(index: u32, partial_json: &str) -> Vec<u8> {
        sse_frame(
            "content_block_delta",
            serde_json::json!({
                "type": "content_block_delta",
                "index": index,
                "delta": {
                    "type": "input_json_delta",
                    "partial_json": partial_json
                }
            }),
        )
    }

    fn content_block_stop_frame(index: u32) -> Vec<u8> {
        sse_frame(
            "content_block_stop",
            serde_json::json!({
                "type": "content_block_stop",
                "index": index
            }),
        )
    }

    #[tokio::test]
    async fn parses_events_split_across_byte_chunks() -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = stream_from_chunks([
            b"event: content_block_delta\ndata: {\"type\":\"content_block_" as &'static [u8],
            b"delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"hel",
            b"lo\"}}\n\n",
        ]);

        let event = stream
            .next()
            .await
            .ok_or_else(|| std::io::Error::other("expected stream event"))??;

        assert_eq!(
            event,
            MessageStreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentBlockDelta::Text {
                    text: "hello".to_owned()
                }
            }
        );
        assert!(stream.next().await.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn parses_known_sse_event_name_when_payload_type_is_omitted()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = stream_from_chunks([
            b"event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hi\"}}\n\n"
                as &'static [u8],
        ]);

        let event = stream
            .next()
            .await
            .ok_or_else(|| std::io::Error::other("expected stream event"))??;

        assert_eq!(
            event,
            MessageStreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentBlockDelta::Text {
                    text: "Hi".to_owned()
                },
            }
        );
        assert!(stream.next().await.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn parses_done_sentinel_as_message_stop() -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = stream_from_chunks([b"data: [DONE]\n\n" as &'static [u8]]);

        assert_eq!(
            stream.next().await.transpose()?,
            Some(MessageStreamEvent::MessageStop)
        );
        assert!(stream.next().await.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn maps_error_events_to_api_errors() -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = stream_from_chunks([b"event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n" as &'static [u8]]);

        match stream
            .next()
            .await
            .ok_or_else(|| std::io::Error::other("expected stream error"))?
        {
            Err(Error::Api(api_error)) => {
                assert_eq!(api_error.status, StatusCode::OK);
                assert_eq!(api_error.request_id(), Some("req_stream"));
                assert_eq!(api_error.kind, ApiErrorKind::Overloaded);
                assert_eq!(api_error.message, "Overloaded");
                assert_eq!(
                    api_error
                        .body
                        .as_ref()
                        .map(|body| body.error.error_type.clone()),
                    Some(ApiErrorType::Overloaded)
                );
            }
            other => {
                return Err(std::io::Error::other(format!(
                    "expected API stream error, got {other:?}"
                ))
                .into());
            }
        }

        assert!(stream.next().await.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn final_message_accumulates_text_content() -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_chunks([
            message_start_frame(),
            text_block_start_frame(),
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n" as &'static [u8],
            message_stop_frame(),
        ]);

        let message = stream.final_message().await?;

        assert_eq!(message.id, "msg_01");
        assert_eq!(message.content, vec![ContentBlock::text("Hello")]);
        Ok(())
    }

    #[tokio::test]
    async fn final_message_preserves_multiple_text_chunks_in_order()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_chunks([
            message_start_frame(),
            text_block_start_frame(),
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}\n\n" as &'static [u8],
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"lo\"}}\n\n",
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\" there\"}}\n\n",
            message_stop_frame(),
        ]);

        let message = stream.final_message().await?;

        assert_eq!(message.content, vec![ContentBlock::text("Hello there")]);
        Ok(())
    }

    #[tokio::test]
    async fn final_message_accumulates_text_citations() -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_owned_chunks([
            message_start_frame().to_vec(),
            text_block_start_frame().to_vec(),
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"See the report.\"}}\n\n".to_vec(),
            citations_delta_frame(0),
            message_stop_frame().to_vec(),
        ]);

        let message = stream.final_message().await?;

        assert_eq!(
            message.content,
            vec![ContentBlock::Text {
                text: "See the report.".to_owned(),
                citations: Some(vec![TextCitation::Page {
                    cited_text: "Revenue increased.".to_owned(),
                    document_index: 0,
                    document_title: Some("Quarterly report".to_owned()),
                    end_page_number: 3,
                    file_id: None,
                    start_page_number: 2,
                }]),
            }]
        );
        Ok(())
    }

    #[tokio::test]
    async fn final_message_errors_on_citations_delta_for_non_text_block()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_owned_chunks([
            message_start_frame().to_vec(),
            thinking_block_start_frame(0, "Thinking."),
            citations_delta_frame(0),
            message_stop_frame().to_vec(),
        ]);

        match stream.final_message().await {
            Err(Error::Stream { message }) => {
                assert!(message.contains("non-text content block"));
                Ok(())
            }
            other => Err(std::io::Error::other(format!("expected Stream, got {other:?}")).into()),
        }
    }

    #[tokio::test]
    async fn final_message_accumulates_thinking_content() -> Result<(), Box<dyn std::error::Error>>
    {
        let stream = stream_from_owned_chunks([
            message_start_frame().to_vec(),
            thinking_block_start_frame(0, "Let me think. "),
            thinking_delta_frame(0, "First, check the facts. "),
            thinking_delta_frame(0, "Then answer."),
            content_block_stop_frame(0),
            message_stop_frame().to_vec(),
        ]);

        let message = stream.final_message().await?;

        assert_eq!(
            message.content,
            vec![ContentBlock::Thinking {
                thinking: "Let me think. First, check the facts. Then answer.".to_owned(),
                signature: None,
            }]
        );
        Ok(())
    }

    #[tokio::test]
    async fn final_message_accumulates_thinking_signature() -> Result<(), Box<dyn std::error::Error>>
    {
        let stream = stream_from_owned_chunks([
            message_start_frame().to_vec(),
            thinking_block_start_frame(0, "Signed thinking."),
            signature_delta_frame(0, "ThinkingSignature"),
            content_block_stop_frame(0),
            message_stop_frame().to_vec(),
        ]);

        let message = stream.final_message().await?;

        assert_eq!(
            message.content,
            vec![ContentBlock::Thinking {
                thinking: "Signed thinking.".to_owned(),
                signature: Some("ThinkingSignature".to_owned()),
            }]
        );
        Ok(())
    }

    #[tokio::test]
    async fn final_message_accumulates_thinking_content_and_signature()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_owned_chunks([
            message_start_frame().to_vec(),
            thinking_block_start_frame(0, "Let me think. "),
            thinking_delta_frame(0, "First, check the facts. "),
            signature_delta_frame(0, "ThinkingSignature"),
            content_block_stop_frame(0),
            message_stop_frame().to_vec(),
        ]);

        let message = stream.final_message().await?;

        assert_eq!(
            message.content,
            vec![ContentBlock::Thinking {
                thinking: "Let me think. First, check the facts. ".to_owned(),
                signature: Some("ThinkingSignature".to_owned()),
            }]
        );
        Ok(())
    }

    #[tokio::test]
    async fn final_message_preserves_redacted_thinking_block()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_owned_chunks([
            message_start_frame().to_vec(),
            redacted_thinking_block_start_frame(0, "Redacted"),
            content_block_stop_frame(0),
            message_stop_frame().to_vec(),
        ]);

        let message = stream.final_message().await?;

        assert_eq!(
            message.content,
            vec![ContentBlock::redacted_thinking("Redacted")]
        );
        Ok(())
    }

    #[tokio::test]
    async fn final_message_errors_on_thinking_delta_for_redacted_thinking_block()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_owned_chunks([
            message_start_frame().to_vec(),
            redacted_thinking_block_start_frame(0, "Redacted"),
            thinking_delta_frame(0, "not visible thinking"),
            message_stop_frame().to_vec(),
        ]);

        match stream.final_message().await {
            Err(Error::Stream { message }) => {
                assert!(message.contains("non-thinking content block"));
                Ok(())
            }
            other => Err(std::io::Error::other(format!("expected Stream, got {other:?}")).into()),
        }
    }

    #[tokio::test]
    async fn final_message_errors_on_thinking_delta_for_non_thinking_block()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_owned_chunks([
            message_start_frame().to_vec(),
            text_block_start_frame().to_vec(),
            thinking_delta_frame(0, "not text"),
            message_stop_frame().to_vec(),
        ]);

        match stream.final_message().await {
            Err(Error::Stream { message }) => {
                assert!(message.contains("non-thinking content block"));
                Ok(())
            }
            other => Err(std::io::Error::other(format!("expected Stream, got {other:?}")).into()),
        }
    }

    #[tokio::test]
    async fn final_message_errors_on_signature_delta_for_non_thinking_block()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_owned_chunks([
            message_start_frame().to_vec(),
            text_block_start_frame().to_vec(),
            signature_delta_frame(0, "ThinkingSignature"),
            message_stop_frame().to_vec(),
        ]);

        match stream.final_message().await {
            Err(Error::Stream { message }) => {
                assert!(message.contains("non-thinking content block"));
                Ok(())
            }
            other => Err(std::io::Error::other(format!("expected Stream, got {other:?}")).into()),
        }
    }

    #[tokio::test]
    async fn final_message_accumulates_tool_use_input_json()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_owned_chunks([
            message_start_frame().to_vec(),
            tool_block_start_frame(0),
            input_json_delta_frame(0, "{\"city"),
            input_json_delta_frame(0, "\": \"San Francisco\", \"units\": \"metric\"}"),
            content_block_stop_frame(0),
            message_stop_frame().to_vec(),
        ]);

        let message = stream.final_message().await?;

        assert_eq!(
            message.content,
            vec![ContentBlock::ToolUse {
                id: "toolu_01".to_owned(),
                name: "get_weather".to_owned(),
                input: serde_json::json!({
                    "city": "San Francisco",
                    "units": "metric"
                }),
            }]
        );
        Ok(())
    }

    #[tokio::test]
    async fn final_message_allows_empty_initial_tool_input_json_delta()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_owned_chunks([
            message_start_frame().to_vec(),
            tool_block_start_frame(0),
            input_json_delta_frame(0, ""),
            input_json_delta_frame(0, "{\"city\": \"London\"}"),
            content_block_stop_frame(0),
            message_stop_frame().to_vec(),
        ]);

        let message = stream.final_message().await?;

        assert_eq!(
            message.content,
            vec![ContentBlock::ToolUse {
                id: "toolu_01".to_owned(),
                name: "get_weather".to_owned(),
                input: serde_json::json!({ "city": "London" }),
            }]
        );
        Ok(())
    }

    #[tokio::test]
    async fn final_message_errors_on_incomplete_tool_input_json_at_block_stop()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_owned_chunks([
            message_start_frame().to_vec(),
            tool_block_start_frame(0),
            input_json_delta_frame(0, "{\"city\""),
            content_block_stop_frame(0),
            message_stop_frame().to_vec(),
        ]);

        match stream.final_message().await {
            Err(Error::Stream { message }) => {
                assert!(message.contains("did not produce valid JSON"));
                Ok(())
            }
            other => Err(std::io::Error::other(format!("expected Stream, got {other:?}")).into()),
        }
    }

    #[tokio::test]
    async fn final_message_errors_on_input_json_delta_for_non_tool_use_block()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_owned_chunks([
            message_start_frame().to_vec(),
            text_block_start_frame().to_vec(),
            input_json_delta_frame(0, "{\"city\": \"London\"}"),
            message_stop_frame().to_vec(),
        ]);

        match stream.final_message().await {
            Err(Error::Stream { message }) => {
                assert!(message.contains("non-tool-use content block"));
                Ok(())
            }
            other => Err(std::io::Error::other(format!("expected Stream, got {other:?}")).into()),
        }
    }

    #[tokio::test]
    async fn final_message_accumulates_text_and_tool_use_blocks()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_owned_chunks([
            message_start_frame().to_vec(),
            text_block_start_frame().to_vec(),
            sse_frame(
                "content_block_delta",
                serde_json::json!({
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": {
                        "type": "text_delta",
                        "text": "Checking."
                    }
                }),
            ),
            content_block_stop_frame(0),
            tool_block_start_frame(1),
            input_json_delta_frame(1, "{\"city\": \"Paris\"}"),
            content_block_stop_frame(1),
            message_stop_frame().to_vec(),
        ]);

        let message = stream.final_message().await?;

        assert_eq!(
            message.content,
            vec![
                ContentBlock::text("Checking."),
                ContentBlock::ToolUse {
                    id: "toolu_01".to_owned(),
                    name: "get_weather".to_owned(),
                    input: serde_json::json!({ "city": "Paris" }),
                }
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn final_message_accumulates_text_tool_use_and_thinking_blocks()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_owned_chunks([
            message_start_frame().to_vec(),
            text_block_start_frame().to_vec(),
            sse_frame(
                "content_block_delta",
                serde_json::json!({
                    "type": "content_block_delta",
                    "index": 0,
                    "delta": {
                        "type": "text_delta",
                        "text": "Checking."
                    }
                }),
            ),
            content_block_stop_frame(0),
            thinking_block_start_frame(1, ""),
            thinking_delta_frame(1, "I should call a tool."),
            signature_delta_frame(1, "MixedThinkingSignature"),
            content_block_stop_frame(1),
            redacted_thinking_block_start_frame(2, "Redacted"),
            content_block_stop_frame(2),
            tool_block_start_frame(3),
            input_json_delta_frame(3, "{\"city\": \"Paris\"}"),
            content_block_stop_frame(3),
            message_stop_frame().to_vec(),
        ]);

        let message = stream.final_message().await?;

        assert_eq!(
            message.content,
            vec![
                ContentBlock::text("Checking."),
                ContentBlock::Thinking {
                    thinking: "I should call a tool.".to_owned(),
                    signature: Some("MixedThinkingSignature".to_owned()),
                },
                ContentBlock::redacted_thinking("Redacted"),
                ContentBlock::ToolUse {
                    id: "toolu_01".to_owned(),
                    name: "get_weather".to_owned(),
                    input: serde_json::json!({ "city": "Paris" }),
                }
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn final_message_applies_message_delta_stop_fields_and_usage()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_chunks([
            message_start_frame(),
            text_block_start_frame(),
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Done\"}}\n\n" as &'static [u8],
            b"event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":\"END\"},\"usage\":{\"input_tokens\":3,\"output_tokens\":6}}\n\n",
            message_stop_frame(),
        ]);

        let message = stream.final_message().await?;

        assert_eq!(
            message.stop_reason,
            Some(anthropic_types::StopReason::EndTurn)
        );
        assert_eq!(message.stop_sequence.as_deref(), Some("END"));
        assert_eq!(message.usage, anthropic_types::Usage::new(3, 6));
        Ok(())
    }

    #[tokio::test]
    async fn final_message_merges_optional_stream_usage_fields()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_chunks([
            b"event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"id\":\"msg_01\",\"type\":\"message\",\"role\":\"assistant\",\"model\":\"claude-sonnet-4-5\",\"content\":[],\"stop_reason\":null,\"stop_sequence\":null,\"usage\":{\"cache_creation_input_tokens\":5,\"cache_read_input_tokens\":13,\"input_tokens\":3,\"output_tokens\":0}}}\n\n" as &'static [u8],
            b"event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},\"usage\":{\"output_tokens\":6}}\n\n",
            message_stop_frame(),
        ]);

        let message = stream.final_message().await?;

        assert_eq!(message.usage.input_tokens, 3);
        assert_eq!(message.usage.cache_creation_input_tokens, Some(5));
        assert_eq!(message.usage.cache_read_input_tokens, Some(13));
        assert_eq!(message.usage.output_tokens, 6);
        assert_eq!(message.usage.total_input_tokens(), 21);
        Ok(())
    }

    #[tokio::test]
    async fn final_message_applies_pause_turn_stop_reason() -> Result<(), Box<dyn std::error::Error>>
    {
        let stream = stream_from_chunks([
            message_start_frame(),
            b"event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":\"pause_turn\",\"stop_sequence\":null},\"usage\":{\"input_tokens\":3,\"output_tokens\":6}}\n\n" as &'static [u8],
            message_stop_frame(),
        ]);

        let message = stream.final_message().await?;

        assert_eq!(
            message.stop_reason,
            Some(anthropic_types::StopReason::PauseTurn)
        );
        Ok(())
    }

    #[tokio::test]
    async fn final_message_propagates_stream_errors() -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_chunks([
            message_start_frame(),
            b"event: content_block_delta\ndata: {not json}\n\n" as &'static [u8],
        ]);

        match stream.final_message().await {
            Err(Error::Json { .. }) => Ok(()),
            other => Err(std::io::Error::other(format!("expected Json, got {other:?}")).into()),
        }
    }

    #[tokio::test]
    async fn final_message_errors_when_message_start_is_missing()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_chunks([
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"orphan\"}}\n\n" as &'static [u8],
        ]);

        match stream.final_message().await {
            Err(Error::Stream { message }) => {
                assert!(message.contains("before receiving message_start"));
                Ok(())
            }
            other => Err(std::io::Error::other(format!("expected Stream, got {other:?}")).into()),
        }
    }

    #[tokio::test]
    async fn final_message_errors_on_malformed_content_sequence()
    -> Result<(), Box<dyn std::error::Error>> {
        let stream = stream_from_chunks([
            message_start_frame(),
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"missing block start\"}}\n\n" as &'static [u8],
        ]);

        match stream.final_message().await {
            Err(Error::Stream { message }) => {
                assert!(message.contains("missing content block index 0"));
                Ok(())
            }
            other => Err(std::io::Error::other(format!("expected Stream, got {other:?}")).into()),
        }
    }

    #[tokio::test]
    async fn text_stream_yields_text_chunks_in_order() -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = TextStream::new(stream_from_chunks([
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Hel\"}}\n\n" as &'static [u8],
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"lo\"}}\n\n",
            b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        ]));

        assert_eq!(stream.request_id(), Some("req_stream"));
        assert_eq!(stream.next().await.transpose()?, Some("Hel".to_owned()));
        assert_eq!(stream.next().await.transpose()?, Some("lo".to_owned()));
        assert!(stream.next().await.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn text_stream_ignores_non_text_events() -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = TextStream::new(stream_from_chunks([
            b"event: ping\ndata: {\"type\":\"ping\"}\n\n" as &'static [u8],
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"city\"}}\n\n",
            b"event: message_delta\ndata: {\"type\":\"message_delta\",\"delta\":{\"stop_reason\":null,\"stop_sequence\":null},\"usage\":{\"input_tokens\":1,\"output_tokens\":2}}\n\n",
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"visible\"}}\n\n",
            b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        ]));

        assert_eq!(stream.next().await.transpose()?, Some("visible".to_owned()));
        assert!(stream.next().await.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn text_stream_propagates_underlying_stream_errors()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = TextStream::new(stream_from_chunks([
            b"event: content_block_delta\ndata: {not json}\n\n" as &'static [u8],
        ]));

        match stream
            .next()
            .await
            .ok_or_else(|| std::io::Error::other("expected stream error"))?
        {
            Err(Error::Json { .. }) => {}
            other => {
                return Err(std::io::Error::other(format!("expected Json, got {other:?}")).into());
            }
        }
        assert!(stream.next().await.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn text_stream_stops_on_message_stop() -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = TextStream::new(stream_from_chunks([
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"before\"}}\n\n" as &'static [u8],
            b"event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
            b"event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"after\"}}\n\n",
        ]));

        assert_eq!(stream.next().await.transpose()?, Some("before".to_owned()));
        assert!(stream.next().await.is_none());
        assert!(stream.next().await.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn preserves_unknown_events_as_other() -> Result<(), Box<dyn std::error::Error>> {
        let mut stream = stream_from_chunks([
            b"event: provider_custom\ndata: {\"type\":\"provider_custom\",\"value\":42}\n\n"
                as &'static [u8],
        ]);

        let event = stream
            .next()
            .await
            .ok_or_else(|| std::io::Error::other("expected stream event"))??;

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
