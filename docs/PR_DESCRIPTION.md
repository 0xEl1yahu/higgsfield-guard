# Re-ground Higgsfield lint rules on canonical MCP contracts

Repository note: the old `0xdefence/higgsfield-guard` repository was deleted. This PR
continues the work in the new public `0xdefence/higgsfield-preprocessor` repository,
preserving the local `main` baseline and feature-branch history.

## Summary

- Reconcile the six generation-tool schemas and model capabilities against the dated official MCP capture; keep CLI and MCP catalogs distinct.
- Replace unsupported generic dimension inference with model-scoped resolution, quality, mode and explicit target-pixel comparisons.
- Add quote-aware typography normalization and video-only supplied-duration over-claim checks.
- Scope checks by tool kind, skip audio text, and remove the retired model loader.
- Strengthen doctor with real fixtures, recursive build-input freshness and six-tool matcher coverage.
- Preserve historical inputs, add independent semantic controls, and document all reviewed golden changes, research coverage and performance tradeoffs.

The existing branch also includes the earlier doctor work, negation-placement removal, conflict-rule recalibration and archival of the verified Python oracle. Rust remains the sole runtime; this is not a second language migration.

## Verification

Re-run locally on 2026-09-10:

- `cargo test --release`: 82 unit and 11 integration tests pass; one intentionally ignored read-only audit helper.
- Golden corpus: 538 distinct inputs, zero divergences.
- `cargo test --test fail_silent`: four pass, including genuine debug panic injection.
- `cargo fmt --check`: clean.
- `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo build --release` and release doctor: all seven checks pass.
- `git diff --check`: clean.

Independent semantic controls cover 59 required negatives, 92 resolution positives,
nine local-conflict positives and 16 duration positives. All 65 research identifiers
are mapped: 57 covered (including explicitly marked representative/helper controls)
and eight deferred.

## Review notes

All 347 historical corpus input hashes are preserved. Exactly 87 old outputs lose
unsupported resolution advice; 191 new semantic cases were added. The historical
Kling fixture now produces only the local prompt/negative conflict. See
`docs/golden-change-ledger.json` for each reviewed before/after output.

Performance is measured, not claimed as a speedup: ordinary checked calls added
roughly 0.15–0.39 ms median in the recorded comparison; the 3.68 MB quote-heavy stress
case increased from 76.5 to 130.0 ms median. A production-comparison-count regression
guard fails under an isolated linear-scan substitution. Measurements and known
limitations are in `docs/rule-regrounding-implementation-report.md`.

## Deployment and limits

Rebuild the release binary after checkout, run doctor, then inspect `/hooks` and a
trace in the intended Claude Code session. Loaded-session interception has not been
verified. Higgsfield CLI commands and other clients bypass this MCP hook.

No generation jobs, global hook installation or recurring task changes were performed.
Ceiling inference, aspect-ratio checks, generic parameter validation, effective-duration
clamping and a CLI adapter remain deferred. Fail-open behavior is preserved.
