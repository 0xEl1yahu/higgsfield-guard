# higgsfield-lint

Advisory Claude Code `PreToolUse` hook for Higgsfield MCP image, video and audio
generation tools. It checks supported output-tier claims, local prompt/negative-prompt
overlap, and video duration over-claims. Audio skips these text checks.

The schema is grounded in a dated [official MCP capture](docs/higgsfield-canonical-contract.md).
Unknown model semantics are not guessed. Bare width/height do not prove rendered size,
and negative_prompt overlap is local text consistency—not proof the provider honors it.

## Build and verify

```sh
cargo build --release
target/release/higgsfield-lint doctor
cargo test --release
cargo test --test fail_silent
```

Run from this checkout. Rebuild after changes under rust/src, Cargo.toml or Cargo.lock.
The project hook configuration in [.claude/settings.json](.claude/settings.json) invokes
`target/release/higgsfield-lint` directly. Doctor checks seven local conditions, including
recursive source/manifest/lockfile freshness, six-tool matcher coverage and actual fixtures.
Its timestamp check is a heuristic, not a build fingerprint.

In the intended Claude Code session, inspect `/hooks` and a tool trace to confirm this
checkout's hook actually loaded. Doctor cannot prove another session's loaded state.
No global installation is performed automatically.

The normal hook is fail-open: exit 0, empty stderr, empty stdout when no findings.
Doctor is the explicit diagnostic exception. A missing executable or OS-level abort
cannot be caught by the hook; the generation is not intentionally blocked.

## Scope and limits

Resolution checks require a supported model/field/option association. Image and video
tiers stay in their declared comparison domains; explicit Topaz targets are described
as requested pixels. No universal 2k-to-pixel conversion or guessed output geometry.
Duration checks require a known video model and eligible mode and compare the supplied
setting, not a promised effective duration. Quotes and a narrow set of typography folds
are supported. Ceiling inference, ratios, parameter clamps and language understanding
remain deferred. See [SPEC](SPEC.md) and the
[implementation report](docs/rule-regrounding-implementation-report.md).

These hooks match MCP calls, not Higgsfield CLI shell commands. CLI use can bypass this
hook; there is no CLI adapter in this implementation.

## Tests and runtime

Rust is the sole runtime; the Python-to-Rust migration is complete. The old Python
implementation and its historical verification are read-only in
[the archive](docs/archive/verified-port/README.md), never a fallback or live oracle.
The current 538-input golden corpus preserves historical input bytes and adds 191
independently specified semantic cases. See the [coverage guide](rust/tests/golden/README.md).

Two direct dependencies: serde_json and regex. No hook-time network, application
data-file reads, data-file writes or telemetry. The optional
[credit-expiry reminder](docs/credit-expiry-task.md) is separate and was not scheduled.
