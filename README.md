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
exactly as if the lint had found nothing.

A `SessionStart` hook (`scripts/check-hook-freshness.sh`) now closes this: it checks the
binary at the start of every session and injects a one-line warning when it is missing or
older than `rust/src/*.rs`. It stays silent when the build is current. That warning is the
only thing distinguishing "the lint didn't run" from "the lint ran and found nothing", so
do not remove it without replacing it. Re-run this build whenever `rust/src/*.rs` changes.
See SPEC.md §12.1.

## Running the tests

```
cargo test --release
```

This runs the Rust unit tests, the pathological-input/fail-silent tests, and the
differential parity harness (`rust/tests/parity.rs`), which is the actual verification
instrument for this port: it feeds an identical corpus of inputs to both the Rust binary
and the Python oracle (below) and asserts their outputs are equal as parsed JSON. See
SPEC.md §9.

## Reviewing what the lint has actually done

```
cargo build --release && ./target/release/higgsfield-lint-review
```

The hook appends one metadata record per call to `data/findings.jsonl` - no prompt text,
clean calls included. This prints per-check fire rates over that log and lists models that
have been seen accepting a `negative_prompt` but are not yet in
`data/negative_prompt_models.json`. It never writes; promoting a model is a deliberate
edit and a commit. See SPEC.md §9.1 and §9.2.

## `src/higgsfield_lint.py` is the parity oracle - do not delete it

The hook itself is the compiled Rust binary. `src/higgsfield_lint.py` is **not** a
runtime fallback and nothing in the hook path calls it. It is retained as the read-only,
fixed point that the parity harness checks the Rust binary against. Do not edit it to
make a comparison pass, and do not delete it - that is a separate, later decision gated
on the Rust binary having run against real production traffic for a period the human
decides (SPEC.md §12.2).

## Stack

Rust binary (`serde_json`, `regex`), built with `cargo build --release`. Full history of
why this shipped in Rust, then Python, then Rust again is in SPEC.md §12.
