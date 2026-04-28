# AGENTS.md

Operational guide for AI coding agents working in this repository.

## Project

This is an idiomatic, typed, async Rust SDK for Anthropic's Claude API. It is
an independent community project and is not affiliated with Anthropic PBC.
The SDK targets Anthropic's public Messages API and any Anthropic-compatible
gateway through configurable base URLs and custom model IDs.

The user-facing documentation, API surface, and design rationale all live in
`README.md`. Treat the README as the source of truth: if behavior differs
from the README, fix one of them in the same change.

## Workspace Layout

- `crates/anthropic-types/` — serde request/response models, validating
  newtypes, API-shaped enums, pagination shapes. No HTTP dependency.
- `crates/anthropic-client/` — async `Client`, configuration, transport,
  retries, request options, the `Messages` / `Models` / `Batches` resources,
  SSE streaming, and auto-pagination. Re-exports the public surface of
  `anthropic-types`.
- `crates/anthropic-mcp/` — optional MCP conversion helpers, kept out of the
  core client so applications that do not use MCP do not depend on it.
- `justfile` — common workflows (`just fmt`, `just check`, `just lint`,
  `just test`, `just doc`, `just deny`, `just verify`).
- `Cargo.toml` — workspace manifest.

## Before Editing

- Read `README.md`. It is the public contract.
- Inspect the relevant crate(s) and the existing tests for the area you are
  changing. Match existing style.
- Prefer meaningful, multi-file slices over tiny isolated fixes. Do not waste
  context on micro-tasks.

## Important Rules

- Prefer typed structs and enums over `serde_json::Value`. Use raw JSON only
  for genuinely arbitrary API data: JSON Schema, tool inputs, metadata,
  provider-specific blobs, raw structured output.
- Use newtypes where they improve correctness. Validate at construction
  (parse, don't validate). Examples: `ApiKey`, `Model`, `RequestId`,
  `MaxTokens`, `BaseUrl`, `ToolName`, `MessageBatchId`, `Temperature`,
  `TopK`, `TopP`.
- Model omitted optional fields with `Option<T>` and `skip_serializing_if`.
  Optional Anthropic request fields stay optional and stay omitted from the
  wire unless the caller sets them, so Anthropic-compatible gateways do not
  break on fields they ignore.
- No `unwrap`, `expect`, `panic!`, or `unsafe` in production SDK code without
  a local justification comment.
- Tests should avoid `unwrap` / `expect`; prefer `Result`-returning tests.
- Every public item in every crate must have a `///` doc comment. The crates
  enforce this with `#![warn(missing_docs)]`.
- Keep `Debug` output safe. `ApiKey` and `McpAuthorizationToken` redact in
  `Debug` and must continue to do so.
- Never log API keys, prompts, model responses, request bodies, response
  bodies, MCP tokens, or sensitive headers by default. `tracing` spans may
  include service name, endpoint path, HTTP method, retry attempt, status
  code, and request ID — nothing else.
- Keep Anthropic-compatible gateway support provider-neutral. Do not
  special-case any vendor.
- Keep MCP-specific code out of `anthropic-client`. New MCP behavior belongs
  in `anthropic-mcp`.
- Cancellation is by future-drop. There is no `Context` parameter.
- README.md must match implemented behavior. Update it in the same PR.

## Adding Public API

- Public items need `///` docs that answer: what it does, when to use it,
  what parameters mean, what is returned, what errors it produces, and any
  panics, safety, performance, or invariant notes.
- Add `# Examples` sections for headline types and functions. Prefer
  examples that compile under `cargo test --doc`. Use `no_run` for examples
  that talk to the network or require environment variables.
- Use intra-doc links (`` [`Type`] ``, `` [`crate::module::function`] ``) so
  rustdoc can cross-link.
- Prefer builder patterns for request structs with required fields, and
  return validating errors instead of panicking.
- When introducing a new error variant, add it to the public `Error` enum,
  document it, and verify that existing call sites still compile.

## Verification After Editing

Run these in order; fix anything that complains. Do not skip a step unless
you can document why.

```sh
cargo fmt --all
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --all-features --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
just verify
git diff --check
```

Optional milestone checks (run when the tools are installed):

```sh
cargo audit
cargo deny check
rust-analyzer diagnostics .
```

Known baseline notes:

- `cargo deny check` exits 0 but emits pre-existing duplicate-crate and
  unmatched-license warnings. Don't try to fix these as part of an unrelated
  change.
- `rust-analyzer diagnostics .` exits 0 but emits weak `inactive-code`
  warnings for `#[cfg(test)]` modules. These are not actionable.

## Pull Request Hygiene

- Keep commits focused. Prefer one logical change per commit.
- Update `README.md` in the same change whenever public behavior changes.
- Update doctests when a public example shape changes; doctests are part of
  the test suite.
- Mention any skipped verification steps and why in the PR description.

## Don't

- Don't introduce `unwrap` / `expect` in production code paths.
- Don't log secrets or user content.
- Don't add a feature flag, fallback, or backwards-compatibility shim for a
  scenario that has no caller.
- Don't change runtime behavior in a doc-only change.
- Don't widen the public API just to make tests easier — add test-only
  helpers behind `#[cfg(test)]` instead.
- Don't reorder or remove existing public re-exports without a deprecation
  path.
