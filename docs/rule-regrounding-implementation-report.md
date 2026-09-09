# Rule re-grounding implementation report

Executed locally on 2026-09-09 from `a7e0039` (`feat/doctor-and-expiry`).
The supported core in PLAN is implemented and verified. No commit, push, generation,
upload, global hook installation, CLI adapter, or recurring-task change was performed.
Target-session loaded-hook verification remains unperformed.

## What changed

- Replaced generic width/height and universal short-side inference with exact
  MCP model/field/option associations. Nominal K tiers, video p tiers and explicit
  requested Topaz pixels remain distinct. Advice names resolution, quality, mode or
  target fields without promising rendered geometry.
- Removed 8K/UHD/Full HD claim vocabulary. Supported quote-aware resolution matches
  preserve original spelling through narrow typography folding, including fullwidth
  ASCII, curly quotes and horizontal spaces. No full-input source-offset map.
- Added video-only duration over-claims with eligible model/mode checks, supplied
  duration bounds, strict finite-number grammar, quote-overlap exclusions and maximum
  claim selection. No inferred default or effective-duration/clamping guarantee.
- Threaded ToolKind through single and batch execution. Audio returns before parameter
  parsing; unknown tool names retain only local prompt/negative conflict checks.
- Removed `rust/src/models.rs` and all loader callers. The file is recoverable in Git.
  `data/.gitkeep` remains a doctor root landmark. No runtime data-file reads were added.
- Kept seven doctor checks, now covering actual committed fixtures, recursive Rust
  sources, Cargo.toml/Cargo.lock, and all six tool shapes under both server-name styles.
  Missing/corrupt/wrong-shape fixtures and missing/newer build inputs fail diagnostics.
- Reconciled SPEC, README, research traceability, schema tolerance, historical reminder
  dates and the superseded project-profile plan. Python migration was already completed;
  archived source/tests remain untouched.

The current canonical source is the
[official MCP reconciliation](higgsfield-canonical-contract.md) and sanitized capture:
79 scoped MCP models, six generation schemas, all catalog cursors exhausted. The
separate CLI catalog is not an alias map. A test cross-checks compiled resolution
associations and duration ids against that saved MCP evidence. Nothing fetches schemas
at hook time.

## Baseline and deliberate behavior changes

Baseline release: 80 unit tests, four doctor tests, four fail-silent tests, and one
347-input golden test passed; debug panic controls passed and doctor reported seven PASS.
Direct probes reproduced the 8K style-header and quoted-4K false positives and the
Nano Banana 2k missed contradiction. The initial combined canonical control failed
before the runtime correction, first at the 8K assertion; its remaining assertions and
the expanded semantic suite now pass. This is not a claim that every later test was
independently demonstrated red.

All 347 historical input byte lengths/hashes remain unchanged. Exactly 87 historical
outputs changed, each removing only an unsupported resolution finding and updating the
header count; remaining conflict text is unchanged. The captured Kling fixture now
produces only prompt_negative_conflict. Seedance remains silent. Added 191 canonical
semantic cases, including each mapped carrier in single/batch contexts.

The [278-row ledger](golden-change-ledger.json) gives the exact before/after outputs and
specific reasons for 87 modifications and 191 additions. A read-only integrity check
validated ledger/output agreement, preserved hashes, no removed labels, and SPEC's
example against the actual fixture golden. Expectations were derived from the intended
contract, not bulk regenerated from current binary output.

## Verification

| Check | Result |
|---|---|
| `cargo test --release` | 82 unit + 4 doctor + 4 fail-silent + 3 parity/semantic/coverage tests pass |
| Golden parity | 538 distinct inputs / 538 entries; 0 divergences, missing, orphaned or changed-input cases |
| Mandatory semantic negatives | 59 / 59 silent |
| Supported resolution positives | 92 / 92 correct |
| Supported local-conflict positives | 9 / 9 correct |
| Supported duration positives | 16 / 16 correct |
| Robustness semantic cases | 17 exact expected outcomes |
| Accepted semantic limitations | 3, actual/ideal outcomes asserted separately |
| Research identifiers | 57 covered, 8 deferred, 0 accepted = 65 |
| Selected repeated process inputs | 13; identical exit/stdout/stderr |
| `cargo test --test fail_silent` | 4 pass, including genuine debug-only panic |
| Explicit release build + doctor | Build succeeds; all 7 checks PASS |
| `cargo clippy --all-targets -- -D warnings` | Clean |
| `cargo fmt --check` and diff validation | Clean |

The one ignored test is the intentionally read-only golden mismatch inspector, not an
unrun acceptance gate. Each semantic positive denominator counts a distinct input
once per rule, including batches. Counts are not real-world precision/recall estimates.
The [65-ID table](../rust/tests/golden/README.md) validates every label/test reference.
Some research examples use explicitly marked equivalent controls: full published
cowboy/drama templates have not been captured as complete fixtures. Deferred ceilings,
ratios, parameter conflicts and clamps do not inflate current positive counts.

