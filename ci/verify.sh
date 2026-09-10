#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

# Doctor resolves this checkout's release binary. Do not allow a stale shell/session
# variable or external target directory to redirect the install-chain checks.
unset CLAUDE_PROJECT_DIR CARGO_TARGET_DIR
rustc --version
cargo --version
# Cargo can reuse an executable after test-only manifest edits, while doctor's mtime
# heuristic correctly sees a newer Cargo.toml. Rebuild our package, not dependencies.
cargo clean --release --package higgsfield-lint
cargo build --locked --release
cargo test --locked --release --all-targets -- --nocapture
# Full debug doctor expects a release executable path. Run only the relevant debug
# targets: unit tests, real panic containment, and process adversarial scenarios.
cargo test --locked --bin higgsfield-lint --test fail_silent --test adversarial --test cli -- --nocapture
# stdout is a pipe under CI's tee, which would select JSON (SPEC.md §6.1); keep the log readable.
./target/release/higgsfield-lint doctor --text
