# Contributing

Thanks for your interest in contributing! This is an independent community
Rust SDK for Anthropic's Claude API. Issues, bug reports, and pull requests
are all welcome.

## Getting set up

You will need a recent Rust toolchain. The workspace pins MSRV at **Rust 1.85**.

```sh
git clone https://github.com/LudwigAJ/community-anthropic-sdk-rust.git
cd community-anthropic-sdk-rust
cargo build --workspace --all-features
cargo test --workspace --all-features
```

The repository includes a `justfile` for common workflows:

```sh
just fmt        # cargo fmt --all
just check      # cargo check --workspace --all-targets --all-features
just lint       # cargo clippy --workspace --all-targets --all-features -- -D warnings
just test       # cargo test --workspace --all-features
just doc        # RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
just verify     # fmt + check + lint + test + doc + cargo deny
```

## Before opening a pull request

Run the full local pipeline. CI runs the same commands and will reject
anything that fails them locally.

```sh
cargo fmt --all
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
```

Optional checks if you have the tools installed:

```sh
cargo audit
cargo deny check
```

## Coding rules

- **Typed first.** Prefer typed structs and enums over `serde_json::Value`.
  Use raw JSON only for genuinely arbitrary data (JSON Schema, tool inputs,
  metadata, provider-specific blobs, raw structured output).
- **Validate at construction.** Use newtypes (`parse, don't validate`) so
  invalid values surface as typed errors before they reach the wire.
- **Optional means omitted.** Model optional API fields as `Option<T>` with
  `#[serde(skip_serializing_if = "Option::is_none")]` so they are not sent
  unless the caller sets them. This keeps Anthropic-compatible gateways
  working when they ignore unfamiliar fields.
- **No production `unwrap` / `expect` / `panic!` / `unsafe`** without a
  local justification comment.
- **Tests should avoid `unwrap` / `expect`.** Prefer `Result`-returning
  tests with `?`.
- **Document everything public.** Each crate has `#![warn(missing_docs)]`.
  Public items need a `///` comment that answers what it does, when to
  use it, what its parameters and return value mean, and which errors or
  panics it can produce.
- **No secrets in logs.** Never log API keys, prompts, model responses,
  request bodies, response bodies, MCP tokens, or sensitive headers.
  `tracing` spans may include service name, endpoint path, HTTP method,
  retry attempt, status code, and request ID — nothing else.
- **Stay provider-neutral.** Anthropic-compatible gateway support means
  no vendor-specific special cases.
- **Keep MCP code in `anthropic-mcp`.** The core client must not depend on
  an MCP runtime.
- **Cancellation by drop.** No `Context` parameter, no separate cancel
  token. Dropping the future cancels the request.
- **README is the contract.** Update `README.md` in the same PR whenever
  public behavior changes.

## Pull request expectations

- Keep commits focused; prefer one logical change per commit.
- Mention any skipped verification steps and why in the PR description.
- Add or update tests for new behavior. Add or update doctests for new
  public examples.
- Update `CHANGELOG.md` under the `[Unreleased]` section with a one-line
  entry under `Added`, `Changed`, `Fixed`, or `Removed`.
- Don't widen the public API solely to make tests easier — add test-only
  helpers behind `#[cfg(test)]` instead.
- Don't reorder or remove existing public re-exports without a deprecation
  path.

## Reporting issues

Open an issue at
<https://github.com/LudwigAJ/community-anthropic-sdk-rust/issues>. For
suspected security vulnerabilities, please use GitHub's private
vulnerability reporting on the same repository rather than opening a
public issue.

## License

By contributing, you agree that your contributions will be licensed under
the [MIT License](LICENSE) that covers the project.
