# community-anthropic-sdk-rust

An idiomatic, typed, async Rust SDK for Anthropic's Claude API.

This is an independent community project. It is not affiliated with, endorsed
by, or sponsored by Anthropic PBC. The SDK targets the public Anthropic
Messages API and any Anthropic-compatible gateway through configurable base
URLs and custom model IDs.

The design priorities are:

- Make invalid API requests difficult to construct.
- Use typed Rust structs and enums instead of unstructured `serde_json::Value`.
- Keep the common path short and obvious.
- Preserve API wire compatibility through serde attributes.
- Stay async-native (`tokio` + `reqwest`) and cancel by dropping futures.
- Keep API keys secret-aware and avoid logging prompts, responses, or other
  sensitive data by default.

## Table of Contents

- [Installation](#installation)
- [Quick Start](#quick-start)
- [Guide](#guide)
  - [Client Configuration](#client-configuration)
  - [Anthropic-Compatible Providers](#anthropic-compatible-providers)
  - [Messages](#messages)
  - [Streaming](#streaming)
  - [Tools](#tools)
  - [Structured Output](#structured-output)
  - [Token Counting](#token-counting)
  - [Pagination](#pagination)
  - [Models](#models)
  - [Message Batches](#message-batches)
  - [MCP Helpers](#mcp-helpers)
  - [Request Options](#request-options)
  - [Errors](#errors)
  - [Observability](#observability)
- [Troubleshooting](#troubleshooting)
- [API Reference](#api-reference)
- [Contributing](#contributing)
- [Disclaimer](#disclaimer)

## Installation

The crates in this workspace are not yet published to crates.io. Add them as a
git dependency or as a local path dependency.

### From GitHub

```toml
[dependencies]
anthropic-client = { git = "https://github.com/LudwigAJ/community-anthropic-sdk-rust" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

To pin a specific commit or branch:

```toml
[dependencies]
anthropic-client = { git = "https://github.com/LudwigAJ/community-anthropic-sdk-rust", branch = "main" }
```

The optional MCP helpers live in their own crate:

```toml
[dependencies]
anthropic-mcp = { git = "https://github.com/LudwigAJ/community-anthropic-sdk-rust" }
```

### From a local checkout

```sh
git clone https://github.com/LudwigAJ/community-anthropic-sdk-rust.git
cd community-anthropic-sdk-rust
```

Then in your project's `Cargo.toml`:

```toml
[dependencies]
anthropic-client = { path = "../community-anthropic-sdk-rust/crates/anthropic-client" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

`anthropic-client` re-exports every public type from `anthropic-types`, so most
applications only need this single crate.

### Building from source

The repository ships a `justfile` for the common workflows:

```sh
just fmt        # cargo fmt --all
just check      # cargo check --workspace --all-targets --all-features
just lint       # cargo clippy --workspace --all-targets --all-features -- -D warnings
just test       # cargo test --workspace --all-features
just doc        # RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
just verify     # fmt + check + lint + test + doc + cargo-deny
```

Or use `cargo` directly:

```sh
cargo build --workspace --all-features
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps --open
```

## Quick Start

Set your API key in the environment:

```sh
export ANTHROPIC_API_KEY="sk-ant-..."
```

Send a single message:

```rust
use anthropic_client::{Client, ContentBlock, MessageCreateParams, MessageParam, Model};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;

    let params = MessageCreateParams::builder()
        .model(Model::ClaudeSonnet4_5)
        .max_tokens(1024)
        .message(MessageParam::user("Hello, Claude"))
        .build()?;

    let message = client.messages().create(params).await?;

    for block in message.content {
        if let ContentBlock::Text { text, .. } = block {
            println!("{text}");
        }
    }

    Ok(())
}
```

## Guide

### Client Configuration

Build a client from environment variables:

```rust
use anthropic_client::{Client, Error};

fn client() -> Result<Client, Error> {
    Client::from_env()
}
```

Recognized environment variables:

| Variable | Meaning |
| --- | --- |
| `ANTHROPIC_API_KEY` | API key for `x-api-key` authentication. Required. |
| `ANTHROPIC_BASE_URL` | Optional base URL override for tests, proxies, or Anthropic-compatible gateways. Paths are allowed. |

Build a client explicitly:

```rust
use anthropic_client::{Client, Error};
use std::time::Duration;

fn client() -> Result<Client, Error> {
    Client::builder()
        .api_key("sk-ant-...")
        .base_url("https://api.anthropic.com")?
        .timeout(Duration::from_secs(600))
        .max_retries(2)
        .build()
}
```

Bring your own `reqwest::Client` when the application owns transport
configuration (proxy, custom DNS, mTLS, etc.):

```rust
use anthropic_client::{Client, Error};
use std::time::Duration;

fn client(http: reqwest::Client) -> Result<Client, Error> {
    Client::builder()
        .api_key("sk-ant-...")
        .http_client(http)
        .timeout(Duration::from_secs(600))
        .build()
}
```

API keys are wrapped in [`ApiKey`], a `Debug`-redacted newtype. The SDK never
logs prompts, response bodies, or sensitive headers by default.

### Anthropic-Compatible Providers

Configure the endpoint with `ANTHROPIC_BASE_URL` or
`ClientBuilder::base_url(...)`, and pass provider-specific model IDs as custom
`Model` values:

```sh
export ANTHROPIC_BASE_URL="https://api.minimax.io/anthropic"
export ANTHROPIC_API_KEY="${YOUR_API_KEY}"
```

```rust
use anthropic_client::{Client, MessageCreateParams, MessageParam, Model};

let client = Client::from_env()?;

let params = MessageCreateParams::builder()
    .model(Model::other("MiniMax-M2.7"))
    .max_tokens(1000)
    .system("You are a helpful assistant.")
    .message(MessageParam::user("Hi, how are you?"))
    .build()?;

let _message = client.messages().create(params).await?;
```

The builder also accepts raw model strings:

```rust
let params = MessageCreateParams::builder()
    .model("MiniMax-M2.7")
    .max_tokens(1000)
    .message(MessageParam::user("Hi, how are you?"))
    .build()?;
```

Provider-specific model strings serialize unchanged. Optional Anthropic
request fields (`cache_control`, `container`, `inference_geo`, `mcp_servers`,
`temperature`, `top_k`, `top_p`, `service_tier`, `context_management`, etc.)
are omitted from the wire unless the caller sets them, so they do not break
gateways that ignore them.

### Messages

Send a list of conversational turns and receive the next assistant message.

```rust
use anthropic_client::{ContentBlock, MessageCreateParams, MessageParam, Model};

let params = MessageCreateParams::builder()
    .model(Model::ClaudeSonnet4_5)
    .max_tokens(512)
    .message(MessageParam::user("Explain Rust ownership in two sentences."))
    .build()?;

let message = client.messages().create(params).await?;

for block in message.content {
    if let ContentBlock::Text { text, .. } = block {
        println!("{text}");
    }
}
```

Multi-turn conversations:

```rust
let params = MessageCreateParams::builder()
    .model(Model::ClaudeSonnet4_5)
    .max_tokens(1024)
    .message(MessageParam::user("Hello there."))
    .message(MessageParam::assistant("Hi. How can I help?"))
    .message(MessageParam::user("Explain async Rust at a high level."))
    .build()?;
```

System prompts use the top-level `system` field (Claude has no `system` role
on `messages`):

```rust
let params = MessageCreateParams::builder()
    .model(Model::ClaudeSonnet4_5)
    .max_tokens(1024)
    .system("You are concise and precise.")
    .message(MessageParam::user("What is SSE?"))
    .build()?;
```

Use `SystemPromptBlock` for structured system prompts with cache markers:

```rust
use anthropic_client::{CacheControl, CacheControlTtl, SystemPromptBlock};

let params = MessageCreateParams::builder()
    .model(Model::ClaudeSonnet4_5)
    .max_tokens(1024)
    .system_block(SystemPromptBlock::text_with_cache_control(
        "Use the following policy for this session.",
        CacheControl::ephemeral_with_ttl(CacheControlTtl::OneHour),
    ))
    .system_block(SystemPromptBlock::text("Answer concisely."))
    .message(MessageParam::user("What is SSE?"))
    .build()?;
```

If the final message has the assistant role, Claude continues from that
content. Returned assistant messages can be replayed via `Message::to_param()`,
which preserves text, citations, thinking signatures, redacted thinking, and
`tool_use` blocks (and reports unsupported blocks as a typed conversion
error).

#### Request Content Blocks

```rust
use anthropic_client::{
    CacheControl, ContentBlockParam, ContentBlockSourceContentBlockParam,
    ContentBlockSourceContentParam, MessageCreateParams, MessageParam, Model,
    SearchResultTextBlockParam, TextCitationParam,
};

let params = MessageCreateParams::builder()
    .model(Model::ClaudeSonnet4_5)
    .max_tokens(1024)
    .message(MessageParam::user_blocks(vec![
        ContentBlockParam::text_with_citations(
            "Compare these sources.",
            vec![TextCitationParam::page_location("reported revenue", 0, 2, 3)
                .with_document_title("Quarterly report")],
        ),
        ContentBlockParam::image_url("https://example.com/chart.png"),
        ContentBlockParam::document_url("https://example.com/report.pdf")
            .with_cache_control(CacheControl::ephemeral())?,
        ContentBlockParam::document_content(ContentBlockSourceContentParam::blocks(vec![
            ContentBlockSourceContentBlockParam::text("Inline document section"),
            ContentBlockSourceContentBlockParam::image_base64("image/png", "base64-image"),
        ])),
        ContentBlockParam::search_result(
            vec![SearchResultTextBlockParam::text("Relevant search excerpt")],
            "https://example.com/source",
            "Source title",
        ),
    ]))
    .build()?;
```

Content sources serialize as typed `base64`, `url`, `text`, or `content`
variants. Citations support `char_location`, `page_location`,
`content_block_location`, `web_search_result_location`, and
`search_result_location`.

#### Sampling, Cache Control, Thinking, Service Tier

```rust
MessageCreateParams::builder()
    .model(Model::ClaudeSonnet4_5)
    .max_tokens(4096)
    .message(MessageParam::user("Hello"))
    .cache_control_ephemeral_with_ttl(CacheControlTtl::OneHour)
    .inference_geo("eu")?
    .stop_sequence("\n\nHuman:")
    .thinking_enabled_with_display(2048, ThinkingDisplay::Omitted)?
    .service_tier(ServiceTier::StandardOnly)
    .temperature(1.0)?
    .top_k(50)?
    .top_p(0.99)?
    .build()?;
```

`temperature`, `top_k`, and `top_p` are validated newtypes. `ThinkingConfig`
is a tagged enum with `enabled`, `disabled`, and `adaptive` shapes; enabled
thinking requires at least 1,024 tokens and a budget less than `max_tokens`.

#### Response Metadata

For request IDs and other successful response metadata without changing the
`Message` model:

```rust
use anthropic_client::{ApiResponse, Message, RequestOptions};

let response: ApiResponse<Message> = client
    .messages()
    .create_with_response(params, RequestOptions::new())
    .await?;

if let Some(request_id) = response.request_id() {
    eprintln!("request_id={request_id}");
}
```

`stop_reason` is an enum (`EndTurn`, `MaxTokens`, `StopSequence`, `ToolUse`,
`PauseTurn`, `Refusal`, `Other(String)`). Unknown values round-trip through
`Other(String)` for forward compatibility.

`Message::usage` exposes `input_tokens`, `output_tokens`,
`cache_creation_input_tokens`, `cache_read_input_tokens`, and optional
telemetry such as `cache_creation` TTL breakdown, `server_tool_use`,
`service_tier`, and `inference_geo`. Optional telemetry fields stay optional
because Anthropic-compatible providers may omit them or return `null`. Use
`Usage::total_input_tokens()` when you want the sum of uncached,
cache-creation, and cache-read input tokens.

### Streaming

Message streaming uses Server-Sent Events.

```rust
use anthropic_client::{MessageCreateParams, MessageParam, MessageStreamEvent, Model};
use futures_util::StreamExt;

let params = MessageCreateParams::builder()
    .model(Model::ClaudeSonnet4_5)
    .max_tokens(1024)
    .message(MessageParam::user("Write a haiku about type systems."))
    .build()?;

let mut stream = client.messages().create_stream(params).await?;

while let Some(event) = stream.next().await {
    match event? {
        MessageStreamEvent::ContentBlockDelta { delta, .. } => {
            if let Some(text) = delta.text() {
                print!("{text}");
            }
        }
        MessageStreamEvent::MessageStop => break,
        _ => {}
    }
}
```

`MessageStream` implements `futures_core::Stream<Item = Result<MessageStreamEvent, Error>>`
and handles partial chunks, `ping` events, API `error` events, malformed JSON,
early termination, split CRLF frame boundaries, and cancellation by drop. For
provider compatibility, it also accepts known SSE event names when the JSON
payload omits its own `type`, and treats `data: [DONE]` as a stream stop
marker.

#### Streaming Text Convenience

```rust
let mut text = client.messages().create_streaming_text(params).await?;

while let Some(chunk) = text.next().await {
    print!("{}", chunk?);
}
```

`TextStream` is intentionally lossy: it yields only `text_delta` chunks,
ignores non-text events, does not accumulate a final `Message`, does not
validate streamed tool-input JSON, and ends quietly if the HTTP body ends
before `message_stop`. Use `MessageStream::final_message()` when you need the
complete response shape or stream-structure validation.

#### Final Message Convenience

```rust
let message = client
    .messages()
    .create_stream(params)
    .await?
    .final_message()
    .await?;
```

The accumulator preserves text deltas, citation deltas, thinking and signature
deltas, redacted thinking, tool-use input JSON deltas, and `message_delta`
stop/usage fields. Malformed or incomplete tool input JSON surfaces as an SDK
stream error at content-block completion. Streaming `message_delta` usage can
omit input/cache counters and `server_tool_use`; the accumulator updates only
fields present in the delta and preserves earlier values from `message_start`.

### Tools

Define a tool:

```rust
use anthropic_client::{CacheControlTtl, JsonSchema, Tool};
use serde_json::json;

let weather_tool = Tool::custom(
    "get_weather",
    JsonSchema::from_value(json!({
        "type": "object",
        "properties": {
            "city": { "type": "string" },
            "units": { "type": "string", "enum": ["metric", "imperial"] }
        },
        "required": ["city"],
        "additionalProperties": false
    }))?,
)
.description("Get the current weather for a city.")
.cache_control_ephemeral_with_ttl(CacheControlTtl::OneHour);
```

Send tools with a message:

```rust
use anthropic_client::ToolChoice;

let params = MessageCreateParams::builder()
    .model(Model::ClaudeSonnet4_5)
    .max_tokens(1024)
    .message(MessageParam::user("What is the weather in London?"))
    .tool(weather_tool)
    .tool_choice(ToolChoice::auto())
    .build()?;

let message = client.messages().create(params).await?;
```

Decode tool input into your own types:

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct WeatherInput {
    city: String,
    units: Option<String>,
}

for tool_use in message.tool_uses() {
    if tool_use.name() == "get_weather" {
        let input: WeatherInput = tool_use.decode_input()?;
        let result = format!("{}: 18 C and partly cloudy", input.city);

        let follow_up = tool_use.tool_result_message(result);

        let params = MessageCreateParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .max_tokens(1024)
            .message(message.to_param()?)
            .message(follow_up)
            .build()?;
    }
}
```

`Message::tool_uses()` preserves response order. `ToolUse::decode_input<T>()`
clones the raw JSON into a caller-owned `T: DeserializeOwned`. The
`ToolInputDecodeError` `Display` does **not** include raw tool input JSON.

Return tool errors with `tool_result_error`:

```rust
let error_block = ContentBlockParam::tool_result_error(
    "toolu_01...",
    "The weather service timed out.",
);
```

`ToolChoice` covers `Auto`, `Any`, `Tool { name }`, and `None`. Omitting
`tool_choice` leaves the API default in effect.

### Structured Output

```rust
use anthropic_client::{Client, MessageCreateParams, MessageParam, Model};
use serde::Deserialize;
use serde_json::json;

#[derive(Debug, Deserialize)]
struct Answer {
    answer: u32,
}

let params = MessageCreateParams::builder()
    .model(Model::ClaudeSonnet4_5)
    .max_tokens(256)
    .message(MessageParam::user("What is 2 + 2?"))
    .output_json_schema(json!({
        "type": "object",
        "properties": { "answer": { "type": "integer" } },
        "required": ["answer"],
        "additionalProperties": false
    }))?
    .build()?;

let answer: Answer = client.messages().create_and_parse(params).await?;
println!("{}", answer.answer);
```

`Message::parse_json_output<T>()` returns `StructuredOutputError` for missing
text, multiple text blocks, unsupported non-text content, invalid JSON, or
JSON that does not deserialize into `T`. Error messages do not include the
model output.

### Token Counting

```rust
use anthropic_client::{MessageCountTokensParams, MessageParam, Model};

let count = client
    .messages()
    .count_tokens(
        MessageCountTokensParams::builder()
            .model(Model::ClaudeSonnet4_5)
            .message(MessageParam::user("How many tokens is this?"))
            .build()?,
    )
    .await?;

println!("{}", count.input_tokens);
```

The `count_tokens_with_response` variant exposes per-request `RequestOptions`
and the response `RequestId`.

### Pagination

List endpoints return `Page<T>`. Build cursor parameters with `ListParams`,
which rejects `limit == 0` and conflicting `before_id` / `after_id` cursors at
build time.

```rust
use anthropic_client::ListParams;

let params = ListParams::builder()
    .limit(20)
    .after_id("claude-sonnet-4-5")
    .build()?;
```

Walk pages manually:

```rust
let mut params = ListParams::new();

loop {
    let page = client.models().list_with_params(params.clone()).await?;
    for info in &page.data {
        println!("{}", info.id);
    }

    match page.next_page_params(params.limit) {
        Some(next) => params = next,
        None => break,
    }
}
```

Use auto-pagination item streams for ergonomic walking. The stream lazily
fetches the first page, follows `last_id` as the next `after_id`, preserves
the caller's `limit`, and applies request options to every page request:

```rust
use futures_util::StreamExt;

let mut models = client
    .models()
    .list_auto_paging_with_params(ListParams::builder().limit(20).build()?);

while let Some(item) = models.next().await {
    let info = item?;
    println!("{}", info.id);
}
```

Use page streams when you need request-id metadata for each fetched page:

```rust
use anthropic_client::RequestOptions;

let mut pages = client.models().list_pages_with(
    ListParams::builder().limit(20).build()?,
    RequestOptions::new(),
);

while let Some(page) = pages.next().await {
    let page = page?;
    eprintln!("request_id={:?}", page.request_id());
    for info in page.data.data {
        println!("{}", info.id);
    }
}
```

### Models

```rust
use anthropic_client::{Client, ListParams};

let client = Client::from_env()?;

let page = client
    .models()
    .list_with_params(ListParams::builder().limit(20).build()?)
    .await?;

for info in &page.data {
    println!("{} — {}", info.id, info.display_name);
}

let info = client.models().retrieve("claude-sonnet-4-5").await?;
println!("{:?}", info.max_input_tokens);
```

`Model` exposes well-known identifiers as enum variants and uses
`Model::Other { id }` for forward compatibility. `ModelInfo::model()` returns
the typed identifier. Capability metadata is preserved as
`Option<serde_json::Value>` so newly added flags do not require an SDK
release.

| Method | HTTP | Path |
| --- | --- | --- |
| `client.models().list()` | `GET` | `/v1/models` |
| `client.models().list_with_params(params)` | `GET` | `/v1/models?…` |
| `client.models().list_with_response(params, options)` | `GET` | `/v1/models?…` |
| `client.models().list_auto_paging_with(params, options)` | `GET` (paged) | `/v1/models?…` |
| `client.models().list_pages_with(params, options)` | `GET` (paged) | `/v1/models?…` |
| `client.models().retrieve(id)` | `GET` | `/v1/models/{model_id}` |
| `client.models().retrieve_with_response(id, options)` | `GET` | `/v1/models/{model_id}` |

### Message Batches

```rust
use anthropic_client::{
    BatchCreateParams, MessageCreateParams, MessageParam, Model,
};
use futures_util::StreamExt;

let inner = MessageCreateParams::builder()
    .model(Model::ClaudeSonnet4_5)
    .max_tokens(1024)
    .message(MessageParam::user("Hello"))
    .build()?;

let params = BatchCreateParams::builder()
    .add("req-1", inner)?
    .build()?;

let batch = client.messages().batches().create(params).await?;
println!("created {} ({})", batch.id, batch.processing_status);

let mut stream = client.messages().batches().results(&batch.id).await?;
while let Some(line) = stream.next().await {
    let item = line?;
    println!("{}: {:?}", item.custom_id, item.result);
}
```

The builder rejects empty batches, duplicate `custom_id`s, empty `custom_id`s,
and entries whose inner `MessageCreateParams` set `stream = true`. Lifecycle
methods validate the `MessageBatchId` newtype and reject blank IDs before
sending a request. `results(...)` first calls `retrieve(id)` and returns
`Error::BatchResultsUnavailable { batch_id, processing_status }` when the
batch has not yet ended; otherwise it streams JSONL lines from the returned
`results_url`. `results_with` applies request options to both the metadata
lookup and the JSONL download.

Auto-paginate listed batches:

```rust
use anthropic_client::ListParams;

let mut batches = client
    .messages()
    .batches()
    .list_auto_paging_with_params(ListParams::builder().limit(10).build()?);

while let Some(item) = batches.next().await {
    let batch = item?;
    println!("{} ({})", batch.id, batch.processing_status);
}
```

Delete completed batches:

```rust
let deleted = client.messages().batches().delete(&batch.id).await?;
assert_eq!(deleted.object_type, "message_batch_deleted");
```

`MessageBatchResult` is a tagged union with `Succeeded { message }`,
`Errored { error }`, `Canceled`, and `Expired` variants.

| Method | HTTP | Path |
| --- | --- | --- |
| `client.messages().batches().create(params)` | `POST` | `/v1/messages/batches` |
| `client.messages().batches().retrieve(id)` | `GET` | `/v1/messages/batches/{batch_id}` |
| `client.messages().batches().list_with_params(params)` | `GET` | `/v1/messages/batches?…` |
| `client.messages().batches().list_auto_paging_with(params, options)` | `GET` (paged) | `/v1/messages/batches?…` |
| `client.messages().batches().list_pages_with(params, options)` | `GET` (paged) | `/v1/messages/batches?…` |
| `client.messages().batches().cancel(id)` | `POST` | `/v1/messages/batches/{batch_id}/cancel` |
| `client.messages().batches().delete(id)` | `DELETE` | `/v1/messages/batches/{batch_id}` |
| `client.messages().batches().results(id)` | `GET` (twice) | `/v1/messages/batches/{batch_id}` then the returned `results_url` |

### MCP Helpers

The `anthropic-mcp` crate offers conversions between MCP shapes and Anthropic
`tool` / `tool_result` blocks, so the core client never depends on an MCP
runtime.

```rust
use anthropic_mcp::{IntoAnthropicTool, McpConversionError, McpTool};
use anthropic_client::{MessageCreateParams, MessageParam, Model};

fn build_params(mcp_tools: Vec<McpTool>) -> Result<MessageCreateParams, Box<dyn std::error::Error>> {
    let tools = mcp_tools
        .into_iter()
        .map(IntoAnthropicTool::into_anthropic_tool)
        .collect::<Result<Vec<_>, McpConversionError>>()?;

    let params = MessageCreateParams::builder()
        .model(Model::ClaudeSonnet4_5)
        .max_tokens(1024)
        .message(MessageParam::user("Use the available tools."))
        .tools(tools)
        .build()?;

    Ok(params)
}
```

Convert an MCP tool result back into an Anthropic `tool_result` block:

```rust
use anthropic_mcp::{IntoAnthropicToolResult, McpConversionError, McpToolResult};
use anthropic_client::MessageParam;

fn follow_up(result: McpToolResult) -> Result<MessageParam, McpConversionError> {
    let tool_result = result.into_anthropic_tool_result()?;
    Ok(MessageParam::user_blocks(vec![tool_result]))
}
```

`McpTool` conversion validates names through `ToolName` and requires a
non-empty object input schema. `McpToolResult` conversion preserves text and
supported image content in order, maps MCP `isError: true` to Anthropic
`is_error: true`, and reports unsupported variants (resource links, audio)
with the failing content index.

The Messages API also supports request-scoped MCP servers through the typed
`McpServer` builder:

```rust
use anthropic_client::{
    ContextManagementConfig, ContextManagementEdit, McpServer, McpServerToolConfiguration,
    MessageCreateParams, MessageParam, Model, ThinkingTurnsKeep, ToolName,
};

let docs_server = McpServer::url("docs", "https://mcp.example.com/sse")?
    .authorization_token("mcp-secret-token")?
    .tool_configuration(
        McpServerToolConfiguration::new()
            .allowed_tool(ToolName::try_new("search_docs")?)
            .enabled(true),
    );

let context_management = ContextManagementConfig::new(vec![
    ContextManagementEdit::clear_thinking_keep(ThinkingTurnsKeep::thinking_turns(1)?),
]);

let params = MessageCreateParams::builder()
    .model(Model::ClaudeSonnet4_5)
    .max_tokens(1024)
    .message(MessageParam::user("Search the product docs."))
    .container("container_01")?
    .context_management(context_management)
    .mcp_server(docs_server)
    .build()?;
```

`McpAuthorizationToken` redacts in `Debug`. It still serializes to the wire
string, so do not log request bodies when tokens or other sensitive values
may be present.

### Request Options

```rust
use anthropic_client::RequestOptions;
use std::time::Duration;

let message = client
    .messages()
    .create_with(
        params,
        RequestOptions::builder()
            .timeout(Duration::from_secs(30))
            .max_retries(0)
            .header("anthropic-beta", "some-beta")
            .build()?,
    )
    .await?;
```

- Client-level options apply to every request.
- Request-level options override client defaults.
- Headers are additive unless a request explicitly overrides a key.
- Header names and values are validated when options are built.
- Request IDs from responses are available through `ApiResponse` and on API
  errors.
- Batch lifecycle methods expose `_with` and `_with_response` variants;
  `results_with` applies the same options to the metadata lookup and the
  JSONL download.
- Auto-pagination `_with` helpers apply request options to every page
  request.

### Errors

The crate exposes a single public error type with structured API errors:

```rust
pub enum Error {
    Config(ConfigError),
    Transport(TransportError),
    Timeout(TimeoutError),
    Api(ApiError),
    Json(serde_json::Error),
    Stream(StreamError),
    InvalidMessageBatchId { source: MessageBatchIdError },
    // ...
}

pub struct ApiError {
    pub status: http::StatusCode,
    pub request_id: Option<RequestId>,
    pub kind: ApiErrorKind,
    pub message: String,
    pub body: Option<ApiErrorBody>,
}

pub enum ApiErrorKind {
    InvalidRequest,
    Authentication,
    Permission,
    NotFound,
    Conflict,
    UnprocessableEntity,
    RateLimit,
    InternalServer,
    Overloaded,
    Unknown(String),
}
```

Match on the kind:

```rust
use anthropic_client::{ApiErrorKind, Error};

match client.messages().create(params).await {
    Ok(message) => println!("{}", message.id),
    Err(Error::Api(api)) if api.kind == ApiErrorKind::RateLimit => {
        eprintln!("rate limited; request_id={:?}", api.request_id);
    }
    Err(error) => return Err(error.into()),
}
```

The retry policy retries connection failures, request timeouts, HTTP `408`,
`409`, `429`, and `5xx` responses. Non-idempotent requests are not retried
blindly.

### Observability

The client emits `tracing` spans around HTTP requests and stream decoding.
Spans include the service name, endpoint path, HTTP method, retry attempt,
status code, and request ID. They never include API keys, prompts, system
prompts, tool inputs, or model responses.

Enable logging in your application:

```rust
tracing_subscriber::fmt()
    .with_env_filter("anthropic_client=debug")
    .init();
```

## Troubleshooting

**`Error::Config(ConfigError::MissingApiKey)`** — `ANTHROPIC_API_KEY` is unset.
Export it, pass it via `Client::builder().api_key(...)`, or check that your
process inherits the environment.

**`401 Authentication`** — the API key is rejected. Double-check that the
secret is current and that you are pointing at the correct base URL.

**`429 RateLimit`** — Anthropic is throttling your account. The client retries
`429` automatically up to `MaxRetries`; surface the error to your caller after
that, optionally honoring any `retry-after` header on the underlying
`ApiError::body`.

**`Error::BatchResultsUnavailable { batch_id, processing_status }`** — you
called `batches().results(id)` before the batch finished. Poll `retrieve(id)`
or the auto-paginated list until `processing_status` is `ended`, then call
`results` again.

**`Error::Stream(_)` mid-stream** — likely a malformed event or truncated
connection. The error preserves any `request_id` for support escalation.
Reconnecting and retrying the request is safe; the SDK does not partially
commit.

**`Error::InvalidMessageBatchId`** — the `MessageBatchId` newtype rejected a
blank ID before the request was sent. Validate IDs at your application
boundary.

**Custom `reqwest` middleware breaks streaming** — when supplying your own
`reqwest::Client`, leave `reqwest::redirect::Policy` at its default and avoid
response decompression layers that buffer the body. The SDK's SSE parser
expects a chunked body.

**Anthropic-compatible gateway rejects fields like `cache_control`** — these
fields are omitted unless you set them. Audit your `MessageCreateParams` for
optional fields and remove the ones the gateway does not support.

**`ToolInputDecodeError` while decoding tool input** — Claude returned input
that does not match your `Deserialize` type. The error includes the tool ID
and tool name; inspect `tool_use.input()` (raw JSON) to see what arrived.

**Doc build fails with `RUSTDOCFLAGS="-D warnings"`** — run
`cargo doc --workspace --all-features --no-deps` and fix the reported broken
intra-doc link or missing-doc comment.

## API Reference

Browse the full API documentation locally:

```sh
cargo doc --workspace --all-features --no-deps --open
```

Or, after publishing to crates.io, the API will be available on
[docs.rs](https://docs.rs/anthropic-client). Until then, the rendered docs
under `target/doc/anthropic_client/index.html` are the source of truth.

The crates and their primary entry points:

- `anthropic-client` — async [`Client`], [`Messages`], [`Models`],
  [`Batches`], [`MessageStream`], [`TextStream`],
  [`AutoItemStream`] / [`AutoPageStream`], [`RequestOptions`],
  [`ApiResponse`], [`Error`], [`ApiError`], [`ApiErrorKind`].
- `anthropic-types` — [`MessageCreateParams`], [`MessageParam`],
  [`ContentBlock`], [`ContentBlockParam`], [`SystemPrompt`],
  [`SystemPromptBlock`], [`Tool`], [`ToolChoice`], [`MessageBatch`],
  [`BatchCreateParams`], [`MessageBatchResult`], [`Model`], [`ModelInfo`],
  [`Usage`], [`CacheCreation`], [`ServerToolUsage`], [`UsageServiceTier`],
  [`MessageDeltaUsage`], [`Page`], [`ListParams`], and validating newtypes
  such as [`MaxTokens`], [`Temperature`], [`TopK`], [`TopP`], [`ToolName`],
  [`MessageBatchId`], [`RequestId`].
- `anthropic-mcp` — [`IntoAnthropicTool`], [`IntoAnthropicToolResult`],
  [`IntoMcpCallToolRequest`] and the supporting `McpTool`,
  `McpToolResult`, `McpCallToolRequest` types.

## Contributing

Issues and pull requests are welcome at
[github.com/LudwigAJ/community-anthropic-sdk-rust](https://github.com/LudwigAJ/community-anthropic-sdk-rust).

Before opening a PR, run:

```sh
cargo fmt --all
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
just verify
```

Or run the optional lints when those tools are installed:

```sh
cargo audit
cargo deny check
```

Coding rules:

- Prefer typed structs and enums over `serde_json::Value`. Use raw JSON only
  for genuinely arbitrary data: JSON Schema, tool inputs, metadata,
  provider-specific blobs, raw structured output.
- Use newtypes where they improve correctness. Validate at construction.
- Model omitted optional fields with `Option<T>` and `skip_serializing_if`.
- No `unwrap`, `expect`, `panic!`, or unsafe in production SDK code without a
  local justification comment.
- Tests should avoid `unwrap` / `expect`; prefer `Result`-returning tests.
- Never log API keys, prompts, model responses, request bodies, response
  bodies, MCP tokens, or sensitive headers by default.
- Keep Anthropic-compatible gateway support provider-neutral.
- Keep this README in sync with implemented behavior.

## Disclaimer

This is an independent community project. It is not affiliated with, endorsed
by, sponsored by, or associated with Anthropic PBC. Anthropic, Claude, and
related names, marks, and copyrights belong to their respective owners.
References to Anthropic products, APIs, SDKs, or documentation are for
compatibility, interoperability, and educational purposes only.
