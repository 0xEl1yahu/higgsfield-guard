# higgsfield-lint

A Claude Code `PreToolUse` hook that lints Higgsfield `generate_*` tool calls before they
run, and tells the model when the prompt argues with its own parameters (a "4K" claim
against a 768px-tall render, a phrase repeated in both `prompt` and `negative_prompt`, a
negation like `"no logos"` sitting in the prompt when the model accepts
`negative_prompt`). It is advisory only: it never blocks a call, never exits non-zero, and
never writes to stderr. See `SPEC.md` for the full design and rationale.

## Required setup: build the binary

```
cargo build --release
```

This is not a one-time convenience, it is required before the hook does anything. The
hook runs `target/release/higgsfield-lint` directly. If that binary is missing, or was
built against an older `rust/src/*.rs`, the hook exits non-zero or runs stale logic -
either way `PreToolUse` treats it as non-blocking and the generation call proceeds
exactly as if the lint had found nothing. **Nothing distinguishes "the lint didn't run"
from "the lint ran and found nothing."** Re-run this build whenever `rust/src/*.rs`
changes, and confirm it before relying on the hook for the first time. See SPEC.md §12.1.

## Running the tests

```
cargo test --release
```

This runs the Rust unit tests, the pathological-input/fail-silent tests, and the golden
corpus (`rust/tests/parity.rs`): it feeds an identical corpus of inputs to the compiled
binary and asserts each one's output matches a recorded expectation in
`rust/tests/golden/expectations.json`, compared as parsed JSON. See SPEC.md §9.

## The Rust is the specification

There is no second implementation checked against it any more. A hand-written Python
port served as a differential oracle until the Rust was proven output-equivalent to it -
320 inputs / 0 divergences, plus 4,200 independently-fuzzed inputs from a separate
reviewer. It is now archived, read-only, at `docs/archive/verified-port/`: a **historical
record**, never a runtime fallback, and never edited to match anything that changes here
afterward. See that directory's README and SPEC.md §12.2 for the full history.

## Stack

Rust binary (`serde_json`, `regex`), built with `cargo build --release`. Full history of
why this shipped in Rust, then Python, then Rust again is in SPEC.md §12.
