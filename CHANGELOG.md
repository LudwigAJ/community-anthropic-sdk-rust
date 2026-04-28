# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-04-28

Initial release.

### Added

- `anthropic-types` crate with typed serde models for the Messages API,
  Models API, Message Batches API, content blocks, tools, citations,
  streaming events, pagination, errors, and validating newtypes
  (`MaxTokens`, `Temperature`, `TopK`, `TopP`, `ToolName`, `ApiKey`,
  `BaseUrl`, `RequestId`, `MessageBatchId`, etc.).
- `anthropic-client` crate with the async `Client`, `ClientBuilder`,
  `RequestOptions`, `ApiResponse`, `Error` / `ApiError` / `ApiErrorKind`,
  retry policy, and resource handles:
  - `Messages` — `create`, `create_with`, `create_with_response`,
    `create_and_parse`, `create_stream`, `create_streaming_text`,
    `count_tokens`, `count_tokens_with`, `count_tokens_with_response`.
  - `Models` — `list`, `list_with_params`, `list_with_response`,
    `list_auto_paging_with(_params)`, `list_pages_with`, `retrieve`,
    `retrieve_with_response`.
  - `Batches` — `create`, `retrieve`, `list`, `list_with_params`,
    `list_auto_paging_with(_params)`, `list_pages_with`, `cancel`,
    `delete`, `results`, `results_with`.
- Server-Sent Events streaming with partial-chunk handling, `ping` and
  `error` event support, malformed-JSON tolerance, drop-to-cancel, and
  `MessageStream::final_message()` accumulation (text, citations,
  thinking, redacted thinking, tool-use input JSON, message deltas).
- Typed cursor auto-pagination shared by Models and Message Batches via
  `AutoItemStream` / `AutoPageStream`.
- Structured output: `output_json_schema` request configuration and
  `Message::parse_json_output<T>()` / `Messages::create_and_parse<T>()`
  with non-leaking `StructuredOutputError`.
- Tool-use helpers: ordered borrowed `ToolUse` views, typed input
  decoding via `decode_input<T>()`, and follow-up `tool_result` message
  helpers.
- Optional `anthropic-mcp` crate with conversions between MCP and
  Anthropic tool / tool-result shapes.
- `#![warn(missing_docs)]` enforced across all three crates; rustdoc
  builds clean under `RUSTDOCFLAGS="-D warnings"`.

[Unreleased]: https://github.com/LudwigAJ/community-anthropic-sdk-rust/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/LudwigAJ/community-anthropic-sdk-rust/releases/tag/v0.1.0