The release dependency manifest/lockfile, hook settings, captured fixtures and Python
archive have no changes. Release retains `panic = "unwind"`. Rustfmt was installed
into the existing local stable toolchain for formatting; no project dependency added.

## Complexity discrimination

The production quote membership search remains partition_point binary search.
A closure counts actual comparisons in tests at 0, 1, 8, 8,000 and 128,000 spans,
checking boundaries/gaps and a logarithmic bound. A separate long-quote functional test
requires silence until a final unquoted conflict appears.

In an isolated temporary checkout copy, replacing only this lookup with a linear
take_while scan made the guard fail: **8 spans: 8 comparisons > 6**. Production sources
were not changed for that experiment. This proves the guard detects the intended
regression; it does not claim that full prompt processing is sublinear in input length.

## Same-machine performance

[Raw measurements and input recipe](evidence/implementation-performance-2026-09-09.json).
Apple M4, arm64, Darwin 25.5.0, release profile. Five warmups for each case/build, then
100 samples per ordinary case or five per large case. Includes process launch,
stdin/stdout and the hook's JSON parsing; external output-validation parsing is after
the timer. RSS is a separate `/usr/bin/time -l` run. Inputs are identical across builds;
output semantics intentionally differ. Outputs were stable within each case/build.

| Case | Baseline median ms | Revised median ms | Revised p95 / p99 / max ms |
|---|---:|---:|---:|
| clean | 1.43 | 1.738 | 2.065 / 2.446 / 3.422 |
| resolution | 1.452 | 1.795 | 2.264 / 3.744 / 3.937 |
| conflict | 1.824 | 2.091 | 2.496 / 2.688 / 2.808 |
| duration | 1.453 | 1.842 | 2.101 / 2.21 / 2.253 |
| all | 1.968 | 2.113 | 2.765 / 6.694 / 6.891 |
| unicode | 1.813 | 2.103 | 2.519 / 3.029 / 3.37 |
| batch | 1.456 | 1.816 | 1.99 / 2.085 / 2.509 |
| malformed | 1.431 | 1.432 | 1.65 / 1.74 / 1.76 |

| Large input | Baseline median / max ms | Revised median / max ms | Baseline / revised peak RSS MiB |
|---|---:|---:|---:|
| quoted_3mb (3680173 bytes) | 76.475 / 77.558 | 129.961 / 132.845 | 24.84 / 21.88 |
| unicode_3mb (2730173 bytes) | 59.675 / 59.838 | 65.905 / 68.72 | 18.48 / 17.16 |
| array_4mb (4194320 bytes) | 23.315 / 26.154 | 23.186 / 24.062 | 72.14 / 72.08 |

This is not a speedup claim. Ordinary checked calls add roughly 0.15–0.39 ms median
in the final run. Source inspection identifies extra normalization/scanning and
duration regex initialization in each short-lived process as added work; this is an
explanation, not a profiler-derived attribution. The quote-heavy case grows from
76.475 to 129.961 ms median (about 70%): each eligible rule currently builds/scans its
own text view. Its observed peak RSS is lower, not larger, than baseline. The 4MB
wide-array case uses about 72 MiB because serde_json materializes the input tree;
it is not solved by avoiding a normalization source map.

An earlier run overlapping compile/test activity had a 326.570 ms ordinary-call maximum.
That run is retained in the raw artifact. Final measurements were repeated after those
jobs completed. OS scheduling/thermals are uncontrolled, builds are measured sequentially,
and 100 samples give only coarse tail estimates. No portable 250ms SLO or flaky
wall-clock test was introduced. Shared per-entry text views are a possible optimization
if real workload evidence justifies the added ownership complexity.

## Boundaries and remaining work

Known accepted cases: negated resolution can still fire; a scene-level “holds for
12 seconds” may be read as a total over-claim; unrelated inch marks can pair as quotes
and hide a conflict. Each has actual and ideal ids in semantic data. Narrow folding
is not general Unicode normalization or multilingual understanding.

Unsupported/ambiguous models, absent controls, mixed geometry, marketing resolution
versus explicit dimensions, Seedance fast unsupported selections, and ignored/source-
driven duration modes stay excluded. Topaz positive tiny targets are deliberately
supported as requested pixels, without the obsolete generic-dimension 128px floor.
video_deflicker is conservatively excluded as a source-processing workflow despite an
optional duration field. Schema-invalid tolerance does not certify server acceptance.

HFR-002 ceilings, HFR-003 ratios, HFR-004/005/007 parameter validation, HFR-006 negation
density and a CLI adapter remain deferred per PLAN. The reminder specification now uses
an inclusive seven-day boundary, but no task deployment or account balance was verified.

Doctor proves this checkout's local configuration and behavior, not that another active
Claude session loaded it. Inspect `/hooks` and a target-session trace before relying on
interception. Higgsfield CLI commands, direct HTTP, other clients/projects and unmatched
workflows can bypass this MCP hook. No live generation was sent for verification.
