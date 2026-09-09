# Finish Rule Re-grounding and Production Alignment Plan

## Execution status — 2026-09-09

The local Rust implementation is complete; see the
[implementation report](docs/rule-regrounding-implementation-report.md) for measured
results, [golden ledger](docs/golden-change-ledger.json) for every changed/new output,
and [research mapping](rust/tests/golden/README.md) for all 65 research identifiers.

| Work package | Outcome |
|---|---|
| 0 canonical evidence / migration | Official MCP acquisition complete for six generation tools; Python-to-Rust migration was already complete |
| 2 context / retired loader | ToolKind wired, audio skipped, unknown scope narrowed, models.rs deleted |
| 1 resolution | Canonical model/field/option comparisons; unsupported dimensions and 8K style assertions removed |
| 3 text | Narrow folding, quote exclusions, original spelling and bounded source-range recovery |
| 4 carriers | Verified resolution/quality/mode/Topaz target associations; ambiguous geometry/precedence excluded |
| 5 duration | Video/model/mode gates, strict numeric grammar, supplied-duration-only messages |
| 6 doctor | Seven checks retained; actual fixtures and recursive sources/manifests/lockfile inspected |
| 7 evidence | 538 goldens, 191 semantic cases, 65-ID traceability, deterministic complexity guard and measured performance |
| 8 handoff | Current SPEC/README/dates reconciled; local build/test/doctor verification recorded in report |

Execution refinements: Topaz's declared positive target pixels do not inherit the
retired generic-dimension 128px floor. Unknown tools cannot assert resolution.
Research template coverage is explicitly representative, not a claim of full published
prompt capture. Semantic positive denominators count each input once per rule.
No current-output bulk golden regeneration was added.

Local completion is not global deployment: target-session `/hooks` inspection and
a real loaded-hook trace remain unperformed. No generation, CLI adapter, global hook
installation, recurring-task mutation, commit or push was performed. Deferred HFR
rules remain deferred. The detailed work-package boxes below preserve the original
execution recipe rather than asserting that every proposed intermediate test run was
performed in precisely that order; this status and the evidence report govern completion.


**Status:** Revised implementation proposal, 2026-09-09. Authenticated canonical MCP/CLI reconciliation is now captured in docs; runtime, SPEC, and golden changes remain future implementation. Checked evidence items report completed review work, not a shipped behavior change.

**Goal:** Make the specification, shipped hook, regression corpus, and installation guidance agree; remove demonstrated false positives; detect well-grounded prompt/parameter contradictions; and make failures diagnosable without adding noise to generation calls.

**First prerequisite — canonical reconciliation:** Read all of `HIGGSFIELD_PROMPT_RESEARCH.md`, then reconcile the MCP envelope, model capabilities, and prompting guidance separately against canonical Higgsfield sources. Task 0 below owns this gate. Existing SPEC text and historical golden outputs are not authority for what Higgsfield actually honors. Any downstream mapping, precedence, message, or fixture expectation contradicted by canonical evidence must be revised explicitly before implementation.

**Success means:** the known-good prompts stay quiet, supported contradictions produce useful advice, every check runs only in its intended tool context, a fresh binary is verifiable, and future maintenance does not depend on guessing what a passing golden test proves. The historical usage figures explain the product's focus on attention and rework; they are not current account statistics or a forecast of business outcomes.

**Revision target:** This is the enhanced version of the original `PLAN.md`. The other two supplied proposals remain reference documents. This plan supersedes conflicting implementation instructions in those proposals and the unfinished portions of `docs/superpowers/plans/2026-08-31-rule-regrounding.md`. It does not restart completed work.

**Starting point:** `feat/doctor-and-expiry` at `a7e0039`. Recheck HEAD before execution. If it moved, reconcile the actual diff and completed steps instead of resetting the worktree or applying stale snippets.

**Architecture:** Keep one advisory Rust binary. The final runtime modules are `main`, `lint`, `checks`, and `doctor`. Extract tool context once, preserve the existing parameter resolver and batch traversal, share quote and text-matching primitives, and remove the retired model-list loader. No service, proxy, runtime LLM, network dependency, telemetry pipeline, or new crate.

**Stack:** Rust 2021; `serde_json` and `regex`; Claude Code `PreToolUse`. `SPEC.md` is the intended behavior contract; the Rust and golden corpus show implemented behavior. “The Rust is the specification” means the archived Python is no longer an oracle, not that a known code/spec disagreement is acceptable.

## 1. Evidence, baseline, and limits

### 1.1 What is already complete

Do not repeat these tasks:

| Completed work | Local evidence |
|---|---|
| Rust port and reviewed golden corpus | `rust/tests/parity.rs`; archived port README |
| Diagnostic subcommand and dispatch isolation | `rust/src/doctor.rs`; `rust/tests/fail_silent.rs` |
| Disable `negation_placement` | No runtime call in `lint_params`; SPEC §5.3 |
| Retire the negative-prompt model data | File absent; doctor checks its absence |
| Recalibrate positive/negative conflicts | Both-field negation stripping, quoted positive literals, curated phrases |
| Demote ambiguous `letterbox` and `watermark` | `letterbox bars` and `watermark overlay` in the lexicon |
| Binary-search quote exclusions and avoid repeated full-text character arrays | `is_excluded` and `find_term_match_excluding` |
| Approve the new resolution lexicon in the spec | Commit `a7e0039`, SPEC §5.1 |

No applicable `AGENTS.md` was found in this checkout or its ancestor directories during this review. Recheck if the execution environment changes.

### 1.1a Python-to-Rust migration: completed, not upcoming

The migration was planned in `docs/superpowers/plans/2026-08-31-rust-port.md` and has already landed on this branch:

| Milestone | Evidence |
|---|---|
| Rust runtime implemented and hook switched | `58c046c`; current `.claude/settings.json` invokes `target/release/higgsfield-lint` |
| Python/Rust differential verification completed | Archive README records 320 deterministic cases with zero divergences |
| Differential harness replaced by golden-output regression | `95edc00`; current `rust/tests/parity.rs` starts only the compiled Rust executable |
| Python implementation and tests archived | `a1b0e68`; `docs/archive/verified-port/` |

The archive also records a separate 4,200-case fuzz run; that is historical reported evidence, not a reproducible checked-in migration test. There is no planned Python fallback, dual runtime, or second port. Remaining work changes the Rust rules and their contracts.

Research §§2, 8, 12, and 13 contain older descriptions of a live Python oracle and three active rules. Interpret those as a dated audit, not instructions to restore Python or the disabled negation-placement rule.

### 1.2 Verification actually performed for this revision

| Check | Observed result on the starting checkout |
|---|---|
| `cargo test --release` | 80 unit, 4 doctor, 4 fail-silent, 1 parity test passed |
| `cargo test --release --test parity -- --nocapture` | 347 distinct inputs, 347 expectations, zero divergences |
| `./target/release/higgsfield-lint doctor` | Seven PASS, no WARN/FAIL, exit 0 |
| `cargo test --test fail_silent` | Four passed, including the debug-only injected panic |
| Nine direct stdin probes | Confirmed the behavior in the table below; every probe exited 0 with empty stderr |

These are baseline results, not evidence that the proposed changes are implemented. A release test containing the debug-only panic field does not exercise the panic injection; the debug run above does.

| Probe | Current result | Intended disposition |
|---|---|---|
| Official-style `8K IMAX` header, resolution 1080p | Resolution finding | Silence |
| Presenter says `"shot in 4K"`, dimensions 1920×1080 | Resolution finding | Silence |
| `4K poster`, resolution `2k` | Silence | Resolution finding |
| Curly-quoted `“close-up”` against negative `close-up` | Conflict finding | Silence |
| Unquoted `slow   motion` against `slow motion` | Silence | Conflict finding |
| `Total: 15s`, duration 5, video tool | Silence | Duration finding |
| `not 4k`, dimensions 1280×720 | Resolution finding | Record current limitation; defer semantic change |
| `wan2_6`, quality 720p, prompt 4K | Silence | Carrier-aware finding after evidence check |
| Audio tool with 4K/handheld plus conflicting fields | Two findings | Silence |

### 1.3 Evidence hierarchy and freshness

1. Explicit task scope and accepted repository contracts determine what work is authorized.
2. SPEC determines intended local behavior. Amend it explicitly for new behavior; never weaken it merely to fit the old binary.
3. Read `HIGGSFIELD_PROMPT_RESEARCH.md` in full: §§3–7 establish evidence classes, platform/model differences, and what the fields mean; §§8–14 audit behavior and propose rules; §15 identifies sources. Its model lists and usage figures are dated evidence.
4. Current source, fixtures, tests, and direct probes determine what actually runs.
5. Earlier review narratives are useful leads, but their claimed live catalog reads, account observations, and hook traces are not new measurements by this executor.

The [official Seedance prompting guide](https://higgsfield.ai/blog/seedance-prompting-guide) was reopened for this review and still instructs authors to state shot count, total duration, and aspect ratio. That supports a bounded duration check; it does not establish model-specific output ceilings.

The [Claude Code hook reference](https://code.claude.com/docs/en/hooks) was also reopened. It documents project and user configuration, structured context, exit-code handling, and interactive hook inspection. Preserve this repository's stronger zero-exit/empty-stderr contract. Do not repeat the old comment that any stderr is intrinsically blocking: blocking depends on the hook's decision and exit behavior.

Follow-up verification on 2026-09-09: the complete 743-line research document was read. The official MCP, Kling, and prompting help pages were reopened. The [Kling table](https://higgsfield.ai/creator-hub/help-center/ai-models/how-do-i-use-kling) still lists Kling 2.6 at a maximum of 1080p, and the [prompting guidance](https://higgsfield.ai/creator-hub/help-center/getting-started/how-do-i-write-a-good-prompt) still permits inline exclusions. These prose checks do not establish the current model parameter schemas.

Authenticated reconciliation completed on 2026-09-09 following the [official connection guide](https://higgsfield.ai/creator-hub/help-center/integrations/how-do-i-connect-higgsfield-to-ai-agent). Installed official CLI 1.1.24, completed OAuth, selected the sole workspace, and used the authenticated connection for read-only MCP discovery. The earlier anonymous401 is no longer a blocker.

The MCP server reports Higgsfield AI 1.0.0. All101 tool names and all six generation input schemas were captured. Fully paginated MCP catalogs contain79 scoped models (34 image, 39 video, 6 audio); 16 relevant details were fetched. CLI separately returned90 all-type entries and 72 scoped model details. The catalogs differ in counts and ids: never substitute CLI ids for this MCP hook.

The [canonical contract matrix](docs/higgsfield-canonical-contract.md) and [sanitized raw evidence](docs/evidence/higgsfield-contract-2026-09-09.json) now govern the plan. Key corrections: video dimensions may derive ratio rather than output size; K labels are nominal tiers, not universally short-side pixels; all three batches require object params; no negative_prompt parameter is published in the 79-model MCP catalog; Seedance 2.5 edit modes require duration exceptions. No generation, upload, cost preflight, or purchase was submitted. Credentials/account data are excluded from the artifacts.

The [official MCP page](https://higgsfield.ai/mcp) recommends CLI for coding agents. That is an access recommendation, not proof of this project's deployment coverage: its Claude Code matcher intercepts six MCP generation names only, not shell CLI commands, dedicated upscalers, or workflow tools. A separate adapter is a successor task, not silently added to this release.

## 2. Design decisions and tradeoffs

The larger plan is justified by additional contracts and verification, not by adding every possible rule. The following decisions reconcile the competing recommendations.

| Area | Adopted decision | Reason and consequence |
|---|---|---|
| Canonical contract | Complete Task 0's evidence matrix before schema-dependent implementation | Permissive JSON acceptance is not evidence of model behavior |
| Resolution fix | Keep approved lexicon/quote fixes; replace generic pixel arithmetic with model-scoped typed comparisons | Nominal tiers and passthrough dimensions do not prove universal short-side geometry |
| Execution detail | Keep exact tests and golden-review steps | A prose acceptance criterion alone does not protect against vacuous tests |
| Tool context | Introduce `ToolKind` once and remove unused model plumbing in the same task | Avoid successive incompatible signatures and recurring filesystem probes |
| Duration scope | Explicitly video-only, including batch | Relying on image tools usually lacking `duration` does not implement a video-only contract |
| Duration direction | Over-claims only; check id `duration_claim` | Under-claims confuse scene beats with total length |
| Unicode | Narrow documented folds plus whitespace handling, no new crate | Fix reproduced gaps without pretending to implement all NFKC |
| Quoted text | Exclude quoted positive content in all claim checks | Curly quotes should not turn dialogue into a directive |
| Original spelling | Recover the matched substring from the original prompt | A folded `4K` string cannot itself reproduce `４Ｋ` |
| Alternate carriers | Use verified field meaning and precedence; retain compatibility only where it does not contradict that evidence | A familiar value or submitted dimension alone does not prove output size |
| Advice | Name the carrier that was read | A quality-based finding must not tell the user to change an absent `resolution` parameter |
| Diagnostics | Add real-fixture coverage under existing check 7 | Useful positive and negative controls without changing healthy PASS count |
| Test metrics | Explicit semantic membership, counts, denominators, separate timing | Label prefixes and corpus size are not a definition of accuracy |
| Performance | Deterministic complexity checks plus separate benchmarks | Larger timing ratios can still reject correct code or pass under noisy measurements |
| Cleanup | Remove active retired-file loading; keep deliberately retained noise filters | The first eliminates real runtime work; the second is an explicit SPEC decision |
| Installation | Keep project scope, document coverage and verification | Global installation is a distinct deployment choice, not a prerequisite to fixing the code |
| Negated resolution | Preserve and measure the existing limitation | Reusing the alphabetic conflict negation regex would not remove numeric `4k` anyway |
| Ceiling/ratio rules | Preserve concrete successor designs with entry criteria | Valuable future work must not hide stale-evidence and parser risks |
| Account reminder | Correct historical arithmetic and stale-data handling in documentation | Do not report September 4 as an upcoming date on September 9 |
| Scheduling/publishing | No automated installation, pushes, or recurring jobs in this plan's implementation scope | These are independent operational actions, not correctness gates |

### 2.1 Resolved choices

No preliminary user decisions are needed to execute the core after implementation is requested:

- Keep the current project-local hook.
- Use `ToolKind::{Image, Video, Audio, Unknown}`.
- Use `duration_claim` as the public id.
- Keep dependencies unchanged.
- Unknown tools retain only local prompt/negative conflict checks; resolution and duration require known tool context.
- Keep explicit negative-resolution wording as a documented limitation for now.
- Keep HFR-002, HFR-003, and HFR-006 outside the core release, with the revisit criteria in §6.
- Do not require unavailable skills, fixed model names, or multi-agent execution. A single capable executor can perform every task.

### 2.2 Product constraints

- Advisory findings only: no `permissionDecision`, `continue: false`, payload rewriting, or tool blocking.
- On the hook path, malformed input and caught internal panics produce exit 0, empty stderr, and no output.
- A valid call with no findings produces no stdout. No heartbeat or informational line in hook output.
- Keep the no-op panic hook and `catch_unwind`; keep release `panic = "unwind"`.
- Abort, allocator exhaustion, OS termination, and failed process launch are not catchable Rust panics. Do not promise immunity to them or give abort an invented fixed exit code.
- Runtime rules perform no network or MCP calls and no filesystem writes. After Task 2, the hook needs no application data-file reads. Doctor may read its checkout.
- Preserve parameter object/string tolerance, batch traversal, explicit-index formatting, and per-entry ordering.
- Rules remain curated, bounded, deterministic checks. No style inference, stemming, broad NLP, or arbitrary adjective scoring.
- The archived Python and historical fixtures remain source evidence. Never rewrite the oracle to fit a rule change.
- Keep `quoted_spans` as the quote-pairing authority and sorted, non-overlapping exclusion ranges.
- Each behavior change lands with its spec, tests, and individually justified golden updates. Local red tests are expected during development; proposed commit boundaries must pass.
- Precision is the priority. Known limitations must be named, counted, and distinguished from required silent controls.

## 3. Work packages and dependency order

| Task | Outcome | Depends on | Release priority |
|---|---|---|---|
| 0 | Read full research; reconcile canonical envelopes/capabilities; verify completed migration and baseline | — | Before schema-dependent edits |
| 1 | Resolution lexicon/quotes, typed comparisons, canonical field corrections | 0, 2 | Critical |
| 2 | Tool classification, audio skip, removal of model loader | 0 | Critical |
| 3 | Typographic quotes, fullwidth ASCII, whitespace matching, original-span recovery | 1–2 | Functional |
| 4 | Exact carrier provenance and model-gated fallback support | 1–2; fresh carrier evidence | Functional |
| 5 | Model/mode-eligible video supplied-duration over-claims | 0, 2–3 | Functional |
| 6 | Fixture-backed doctor and deployment verification | 2–5 | Reliability |
| 7 | Coverage accounting, deterministic complexity protection, measured performance | 1–6 | Reliability |
| 8 | Final docs, metadata, release and rollback handoff | 1–7 | Completion |

Recommended implementation order is **0 → 2 → 1 → 3 → 4 → 5 → 6 → 7 → 8**. Task numbers remain stable for review references; Task 2 now supplies tool context before Task 1's model-scoped resolution corrections. The dated canonical capture is complete for the six-tool scope. Refresh on drift, and preserve explicit semantic exclusions rather than blocking supported fixes on unknowable geometry. Task 4 consumes the completed matrix.

Each completed task report records: affected files; intended behavior; tests and observed result; every changed/new/deleted golden label and reason; limitations; and unresolved dependencies. Keep one concise tracked implementation report at `docs/rule-regrounding-implementation-report.md` when execution begins. This file is an execution deliverable, not something already completed by the plan revision.

### 3.1 Files and ownership of responsibilities

| File | Planned responsibility |
|---|---|
| `rust/src/main.rs` | Extract tool name, classify once, invoke lint; retain dispatch and failure behavior |
| `rust/src/lint.rs` | `ToolKind`, parameter resolution, batch context, stable check order |
| `rust/src/checks.rs` | Resolution rules, normalized matching, carrier provenance, bounded duration parser |
| `rust/src/models.rs` | Delete in Task 2 after removing every caller |
| `rust/src/doctor.rs` | Root discovery, freshness inputs, fixture self-test, matcher coverage, diagnostic caveat |
| `rust/tests/parity.rs` | Stable corpus inputs, new cases, semantic coverage groups and output comparison |
| `rust/tests/golden/expectations.json` | Individually reviewed expected behavior and accurate provenance |
| `rust/tests/golden/README.md` | Capture discipline plus semantic-case coverage documentation |
| `rust/tests/fail_silent.rs` | Real process failure/dispatch protections |
| `rust/tests/doctor.rs` | Real checkout/environment wiring protections |
| `SPEC.md` | Intended rule contracts, accepted costs, documentation corrections |
| `README.md` | Build, diagnostics, hook loading, scope, rebuild workflow |
| `docs/credit-expiry-task.md` | Historical anchor and boundary-correct warning schedule description |
| `docs/superpowers/plans/2026-08-30-project-intent-profile.md` | One superseded-status banner; retain historical body |
| `docs/rule-regrounding-implementation-report.md` | Decisions, verification, per-label change ledger |
| `docs/higgsfield-canonical-contract.md` | Completed dated reconciliation, binding decisions, source paths and planned tests |
| `docs/evidence/higgsfield-contract-2026-09-09.json` | Sanitized MCP envelopes, paginated catalogs, model details, and separate CLI evidence |

No new runtime data file is needed for the core. Retain `data/.gitkeep` because doctor still uses `data/` as its repository landmark. A future root-discovery redesign is separate.

### Task 0: Establish a trustworthy execution baseline

- [x] Read all 743 lines of `HIGGSFIELD_PROMPT_RESEARCH.md`; completed work and remaining disagreements recorded above.
- [x] Capture and reconcile canonical MCP envelopes, scoped catalogs, and model details; record semantic unknowns as exclusions in the matrix.
- [x] Verify completed Python-to-Rust migration from hook, harness, archive and commits; no second migration scheduled.
- [ ] Read the current SPEC, this plan, golden README, and current implementations before editing.
- [ ] Check `git status --short` and `git log -5 --oneline`. Preserve unrelated and untracked work. The three input plans were untracked at review time.
- [ ] Run `cargo test --release`, `cargo test --test fail_silent`, and the release doctor. Record actual counts and environment caveats.
- [ ] Reproduce the two core resolution false positives and `2k` miss. Read emitted findings, not merely exit status.
- [ ] Establish the implementation report and enumerate the intended new controls before changing expectations.
- [ ] If a baseline check fails, determine whether it is an environment mismatch, existing failure, or changed HEAD. Do not edit golden data to conceal it.
- [ ] Record future implementation changes as reviewed local changes; staging/committing/pushing follows the actual execution request.

The full release suite is required at each behavior boundary. A docs-only correction after the last green run needs link/example/diff validation, not another unchanged full test run.

#### 0.1 Three different contracts to reconcile

| Layer | Canonical evidence | What it can establish |
|---|---|---|
| MCP transport/tool envelope | Fresh `tools/list` input schemas from the official connected server | Names, required fields, types, object/string alternatives, batch item constraints |
| Model capabilities | Fresh fully paginated `models_explore` catalog and relevant model detail | Actual model ids, supported controls/options, ranges, defaults, declared adjustments and constraints |
| Prompting guidance | First-party help/model guides with retrieval dates | Recommended prompt language, inline exclusions, quoted literals, duration/ratio conventions |

Do not infer model support from `additionalProperties`, nor derive a rigid universal prompt schema from one model's example template. Official guides differ by workflow, as research §5.3 explicitly records.

#### 0.2 Capture and traceability deliverable

- [x] Capture all six current generation input schemas and 101 discovered names; one tools/list page, no next cursor.
- [x] Exhaust MCP catalog cursors:34 image in 2 pages, 39 video in 2 pages, 6 audio in 1 page; 79 unique ids.
- [x] Fetch 16 targeted MCP details plus all 72 scoped CLI details; record Soul ids, Wan quality, Kling mode, Topaz targets and edit-mode exceptions.
- [x] Save dated, sanitized schemas/catalogs with endpoint, protocol/server/CLI versions and source paths; exclude credentials and account state.
- [x] Write `docs/higgsfield-canonical-contract.md` with field/type/effect/source/current behavior/decision/test matrix.
- [ ] Distinguish confirmed output control, aspect-ratio input, stored metadata, opaque passthrough, and unknown effect. Absence from a catalog is not proof the backend ignores a field.
- [ ] Identify and explicitly amend affected SPEC clauses before editing Rust. Later task snippets are provisional wherever their semantics depend on an unresolved row.

This is design-time evidence, not a new runtime schema service or hook-time network call.

#### 0.3 Issues that must be resolved

| Research finding | Required reconciliation |
|---|---|
| Single-call object/string params versus batch object params (§2.4) | Inspect each actual envelope independently; keep tolerance tests separate from schema-valid fixtures |
| No published negative_prompt support in the recorded catalog (§§5.1, 6) | Verify current evidence; retain overlap lint only as an explicitly labeled local textual consistency convention unless model support is established |
| Kling 2.6 historical width/height fields were not declared (§§5.1, 8.1) | Do not assert that those submitted numbers are the generated output dimensions; determine whether they affect output, ratio, metadata, or remain unverified |
| At least five incompatible resolution vocabularies (§5.1) | Resolve field and model semantics; distinguish nominal size tiers, long side, short side, pixel counts, and fixed model ceilings |
| Soul quality `1.5k/2k` versus generic quality `low/medium/high` | Include verified image carriers; do not limit Task 4's review to two video models |
| Additional recorded values such as `768p`, `1080`, and `360p` | Inventory them, then explicitly support or defer each; do not claim the existing ten-value table reconciles the whole catalog |
| Duration may be clamped or adjusted (§§5.1, 9, HFR-007) | Distinguish requested duration from actual/effective duration; the base rule compares supplied values, not a guaranteed render outcome |
| Model ceilings and aspect-ratio mismatches are high-priority recommendations (§14) | Record their coverage gaps even while their implementation remains deferred; do not label the release “all canonical rules implemented” |

**Output-claim policy:** A field may only support a claim about generated output when its meaning is established for that tool/model. Otherwise suppress that output claim or explicitly describe submitted-value arithmetic as a local consistency check. A documentation caveat cannot repair a runtime message that asserts an unverified output. Resolve the choice per evidence row before finalizing code and golden expectations.

**Precedence policy:** Proven output controls take precedence over unverified passthrough dimensions. If canonical evidence does not resolve conflicting controls, do not guess an actual output size. Preserve old order only where compatible with the reconciled field meanings.

**Fixture policy:** Preserve captured historical input bytes as evidence, but revise their expected findings if the old expectation relied on unsupported semantics. In particular, the historical Kling fixture's two findings are not an immutable product requirement. Update doctor's expected ids and all later acceptance examples coherently after Task 0; keep a separate confirmed positive control for every active rule.

**Exit criterion:** Every field used by an active/planned check has a dated evidence row or is explicitly classified as a local convention/unknown. Acquisition, SPEC and supported runtime associations now meet this criterion; see the dated execution report. Unknown geometry/precedence is excluded from output assertions. Rust migration remains already complete.

---

### Task 1: Reconcile SPEC §5.1 and implement grounded resolution comparisons

**Files:** `rust/src/checks.rs`, `rust/tests/parity.rs`, golden expectations, SPEC §5.1/§6/§9, doctor self-test.

**Dependency:** Complete Task 2's ToolKind wiring first. The previous generic arithmetic implementation is replaced, not merely given more string aliases. The [canonical matrix](docs/higgsfield-canonical-contract.md) is binding for field meaning.

**Outcome:** Remove the demonstrated noisy claims, exclude quoted content, recognize supported model output tiers, and stop presenting arbitrary passthrough dimensions or nominal K labels as guaranteed output pixels.

#### 1.1 Amend the behavioral contract before code

- Remove `8k`, `uhd`, and `full hd` from active claim recognition; retain them as permanent negative controls.
- Retain claim spellings `4k`, `1080p`, `720p`, `480p`. The local 4K→2160 threshold convention is applicable only to an explicitly labeled target-pixel comparison, not every model field.
- Replace universal `RESOLUTION_PARAM -> short_side` arithmetic with typed output-tier/target-pixel comparisons. Do not install the proposed `2k=2048`, `1.5k=1536`, or `1k=1024` short-side mappings.
- Require a known tool kind and a supported model/field/option association for output-size claims. Unknown tools/models remain silent for resolution; local prompt/negative consistency can still run.
- Suppress generic `width`/`height` output inference. Video envelope describes ratio derivation; Seedream describes stored dimensions; neither justifies the historical universal output assertion.
- Distinguish explicit submitted settings from defaults and eventual output. Do not synthesize defaults or promise a render result.
- Preserve one finding per rule/entry and existing finding order.
- Keep the retired model-list loader removed; the small capability associations are source constants backed by dated evidence, not filesystem/config state.

The historical Kling fixture now expects only `prompt_negative_conflict`. Do not change its input to preserve its old resolution warning. SPEC's recorded historical observations remain historical; its current example/output must reflect this correction.

#### 1.2 Comparison domains

| Domain | Initial support | Comparison / wording |
|---|---|---|
| Nominal image/video K tier | Exact model's declared resolution or quality options 1k/1.5k/2k/4k | Compare like K labels; 4K against 2k fires; do not derive pixel geometry |
| P-labelled video tier | Exact model's declared 360p/480p/720p/768p/1080p/2160p options | Order p-labelled options; 4K vs a sub-4K video tier uses an explicitly documented local label convention, not a universal short side |
| Explicit requested target pixels | Task 4's Topaz output_width/output_height associations | Compare positive integer target pair using min(targets) and the documented local claim thresholds; describe requested dimensions |
| Mixed nominal image K and p claim | No proven conversion | Silence, not invented common units |
| Bare width/height, unknown model/option, conflicting declared controls | No proven controlling output | Silence |

Use exact source values in advice. Example:

```text
resolution_claim: prompt says "4K" but resolution selects the "2k" output tier. Choose a supported higher tier, or revise the claim.
```

This replaces “the call produces a 2048px short side.” Wan advice names quality; Topaz advice names output_width/output_height. A mode 4k equality case produces no warning, so no fictitious lower-mode positive is needed.

#### 1.3 Quote-aware original text

Keep the existing quote primitives and add/reuse an original-case excluding helper:

```rust
pub fn find_original_case_excluding(
    text: &str,
    term: &str,
    excluded: &[(usize, usize)],
) -> Option<String> {
    find_term_match_excluding(text, term, true, excluded)
        .map(|(start, end)| text[start..end].to_string())
}
```

Compute quote spans once per scanned representation. Before Task 3 typography folding, these offsets are original-text offsets. After folding, follow Task 3's original-range recovery contract; never index the original string with normalized byte offsets. Keep the old helper as a wrapper if callers still require it.

Pair each quoted-negative control with an identical unquoted positive on a model/setting that can actually produce a resolution finding. Unknown-model silence must not masquerade as successful quote exclusion. When quoted and unquoted instances coexist, use an eligible unquoted match and preserve its original spelling.

#### 1.4 Required failing and retained-behavior controls

| Case | Params/context | Expected after implementation |
|---|---|---|
| Official 8K IMAX header | seedance_2_0, resolution 1080p, mode std, Video | Silence; current lexicon bug reproduced |
| Quoted4K only | nano_banana_pro, resolution 2k, Image | Silence |
| Unquoted4K plus quoted4K | Same model/tier | One resolution finding from unquoted occurrence |
| 4K poster | nano_banana_pro, resolution 2k, rectangular aspect ratio | Nominal-tier warning; no 2048/short-side promise |
| 4K vs 4k | Matching declared tier | Silence |
| 4K vs quality 720p | wan2_6, Video | Quality-specific finding after Task 4 |
| 4K with width1376/height768 | Historical kling2_6 fixture | No resolution finding; conflict remains |
| 4K with arbitrary width/height | Unknown model or tool | No resolution finding |
| 4K with resolution 1080p on kling2_6 | Undeclared field | No resolution finding |
| 4K with nominal 1.5k tier | seedream_v5_pro, Image | Tier warning; no 1536px assertion |
| 1080p claim vs nominal image2k | nano_banana_pro, Image | Silence: mixed-unit geometry unresolved |
| Target-pixel malformed values | Topaz target fields: bool/string/float/null/negative/zero | Silence |
| Valid asymmetric target | Topaz output 1920×100, Image | Local requested-target finding; compare with bool counterpart |
| Target 3840×2160 | Topaz, 4K | Silence under documented local threshold convention |
| Missing/wrong prompt | Any otherwise supported source | Silence |
| Unknown resolution value | auto/native or unsupported option | Silence unless a separate valid carrier is unambiguously controlling |

Task 1 implements the base declared-resolution associations; Task 4 extends quality/mode/target-pixel carriers using the same representation. Every association test uses its correct tool kind. A single payload switched from video to image is not automatically a valid image positive if its model is video-only.

#### 1.5 Golden migration without losing evidence

1. Keep old8k/UHD/full hd labels and input bytes. Review their flips to silence rather than deleting the cases.
2. Preserve historical generic dimension sweeps as schema-invalid/local robustness evidence, with corrected silence expectations; add separate Topaz target-pixel sweeps. Do not quietly replace the old captured fields.
3. Preserve prior model-less resolution sweeps as unknown-model controls. Add new labeled model-scoped tier cases instead of pretending old inputs prove model support.
4. Maintain boolean/float/overflow discrimination with explicit positive target controls. A malformed input test that passes because no model is recognized proves nothing about numeric validation.
5. Add new labels for actual declared vocabulary and deliberate deferred cases. The complete inventory is in the evidence record; not every catalog option needs a new active rule.
6. For each changed/new label record reason, input hash/length change if any, expected ids, and complete output. Review against the contract first, then capture actual output. Never blindly accept the binary's current answer.
7. Follow `rust/tests/golden/README.md` for placeholder/hash capture, reconcile all distinct inputs, and verify no orphaned/duplicate/missing labels.
8. Update doctor's synthetic positive to a valid video model with a declared resolution tier and the local conflict pair. Update historical Kling expected ids to conflict only in this same behavior boundary; Task 6 later expands diagnostic coverage.

#### 1.6 Verification and completion

- Run focused resolution tests with one Cargo filter per command: `cargo test --release --bin higgsfield-lint resolution_claim_`.
- Record which new tests fail on execution HEAD and why; do not assert all tests must initially fail.
- Run full binary unit tests, parity with reviewed expectations, doctor integration, and fail-silent checks.
- Run full release suite. Run debug fail-silent if orchestration changed since its last genuine panic verification.
- Reconcile SPEC, source comments, golden counts, exact messages, and doctor expectations before handoff.
- Suggested subject: `fix(checks): ground resolution claims in canonical model controls`. Commit only when the execution workflow authorizes it.

No corpus result, old SPEC arithmetic, or successful historical port can override the newly captured field meaning.

---

### Task 2: Thread tool context and remove the retired model loader

**Files:** `main.rs`, `lint.rs`, `doctor.rs`, `models.rs`, relevant unit/integration tests, SPEC §5.3 and new §5.4, golden corpus.

**Outcome:** Tool scoping is an explicit contract, and the hook no longer walks ancestor directories for a retired file. These changes share the same call sites, so perform one signature migration.

**Public interfaces:**

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolKind {
    Image,
    Video,
    Audio,
    Unknown,
}

pub fn tool_kind(tool_name: Option<&str>) -> ToolKind;
pub fn lint(tool_input: &Value, kind: ToolKind) -> Vec<Finding>;
fn lint_params(params: &Value, kind: ToolKind) -> Vec<Finding>;
```

The semicolon declarations above describe signatures, not code to paste as standalone function definitions.

#### 2.1 Classification contract

- Extract `payload.get("tool_name").and_then(Value::as_str)` in `main::run`.
- Accept only a complete tool component: a bare exact `generate_image`/`generate_video`/`generate_audio` name, its exact `_batch` variant, or that exact final component after `__` in an MCP name.
- Do not use `contains("audio")` or an unanchored suffix such as `ends_with("generate_audio")` that accepts `show_generate_audio`.
- Missing, non-string, empty, and unrecognized names become `Unknown`.
- Names are case-sensitive, matching the configured tool spelling.
- Do not dispatch by model name, prompt contents, or presence of the `duration` field.
- Classify once per payload. Every batch entry inherits the same outer kind.

| Tool context | Resolution | Prompt/negative conflict | Duration after Task 5 |
|---|---|---|---|
| Image / image batch | Run | Run | Skip |
| Video / video batch | Run for verified model controls | Run | Run only for declared duration semantics and eligible modes |
| Audio / audio batch | Skip | Skip | Skip |
| Unknown / missing / non-string | Skip | Run as local consistency | Skip |

Fail-open governs blocking/error behavior; this table governs advisory scope. Unknown resolution behavior is deliberately narrowed by canonical reconciliation. Do not infer tool type from model to restore it.

#### 2.2 Implementation sequence

- [ ] Add a table-driven classifier test covering six supported shapes under both recorded server-name styles, bare exact names, missing/non-string context, and lookalikes.
- [ ] Add audio controls using a supported video model payload that demonstrably produces resolution plus local conflict. Add audio batch and unknown controls. Image positives need an image model; merely changing the tool kind makes a video-model resolution association ineligible.
- [ ] Amend SPEC §5.4 with the exact table before implementing the gates.
- [ ] Replace the unused `&HashSet<String>` argument with `ToolKind` at every call site. Pass the kind through both single and batch paths to `lint_params`.
- [ ] Return immediately for `Audio` before parameter parsing. Preserve debug panic injection's explicit test behavior in `main.rs`.
- [ ] Delete `lint::load_models`, `use crate::models`, obsolete model-set imports/test helpers, and `mod models` in `main.rs`.
- [ ] Delete `rust/src/models.rs` after confirming there are no remaining runtime consumers. The removed file remains recoverable through Git history.
- [ ] Pass `ToolKind::Video` in doctor's existing synthetic self-test.
- [ ] Update doctor comments/messages that explain root discovery in terms of `load_models`. Preserve the root landmark and retired-file absence check.
- [ ] Update SPEC §5.3 to say runtime loading is removed, while the deliberate unused noise filters stay.
- [ ] Add/review golden controls and run the release suite and debug fail-silent test.

#### 2.3 Required controls

| Label family | Input distinction | Expected |
|---|---|---|
| `scope:audio_single` | Video's offending payload under audio | Silence |
| `scope:audio_batch` | Offending entry under audio batch | Silence |
| `scope:image_single` | Same offending text, supported image model/tier | Resolution plus local conflict |
| `scope:video_single` | Supported video model/tier | Resolution plus local conflict |
| `scope:unknown` | Unrecognized complete tool name | Local conflict only; no resolution/duration |
| `scope:missing_name` | No tool_name field | Local conflict only; no resolution/duration |
| `scope:nonstring_name` | tool_name is a number or object | Local conflict only; no resolution/duration |
| Unit lookalikes | `show_generate_audio`, `generate_audio_clone`, `generate_videography` | Unknown |
| Batch regression | Non-empty requests with invalid entries and valid top-level params | No new top-level fallback |

Make all new corpus inputs distinct or map an already-covered behavior to its existing label. Do not add duplicate bytes under new labels.

**Acceptance:** Existing corpus outputs and hashes do not change. Added scope labels match the table. Doctor remains seven PASS. `rg -n 'load_models|crate::models|mod models' rust/src` finds no runtime references. Root discovery still works after the loader module disappears. No privileged filesystem tracing is required.

**Failure mode prevented:** A suite that exercises `lint` directly with a hard-coded video kind can pass even if `main` never reads `tool_name`. At least one audio control must run through the compiled binary.

---

### Task 3: Normalize supported typography without losing quote safety or original spelling

**Files:** `checks.rs`, `parity.rs`, golden entries, SPEC §§5.1–5.2 and shared-text contract.

**Outcome:** Curly/fullwidth double quotes protect quoted content; fullwidth ASCII claims can be recognized; ordinary spacing variations in curated phrases match. No new dependency and no claim of full Unicode normalization.

This task deliberately includes whitespace handling: `slow   motion` was reproduced as a missed conflict during this review. Byte-offset changes are a design constraint to solve, not a reason to leave that measured gap open.

#### 3.1 Exact normalization policy

| Input | Matching representation |
|---|---|
| U+201C, U+201D, U+201E, U+201F | ASCII double quote |
| U+FF01–U+FF5E | Corresponding ASCII character, including fullwidth quote |
| U+00A0 and U+3000 | Ordinary space |
| Runs of ordinary horizontal spaces/tabs after folding | One space for phrase comparison |
| CRLF or CR | LF; retain line boundaries |
| Other Unicode scalars | Preserve unchanged |
| Case in resolution text | Preserve in representation; compare ASCII case-insensitively |
| Case in conflict comparison | Lowercase after transformations, as current behavior requires |

Do not collapse across a line break. Otherwise `slow` on one line and `motion` in an unrelated list item could become a fabricated phrase. Ordinary prose using a newline inside a phrase remains an explicit accepted miss for this release.

Do not delete punctuation or join across separators:

- `(slow motion),` already matches through term boundaries.
- `slow, motion` and `slow / motion` must not become `slow motion`.
- `close - up` remains outside the literal `close-up` phrase.
- Hyphen-space canonicalization is a separate behavior change requiring evidence.
- Do not fold single quotes/apostrophes into double quotes.
- This mapping is neither NFKC nor a generic transliterator. It does not fold ligatures, arbitrary combining sequences, all Unicode spaces, or every quotation style.

#### 3.2 Offset ownership and minimal implementation

Use two small helpers with clearly separated needs:

```rust
// Produces supported folds and horizontal-space normalization.
// Prefer a borrowed fast path when the input needs no change.
fn normalize_for_matching(text: &str) -> std::borrow::Cow<'_, str>;

// After a match is selected in normalized text, replay the same transform
// once to recover the corresponding source-byte interval.
fn original_range_for_match(
    original: &str,
    normalized_range: std::ops::Range<usize>,
) -> Option<std::ops::Range<usize>>;
```

These are design interfaces, not a requirement to introduce an extensible normalization framework. Keep helpers in `checks.rs`.

Rules for source recovery:

1. Every emitted normalized scalar derives from a known original `char_indices` interval.
2. A collapsed space corresponds to the entire original run.
3. Replay the same scalar mapping for the chosen match; return source start at its first contributing scalar and source end at its last.
4. Require normalized offsets to align to emitted scalar boundaries. Return `None` on an impossible range instead of slicing unchecked.
5. Never index the original UTF-8 string using a normalized offset.
6. Never allocate a per-character source map for an entire multi-megabyte prompt. One replay for the selected resolution match is enough.
7. Preserve the original source spelling in findings: `４Ｋ` must render as `４Ｋ`, not `4K`.
8. Select the highest claim first and recover its original text once. Do not normalize/replay separately for every lexicon term.

For the resolution rule, scan one normalized view, compute `quoted_spans` on that same view, choose the highest eligible claim using the existing boundary scanner, and recover its original substring.

For the conflict rule, normalize each field, preserve current both-field negation stripping, lowercase, then compute positive quote spans on the exact transformed string the scanner consumes. If negation removal alters spacing, apply the same horizontal-space policy before scanning. Do not carry quote offsets across lowercasing or replacement operations.

The conflict finding lists canonical curated phrases today, so it does not need original-text recovery. Do not add a source map solely for that rule.

For duration in Task 5, reuse the normalization/quote view but parse numeric lexemes from the matched normalized span. Duration's diagnostic states a numeric value rather than quoting the original phrase, so it also does not require a source map.

#### 3.3 Behavioral tests before implementation

- [ ] Curly-quoted `“close-up”` in the positive field is silent against negative `close-up`.
- [ ] The same phrase without quotes still fires.
- [ ] Curly-quoted `“shot in 4K”` at 720p is silent.
- [ ] Fullwidth quoted `＂４Ｋ＂` is silent, proving folding happens before quote pairing.
- [ ] Unquoted `shot in ４Ｋ` at 720p fires and quotes the original fullwidth bytes.
- [ ] Unquoted fullwidth `ｓｌｏｗ　ｍｏｔｉｏｎ` conflicts with ASCII `slow motion`.
- [ ] `slow   motion`, `slow\tmotion`, and `slow\u{00A0}motion` match.
- [ ] `slow\nmotion`, `slow, motion`, and `close - up` do not match the corresponding multi-token phrase.
- [ ] Surrounding punctuation remains tolerated without changing internal punctuation.
- [ ] Fullwidth `ｎｏ ｈａｎｄｈｅｌｄ` is stripped as a supported negation after folding; its unnegated counterpart fires.
- [ ] A non-ASCII prefix before a fullwidth claim does not corrupt source recovery.
- [ ] Repeated quoted and unquoted occurrences return the eligible occurrence, in original spelling.
- [ ] A final unpaired quote remains unpaired; doubled quotes and inch-mark cases retain positional semantics.
- [ ] Normalization is idempotent for the supported mapping.
- [ ] Matched ranges recovered through multibyte prefixes and collapsed spaces remain valid UTF-8 slices.

Some ASCII controls already pass. Only new bug-reproducing cases must demonstrate a pre-fix failure; passing controls prove retained behavior.

#### 3.4 Implementation and acceptance

- [ ] Write the precise SPEC normalization table and accepted costs.
- [ ] Implement a single scalar-fold definition shared by normalization and source-range replay.
- [ ] Wire normalized matching into resolution and conflict rules; retain one quote-pairing helper.
- [ ] Add golden labels for representative positive, negative, and mixed-occurrence cases.
- [ ] Review every changed existing label, including encoding controls. Fullwidth controls that previously encoded an accepted miss may intentionally change.
- [ ] Run focused Unicode/quote tests, then the release suite.
- [ ] Compare large-input behavior against baseline and investigate any proportional memory increase before accepting the implementation.

Do not promise “zero existing flips”: this is a behavior extension, and existing Unicode cases may change. The requirement is that every flip is explained and no unrelated label changes.

**Accepted costs:** unsupported Unicode equivalences, single-quoted literals, newline-separated phrases, positional quote pairing, and context-sensitive meanings remain outside the matcher. NFKC alone would not supply the custom curly-quote semantics this rule needs.

---

### Task 4: Extend verified carriers without inventing pixel semantics

**Files:** `checks.rs`, model-scoped tests/golden corpus, SPEC §5.1, implementation report.

**Outcome:** Use Task 1's typed representation for quality, mode, and explicit target dimensions. The 2026-09-09 MCP capture has resolved carrier names/options; the remaining uncertainty concerns geometry and precedence, not lack of access.

#### 4.1 Exact associations and boundaries

| MCP model id | Carrier | Implemented comparison |
|---|---|---|
| wan2_6 | quality 720p/1080p | Video p-tier comparison and field-specific advice |
| soul_2, soul_cinematic, soul_v2 | quality 1.5k/2k | Nominal K-tier only |
| kling3_0, kling_video_edit | mode 4k | Recognized4K tier; std/pro have unknown numeric size and stay unmapped |
| topaz_image, topaz_image_generative | output_width/output_height | Explicit requested pixel targets, Image context only |

For the current MCP hook do not substitute the CLI id text2image_soul_v2 for the MCP Soul ids. Do not infer aliases from display names. Capture differences are documented, not automatically rewritten.

Add base resolution associations from the captured catalog, maintaining exact model/output-kind/options and known disallowed combinations. The candidate inventory includes additional360p, 768p, 2160p and nominal 1k/1.5k/2k values. Naked Hailuo512/768/1080 units remain explicitly deferred; no global numeric coercion. Generic quality low/medium/high, Seedance fast, Kling std/pro, and creative modes do not become pixel values.

#### 4.2 Source representation

```rust
enum ResolutionSource {
    Resolution,
    Quality,
    Mode,
    TargetDimensions { width: i64, height: i64 },
}

enum ResolutionValue {
    NominalK { twice_k: u8 }, // 1k=2, 1.5k=3, 2k=4, 4k=8; rank only, not pixels
    VideoPTier(u16),
    RequestedPixels { width: i64, height: i64 },
}
```

This is a design sketch, not a requirement to duplicate dimensions in both fields. Choose one coherent representation with no invalid source/value combinations; a single enum carrying source+typed value is acceptable. Comparators return comparable/unknown, not an invented common short side. Keep lookup constants and pure helpers in checks.rs unless demonstrated complexity warrants another module.

The public interface is `check_resolution_claim(params: &Value, kind: ToolKind) -> Option<Finding>`. Update all callers with ToolKind from Task 2. Low-level string/numeric helpers stay independent of tool context.

#### 4.3 Controlling source and advice

- First establish the model's declared output controls and applicable mode. Never use a global resolution→dimensions→quality→mode priority list.
- A stray undeclared width/height or resolution field cannot override Wan quality, Soul quality, or Kling mode.
- For one valid source, compare the submitted value in its own domain. Omitted fields do not imply the captured default was actually requested.
- Multiple declared size controls with unresolved precedence, such as marketing_studio_video resolution plus width/height: suppress inference rather than guess.
- Seedream stored dimensions and Clipify processing caps are never target-pixel carriers.
- Topaz advice names output_width/output_height, describes requested targets, and does not promise final output.
- No source read can yield advice about an absent or unsupported resolution option. A known fixed ceiling is a separate rule, not an imaginary configurable source.

#### 4.4 Discriminating tests

- Wan 4K/quality 720p fires; 720p/quality 720p stays silent.
- Wan quality 720p plus arbitrary width3840/height2160 still uses quality.
- Soul 4K/quality 2k fires for each verified MCP id, while generic image quality high and CLI-only Soul id stay silent.
- Kling mode 4k against 4K stays silent. Unsupported mode 1080p, std, and pro do not receive guessed pixel values.
- Known carrier used with wrong model or wrong tool kind stays silent.
- Unknown values/types, unknown models, missing model, and Unknown tool stay silent for output-size checks.
- Topaz valid targets produce requested-dimension wording; raw width/height does not act as an alias; malformed numeric tests are paired with valid positives.
- Seedream nominal tier plus stored dimensions uses tier, not stored pixel arithmetic.
- Marketing conflicting declared controls stay silent until precedence is verified.
- Seedance 2.0 fast with 1080p/4k is not described as a confirmed render selection; valid std control proves the supported path.
- Image nominal tiers do not become p-unit geometry through an aspect-ratio change.
- Audio is skipped before visual lint; each new association has process-level single/batch coverage where applicable.

The primary six-tool matcher does not cover dedicated upscale_image/upscale_video tools. Topaz coverage here means calls routed through generate_image, not universal upscaler interception.

---

### Task 5: Add strict video-only duration over-claims

**Files:** `checks.rs`, `lint.rs`, tests/golden corpus, SPEC new §5.5 and shared rule ordering.

**Outcome:** An eligible video prompt that explicitly describes more seconds than its supplied duration receives one arithmetic advisory. Require a known video model with declared duration semantics. Images, audio, unknown tool/model, ignored/source-driven duration modes, under-claims, and unsupported syntax stay silent.

The public id is `duration_claim`. This is the display id already used by the research's diagnostics and is consistent with `resolution_claim`.

#### 5.1 Rule contract

```rust
pub fn check_duration_claim(params: &Value) -> Option<Finding>;
```

Only `lint_params(..., ToolKind::Video)` invokes it. Within the rule require a captured model with declared duration semantics; exclude seedance_2_5/ad_multiplier video_edit (duration ignored) and video_extension (final total ambiguous). kling_video_edit and source-only workflows without declared duration stay silent. Do not put an image exception inside the numeric parser or assume images lack duration.

1. Require `prompt` to be a string.
2. Require `duration` to be a JSON number, finite and in `1.0..=600.0`. Strings, booleans, null, and missing values are silent.
3. Normalize supported typography using Task 3; compute quote exclusions on the exact scanned text.
4. Recognize only the forms below, case-insensitively.
5. Reject invalid/embedded numeric tokens and quote-overlapping matches.
6. For a time span, take its end value; reject reversed spans.
7. Take the maximum valid claimed value across every recognized form.
8. Fire only when that maximum exceeds the supplied duration.
9. Append after resolution and prompt/negative conflict. At most one duration finding per entry.

**Diagnostic:**

```text
duration_claim: prompt describes 15s of action but the supplied duration is 5s. Check the model's supported duration or retime the beats.
```

`Finding.message` contains only the text after `duration_claim:`; rendering adds the id and optional batch prefix in the existing manner.

Use concise stable decimal formatting: integer-valued numbers display without `.0`, non-integers preserve the meaningful decimal value. Do not cast an unchecked floating-point claim to `i64` to format it.

#### 5.2 Supported grammar

Use precompiled bounded regex candidates plus explicit boundary/numeric validation, or an equally small deterministic scanner. Regex is a candidate finder; `\b` alone is not the entire validity rule.

| Form | Example | Claimed value |
|---|---|---|
| Total label with colon | `Total: 15s`, `Total: 15 s`, `Total: 15 seconds` | 15 |
| Seconds phrase | `12-second`, `12 second`, `12 seconds` | 12 |
| Seconds followed by total | `15s total` | 15 |
| Dash span | `0–3s`, `12-15s`, `12—15s` | End value |
| Explicit “to” span | `3s to 7s` | 7 |
| Fractional form using the same grammar | `Total: 5.5s`, `0–5.5s` | 5.5 |

Numeric syntax is unsigned ASCII decimal: one or more digits, optionally followed by a dot and one or more digits. No exponent, sign, thousands separator, or word-number conversion. Require at least one digit on both sides of a decimal point when a decimal is present.

Supporting fractional prompt durations avoids extracting `5s` from the tail of `0.5s`. Accepting fractional supplied durations is a deliberate schema-invalid local tolerance: the fresh video envelope declares integer duration. Label such tests accordingly; ordinary schema-valid end-to-end fixtures use integer settings. Never equate supplied seconds with effective clamped duration.

Do not add optional colon-less `total`, `secs` abbreviations, worded numbers, arbitrary “runtime” phrases, or frame-rate conversions in this release. Existing evidence supports the bounded forms above.

#### 5.3 Boundary and numerical rules

- ASCII alphanumeric characters adjoining a candidate make it embedded: `subtotal: 15s`, `x12-second`, `12-secondhand`, and `15shots` must not be rescued as valid claims.
- A leading sign, dot, comma, exponent fragment, or additional numeric component must not allow scanning to restart at an interior digit.
- Specifically reject partial matches from `-15 seconds`, `+15 seconds`, `1e3 seconds`, `1, 000 seconds`, and `1.2.3 seconds`.
- Reject terminal numeric/unit continuations such as `15s2`. Ordinary sentence punctuation after a complete phrase remains valid.
- A colon attached to a bare seconds candidate can indicate a clock token: `01:15 seconds` must not become `15 seconds`. The explicit `Total:` grammar handles its own colon.
- Use ASCII digit matching, not Unicode `\d` whose captures may fail numeric parsing unexpectedly. Task 3's fullwidth fold supplies the one supported digit equivalence.
- Reject any candidate whose whole range overlaps a paired quote span. Checking only the numeric start is insufficient for a phrase crossing a quote boundary.
- Accept only finite parsed values. A huge digit run that overflows parsing is ignored, not rendered as infinity or an integer saturation value.
- A malformed candidate does not suppress a later independent valid claim. Keep scanning.
- Span start must be nonnegative and at most its end. `15–3s` is not a valid schedule.
- Do not impose the parameter's 600-second cap on all prompt claims: an explicit finite 900-second claim against a 5-second render is still an over-claim. Overflow handling and representability must remain safe.
- Preserve a simple numeric comparison; do not invent a 20% tolerance or an under-claim branch.

An implementation can retain a finite floating value plus its validated display lexeme, trimming insignificant fractional zeros. For extremely long but finite lexemes, bound diagnostic display length and use finite numeric formatting. Test this independently of overflow. Avoid allocating a collection of every match: keep the best candidate so far.

#### 5.4 Required test matrix

Every positive row below names an eligible duration-capable video model/mode (for example kling2_6 with a supplied setting); parser-only numerical boundary rows are explicitly local tolerance tests, not proof that every setting is schema-valid. Add silence rows for unknown/missing model, seedance_2_5 and ad_multiplier video_edit/video_extension, and source-only workflows. Pair each mode exclusion with a valid t2v control.

| Input prompt | Duration/context | Expected |
|---|---|---|
| `Total: 15s / 6 shots / 16:9` | 5, video | 15s finding |
| `A 12-second cinematic sportswear ad` | 5, video | 12s finding |
| `0–3s … 12–15s` | 8, video | 15s finding |
| `3s to 7s` | 5, video | 7s finding |
| `15s total` | 5, video | 15s finding |
| `Total: 5.5s` | 5, video | 5.5s finding |
| `Total: 5s` | 5 or 5.0, video | Silence |
| `Total: 5s` | 5.5, video | Silence |
| `a 5 second pause between lines` | 15, video | Silence |
| `holds for 3 seconds` | 15, video | Silence |
| `holds for 12 seconds` | 5, video | Finding; recorded context ambiguity |
| `shot on a 50mm at T2.0` | 5, video | Silence |
| `15 shots, 24fps` | 5, video | Silence |
| `Total: 15s` | Missing/string/bool/null/0/601 duration | Silence |
| `Total: 15s` | 5, image/audio/unknown | Silence |
| `The narrator says "Total: 15s"` | 5, video | Silence |
| Same with curly/fullwidth double quotes | 5, video | Silence |
| Quoted 15s plus unquoted 12-second claim | 5, video | 12s finding |
| `subtotal: 15s` | 5, video | Silence |
| `0.5 seconds` | 1, video | Silence; never extract 5 |
| Negative/signed/exponent/thousands/malformed decimal cases | 5, video | Silence |
| `15–3s` | 1, video | Silence |
| Huge overflowing digit run | 5, video | Silence for that candidate |
| Invalid candidate followed by `Total: 15s` | 5, video | 15s finding |
| `Total: 900s` | 5, video | Finite 900s finding |
| Several totals, compounds, and spans | 5, video | Maximum eligible value |
| All three checks triggered | Video | IDs in resolution/conflict/duration order |

Add threshold cases at duration 1 and 600, mixed case, hyphen/en-dash/em-dash forms, no prompt, wrong prompt type, and multibyte prefixes. Avoid redundant permutations that merely duplicate implementation branches without protecting a failure mode.

#### 5.5 Process and batch coverage

- [ ] Add failing pure-parser/rule tests for supported positives and false-positive exclusions.
- [ ] Add a real binary video control and the identical params under image, audio, unknown, missing-name, and non-string-name contexts.
- [ ] Add a video batch with one clean entry, one offending entry carrying explicit index 20, and one invalid entry.
- [ ] Assert that only the offender gets `request 20:` and that invalid entries do not shift the explicit index or fall back to top-level params.
- [ ] Add a positional-index case and a two-offender case to prove entry order followed by per-entry check order.
- [ ] Keep captured fixture input bytes unchanged. Assert Task 0's reconciled Kling findings and Seedance silence; neither fixture contains a duration claim, so adding duration alone must not change its result.
- [ ] Assert exact complete message on representative integer and fractional cases.
- [ ] Add deliberate golden cases and record the before/after label ledger.
- [ ] Run focused duration tests, release suite, and debug fail-silent tests if hook orchestration changed since its last debug verification.

**Accepted limitations:** Worded durations, frame counts, inherited/default durations, non-total scene timing, unsupported syntax, and negated duration language are not generally understood. Explicit within-scene over-claims can still fire; quoted content is excluded. Record such examples separately from mandatory silent controls. Never describe the parser as understanding a beat sheet or guaranteeing truncation.

---

### Task 6: Make doctor prove the installation it can actually inspect

Canonical reconciliation now fixes the historical Kling expectation to conflict only. Add a separate model-grounded resolution positive. Doctor must not force restoration of the removed passthrough-dimension assertion.

**Files:** `doctor.rs`, its unit tests, `rust/tests/doctor.rs`, README and SPEC diagnostic section.

**Outcome:** Doctor tests real positive and negative fixtures, covers all configured tool families, and explains what a local diagnostic can and cannot establish.

#### 6.1 Keep seven top-level checks

| Order | Check | What it proves |
|---|---|---|
| 1 | Binary location | A runtime ancestor has the repository landmark |
| 2 | Retired file absent | Unsupported old model-list data has not reappeared |
| 3 | Binary freshness | Required build inputs are present/readable and no newer than the running binary |
| 4 | Settings structure | Checkout settings parse and contain a PreToolUse array |
| 5 | Matcher command | A hook entry resolves to this executable under the selected project directory |
| 6 | Matcher behavior | All six intended tool shapes match; designated lookalikes do not |
| 7 | Self-test | Synthetic and committed fixtures produce the intended finding ids/order |

Keep the current rule that FAIL yields exit 1 and WARN-only yields exit 0. Healthy checkout tests require seven PASS with no WARN/FAIL. The scope-disagreement test still expects a project-directory warning and exactly two matcher failures.

Seven PASS is not sacred if a later independently approved feature needs another check; this core change does not.

#### 6.2 Ground the self-test in actual fixtures

Change the self-test to receive the discovered root, e.g. `check_self_test(repo_root: Option<&Path>) -> Check`, and fold all of its cases into one report item.

Cases:

1. Synthetic two-finding payload revised in Task 1 to name a supported video model/resolution tier plus the local conflict pair: confirms dispatch/order.
2. `tests/fixtures/kling2_6_4k_slowmo.json`: parse its existing `{"params": ...}` tool-input shape, run as video, expect exactly prompt_negative_conflict and no resolution finding.
3. `tests/fixtures/seedance_tree_clean.json`: run as video, expect no findings.
4. Small synthetic duration-only payload naming a duration-capable video model and eligible mode: expect only `duration_claim`.
5. Same duration payload as image and audio: no duration finding.
6. A compact supported-typography control: curly-quoted claim stays silent.

Keep separate expected ids for each case. The revised synthetic positive still expects resolution/conflict; the unchanged historical Kling input now expects conflict only. A duration-only case must not change unrelated expectations.

Fixture file failures must be diagnostic failures, not silently replaced by embedded fixture copies:

- Missing file → FAIL naming its relative path.
- Invalid JSON / wrong shape → FAIL naming the fixture and reason.
- Wrong findings/order → FAIL with expected and actual ids.
- Root discovery unavailable → self-test explicitly reports inability to validate fixtures, while the independent synthetic checks may still run.

Read fixtures only in doctor. The hook must not inherit these filesystem reads. Do not modify real fixture files to test the failure branch; use the existing `TestRepo` temporary-directory pattern and explicit path parameters.

#### 6.3 Freshness and coverage

The current freshness scan covers Rust sources and Cargo.toml. Include `Cargo.lock` because a dependency resolution change can change the binary with no source edit.

- Required build inputs missing or unreadable must not become a false PASS.
- Enumerate runtime source files, including any future runtime source subdirectory if introduced.
- Fixtures read dynamically by doctor do not need rebuilding; fixture correctness is covered by self-test.
- Metadata-only docs and golden expectation edits do not make the runtime binary stale unless they become build inputs.
- Keep mtime checking lightweight. State its limits: timestamps can be preserved or manipulated; a newer timestamp is not a cryptographic proof of source identity.
- Do not add build scripts, content hashes embedded into the executable, or auto-rebuild logic without evidence that the mtime limitation is causing real failures.

Test changed Cargo.lock, missing required input, source-newer-than-binary, and healthy inputs using a temporary fixture. Preserve the existing real-environment tests for `CLAUDE_PROJECT_DIR`.

#### 6.4 Matcher cases

Change the single `SHOULD_MATCH` example to a slice including image/video/audio and their batch forms. Cover both the historical UUID-style server id and the recorded `mcp__claude_ai_Higgsfield__...` name.

Reject at least:

- `generate_3d`.
- `show_generations`.
- `generate_audio_clone`.
- `show_generate_audio`.
- A supported tool name with trailing extra characters.

Keep the repository's existing matcher unless a concrete test demonstrates the need for a separate change. The wildcard server matcher also matches other MCP servers with these tool shapes; record this scope limitation instead of asserting it uniquely identifies Higgsfield.

#### 6.5 Diagnostic caveat and acceptance

Print one informational line after doctor's check list:

```text
doctor checks this checkout and binary; confirm the hook is loaded with /hooks and verify a hook trace in the target Claude Code session.
```

This line is neither PASS nor WARN and never appears on the hook path.

- [ ] Healthy release doctor: seven PASS, exit 0, empty stderr.
- [ ] Missing/corrupt/changed fixture in temporary repo: one self-test FAIL naming the case.
- [ ] Wrong tool-kind wiring breaks audio/image self-tests.
- [ ] Stale source or Cargo.lock fails freshness.
- [ ] Missing-root paths remain diagnosable without panic.
- [ ] Different CLAUDE_PROJECT_DIR preserves the existing two matcher failures.
- [ ] Unknown command-line argument still uses the hook path.
- [ ] Direct generation-shaped input never prints doctor text.

Local self-test proves the running logic and checkout conditions. It does not prove the assistant will act on a finding, the service will honor parameters, or the target session loaded the hook.

---

### Task 7: Make corpus coverage and performance claims auditable

**Files:** `parity.rs`, `checks.rs` test support, golden README, implementation report; golden metadata only when justified.

**Outcome:** Semantic regression counts have defined membership; performance evidence can detect scaling regressions without pretending wall-clock measurements are deterministic.

#### 7.1 Separate four kinds of evidence

| Evidence | Question answered | Assertion/reporting policy |
|---|---|---|
| Golden parity | Did reviewed behavior or generated input bytes change? | Assert all labels, hashes, stdout/JSON, status, stderr |
| Semantic controls | Did supported positives fire and required negatives stay silent? | Assert explicit expected ids and silence |
| Determinism | Does the same input produce the same bytes repeatedly? | Assert stdout/stderr/status equality on selected controls |
| Performance measurements | How fast and memory-efficient is the current release here? | Report measured distributions and conditions; use deterministic operation bounds for the core algorithm guard |

Do not call the proportion of non-empty golden outputs “recall.” Some cases intentionally stay silent, some encode accepted false positives, and some belong to rules that have not shipped.

#### 7.2 Explicit research-to-corpus coverage

Map all 65 fixture identifiers in research §12 to one of:

- Covered by an exact existing or newly added corpus label.
- Covered by a named unit test where the case is a pure helper property, with an explanation of why process coverage is unnecessary.
- Deferred with a specific unimplemented rule.
- Accepted limitation with expected current behavior.

Put the mapping in the golden README rather than recreating every fixture. Include sets A–F and every member, even if several map to an equivalent existing input. A new label must not duplicate the exact bytes of another.

For semantic metrics, keep explicit groups or metadata in the test source with:

- Label.
- Rule id or applicable rule set.
- Expected finding ids (or required silence).
- Class: supported positive, mandatory negative, robustness-only, accepted limitation, deferred.
- Evidence/fixture reference.

Validate that group references point to existing labels and do not silently disappear when generators change. A label can document several research examples, but metrics count a distinct input once within each reported denominator.

Accepted false positives are included in a separate “known limitations” row with both actual and ideal behavior; they must not be relabeled as successes. Deferred rules do not inflate current recall.

#### 7.3 Metrics contract

After the normal parity comparison, print a compact report under `--nocapture`:

- Generated distinct inputs / recorded entries / missing / orphaned / changed-input cases.
- Mandatory negative controls: total and unexpected firings; required target zero.
- Supported positives per active rule: total and correct detections; explicit numerator and denominator.
- Accepted limitations: total, expected findings, and observed findings.
- Mapped research cases: covered / deferred / accepted, summing to 65.
- Selected repeated-input determinism cases: count and any output mismatch.
- Optional benchmark output in a separate block/run.

Keep semantic expectations as assertions. Printing zero false-positive controls without asserting the contract provides weaker protection than the existing golden corpus.

Do not infer semantic truth from the golden output itself: that would make a mistaken golden flip redefine a false positive as correct.

#### 7.4 Deterministic complexity regression guard

The current quote-scaling test compares 8, 000 with 64, 000 spans and asserts a timing ratio below 16. Its comments incorrectly imply that the complete check should barely grow as text grows.

A scan of eight times as much input should grow with input size; with binary-search exclusions its relevant bound includes `O(matches × log spans)`. Merely changing the large input to 128, 000 while keeping a strict 16× threshold can reject a correct implementation: input alone grew 16× before logarithmic overhead.

Adopt a deterministic guard:

- Retain the production binary search over sorted, disjoint spans.
- Factor the membership search so a test-only observer can count the same comparisons used by production logic, without global counters or production allocation.
- Count comparisons for probes at beginnings, middles, ends, gaps, and outside the span set at small and large sizes.
- Assert a logarithmic bound per lookup, e.g. at most `ceil(log2(S + 1)) + 2` comparisons for `S` spans, with the precise bound matched to the chosen implementation.
- Ensure instrumentation measures the actual lookup comparisons, not a disconnected test implementation.
- Include a large repeated-quoted-term functional test to prove the full rule stays silent and a final unquoted occurrence still fires.
- Keep any wall-clock stress threshold very generous and outside the main cross-machine correctness gate if it proves flaky.

A small local binary-search helper with a test observer is acceptable if the standard-library call cannot expose comparison counts cleanly. Do not add a general profiling abstraction.

Demonstrate discrimination once during implementation by substituting a linear scan in an isolated temporary copy and observing the operation-count guard fail. Do not mutate the user's working sources for this experiment or add a permanent second implementation.

#### 7.5 Performance measurement protocol

Record performance after the final behavior changes, using the release binary:

- Include clean small input, each active finding, all three findings, representative batch input, long quoted prompts, supported Unicode, and malformed/large inputs.
- Warm up before samples; state whether each measurement includes process launch, stdin/stdout, and JSON parsing.
- For ordinary-call latency, collect enough repeated samples to make a percentile meaningful, e.g. at least 100 per representative case. Report median, p95, p99, maximum, sample count, platform, and build profile.
- For multi-megabyte stress inputs, a smaller explicitly stated sample count is acceptable; report maximum/median rather than presenting an unstable p99.
- Compare baseline and revised builds on the same machine and input set if a baseline binary is available.
- Investigate material changes; do not invent a product latency SLO from a prior “about 2 ms” narrative.
- Check peak memory for the normalization path with platform tooling when available, particularly the previously problematic repeated-character-array input.
- Keep timing metrics out of hook stdout and out of golden expectations.
- Stable output bytes and semantic counts must agree across runs; elapsed times are expected to differ.

Use test-local or temporary benchmark orchestration. Do not add runtime telemetry, a service, or a scheduled benchmark job.

#### 7.6 Golden discipline

For each behavior task:

1. Decide the expected behavior from SPEC and the test case before reading “actual output.”
2. Add discriminating negative/positive tests and record the relevant red failure.
3. Change production behavior.
4. Run parity and classify every failure as output change, input-byte recapture, new label, removed label, or unintended regression.
5. Edit each golden row deliberately. Input hashes change only when the generated stdin changes.
6. Record before/after ids or message text and the specific reason for each changed label.
7. Re-run parity; reject unexpected missing/orphaned entries and duplicate-byte labels.
8. Verify the semantic groups still point to the cases they claim to measure.

There is still no bulk regeneration command. Read-only tooling that lists mismatches or validates the ledger is fine; automatically replacing expectations with whatever the binary emitted is not.

Do not hard-code 353 or any other future final count. Task 1 alone adds mapped values and false-positive controls; subsequent tasks add further cases. Report the actual final distinct-input count and reconcile every addition/removal.

For provenance, distinguish the original capture commit from later deliberate behavior revisions. Do not relabel an entire mixed-history corpus as captured from a commit that predates some entries, and do not invent the hash of an uncommitted future change.

---

### Task 8: Reconcile documentation and hand off a verifiable release

**Files:** SPEC, README, golden README/metadata, credit-expiry task, superseded plan banner, source comments, implementation report.

**Outcome:** The repository explains one product, accurately states its limits, and provides a repeatable build/verify/deploy procedure.

#### 8.1 SPEC changes by section

| Section | Required change |
|---|---|
| §2 | Distinguish the two historically observed checks from the newly added duration rule |
| §3 | State project-local installation and broad server-name matching; preserve advisory output |
| §5.1 | Task 1 typed domains/quotes and removal of false output assertions; Task 3 folds; Task 4 verified sources/precedence/limits |
| §5.2 | Exact normalization order/table; retain curated lexicon and both-field negation semantics |
| §5.3 | Rule disabled; deliberate noise filters retained; runtime model loader removed |
| §5.4 | ToolKind scope table, including Unknown |
| §5.5 | Duration grammar, supplied-vs-effective distinction, model/mode eligibility, integer schema vs fractional tolerance, quote/numeric controls |
| §6 | Exact finding order and actual message text; retain historical example only with its reconciled expected findings |
| §7 / §7.1 | Failure guarantees and limits; seven doctor checks and diagnostic-only reporting |
| §8 | Historical 30-day reset evidence, fresh-anchor requirement, stale-data condition, reminder status |
| §9 | Single-call vs batch schema claims; golden policy; actual counts and semantic coverage |
| §10 | Current module layout, doctor, removed models module, data landmark, execution report |
| §11 | Continue excluding docstring duplication, aesthetic judgments, proxy/gate machinery |
| §12 | Build/rebuild requirement, mtime limitations, archived oracle distinction |
| Evidence pointer | Research sections and evidence classes without reproducing the full research document |

For a new behavior, edit its SPEC contract as part of that behavior's work package. Task 8 consolidates examples and cross-references; it is not permission to leave the spec stale through all earlier changes.

#### 8.2 README install and verify flow

Write a short operational sequence:

1. Build: `cargo build --release`.
2. Run `./target/release/higgsfield-lint doctor` from the checkout.
3. Inspect the target Claude Code session's loaded hook configuration with `/hooks`.
4. Verify a real hook trace if the intended connector/session is available.

State that current `.claude/settings.json` covers six matched MCP generation tools in this Claude Code project. It does not intercept Higgsfield CLI shell commands, direct HTTP calls, dedicated upscalers/workflows, other clients/projects, or other computers. CLI OAuth/schema access does not install or prove lint interception.

A cost-only connector preflight can be a useful trace **only when** its current schema/documentation confirms it submits no generation and the session is already within the user's authorized workflow. Do not infer that from an old payload, submit a generation as a test, or call a cost endpoint that bypasses the configured generation matcher. Direct stdin probes are adequate for local binary validation; mark loaded-session verification unperformed when unavailable.

Do not claim doctor is the only possible way to inspect stale/missing binaries. It is the supported local diagnostic command, and a missing binary cannot diagnose itself.

Rebuild after changes to runtime Rust sources, Cargo.toml, or Cargo.lock. Run doctor again. Hook silence alone cannot establish that the intended build ran.

A local opt-in post-merge/post-checkout rebuild hook can reduce stale-binary mistakes but is not installed by this plan. It must preserve existing user Git hooks and is a separate workflow choice.

#### 8.3 Credit-expiry documentation

Preserve the historical measurements and correct their time framing:

- The six observed resets were 30 days apart.
- `2026-08-05 → 2026-09-04` is a historical example, not a current upcoming renewal.
- At each future reminder run, retrieve the latest actual Reset row and current balance.
- If the latest reset is more than 31 days old, explicitly flag stale evidence instead of projecting a plausible-looking date.
- Do not infer that a scheduled job currently exists or is absent from this repository's documents alone. The repository specifies one; actual account scheduling state must be verified separately if requested.

Resolve the boundary between a weekly schedule and a “fewer than seven days” rule. Checking exactly seven days before reset can miss a strict `< 7` window until reset day.

Recommended wording for the proposed weekly reminder: warn when remaining credits exceed 400 and the next reset is within seven days **inclusive**, using exact timestamps and the configured local timezone. Update SPEC §8 and `docs/credit-expiry-task.md` together so the boundary is explicit. If strict `< 7` is retained for a separate operational reason, a weekly cadence must not be described as guaranteed coverage.

Creating or changing the recurring task is outside this implementation release. The plan supplies a coherent future task specification.

#### 8.4 Schema accuracy

Use Task 0's completed canonical-contract matrix; this is final documentation reconciliation, not the first schema verification. Single-call and batch parameter shapes must each have current captured evidence or an explicit unresolved status. An unresolved schema gate prevents claiming complete canonical alignment.

Keep distinct:

- The outer request-entry object's `additionalProperties` setting.
- The type of its `params` field.
- The required integer index.
- The runtime's deliberately permissive handling of JSON-string params and unusual indices.

Do not attach `additionalProperties: false` to the inner params object unless the inspected schema actually does. Schema-invalid tolerance remains useful robustness coverage, even when it is not an ordinary user-flow test.

#### 8.5 Historical comments and reports

- Change present-tense “Port of” guidance to historical explanations where appropriate.
- Update comments that say the caller “must port” behavior already implemented.
- Preserve explanations of accepted compatibility quirks that still apply.
- Add a small “superseded” banner to the project-intent-profile plan explaining that it targets the retired Python architecture and is not part of this release.
- Do not rewrite archived source, research evidence, or completed historical plans.
- If an old task's re-review verdict is missing, record “not found” with the inspected location; do not invent a review outcome.
- Keep the current implementation report authoritative for this work. Do not require hidden or gitignored ledgers to establish completion.

#### 8.6 Final verification and delivery

- [ ] All behavior work packages satisfy their stated contracts and have a per-label golden ledger.
- [ ] Run `cargo test --release` after the final runtime/test edits.
- [ ] Run `cargo test --test fail_silent` after final dispatch/panic-related edits; retain the recorded result if later changes are docs-only.
- [ ] Build the final release binary explicitly and run release doctor.
- [ ] Confirm no new dependencies and release unwind retained.
- [ ] Confirm hook stdout contains only the expected structured JSON or nothing; no diagnostic banner leaks.
- [ ] Compare SPEC's example with the actual parsed output and exact additionalContext string. Pretty-printed JSON whitespace need not be byte-identical.
- [ ] Check docs links, section numbers, current file layout, historical date labels, and actual corpus counts.
- [ ] Run `git diff --check` on tracked changes and explicitly inspect new/untracked artifacts.
- [ ] Report loaded-session verification separately from local process verification.
- [ ] Hand off the changed files, test results, remaining accepted limitations, and any deferred operational action.

No unconditional `git push`, global-settings edit, generation job, or recurring automation is part of this release. Commit boundaries are suggested below; execute Git publication only under the actual user request.

## 4. Reviewable change series

Each row is one cohesive change boundary. Combine a purely mechanical prerequisite with its dependent behavior if separating them would leave broken interfaces.

| Order | Suggested subject | Included artifacts |
|---|---|---|
| 0 | `docs: reconcile canonical Higgsfield contracts and migration status` | Task 0 schema/catalog matrix, explicit evidence gaps, migration-completion record, amended rule contracts |
| 1 | `fix(checks): complete resolution rule re-grounding` | Task 1 code, controls, golden deltas, relevant counts |
| 2 | `refactor(lint): scope tools and remove retired model loading` | Task 2 context API, audio behavior, deleted loader, docs/tests |
| 3 | `fix(checks): normalize supported typography safely` | Task 3 transforms, quote safety, original spelling, tests/spec |
| 4 | `feat(checks): recognize verified resolution carriers` | Task 4 evidence, associations, source-aware advice, tests/spec |
| 5 | `feat(checks): detect video duration over-claims` | Task 5 rule, scope, grammar, numeric defenses, tests/spec |
| 6 | `feat(doctor): verify fixtures and build inputs` | Task 6 diagnostic coverage and install guidance |
| 7 | `test: make coverage and complexity checks explicit` | Task 7 semantic mapping, deterministic guard, measured report |
| 8 | `docs: reconcile rule contracts and operating guidance` | Task 8 final examples, metadata, dates, report |

Stage only the intended files if committing. The original planning inputs and unrelated user work must not be swept into a broad `git add .`.

## 5. Acceptance gates and recovery

### 5.1 Release gates

**Gate A — Correctness and noise**

- Official-style 8K headers and matched quoted claims are silent.
- Supported resolution contradictions fire using the right source and advice.
- Curated phrase normalization fixes supported forms without joining punctuation or line boundaries.
- Audio produces no text findings; image and unknown contexts produce no duration findings.
- Eligible video supplied-duration over-claims fire; ignored/source-driven modes, unknown models, under-claims, quoted dialogue, malformed numbers, and unsupported units are silent.
- Historical Kling positive and Seedance negative fixtures remain discriminating.
- Every known limitation is named separately from required negatives.

**Gate B — Reliability**

- Release tests and real debug panic test pass.
- Golden labels/hashes reconcile; zero unexplained flips, duplicates, missing, or orphaned cases.
- Deterministic semantic groups cover every claimed behavior.
- No repeated full-text source map or quadratic quote lookup has been introduced.
- Hook path remains zero-exit and empty-stderr for tested failure inputs.
- Doctor reports seven PASS on a healthy checkout and fails meaningful damaged-checkout cases.

**Gate C — Operational truth**

- Rebuilt release binary and configured executable agree.
- README accurately states project scope and wildcard server matching.
- Loaded-session proof is either recorded or explicitly unverified.
- Current service schema claims have dated sources.
- Task 0's envelope/model/guidance matrix is complete; unverified passthrough fields are not described as proven render controls.
- No current balance/reset assertion is fabricated from historical account evidence.
- The final implementation report identifies the version, completed tasks, tests, and limitations.

If carrier evidence is unavailable, report the carrier task as pending rather than marking the whole core complete. The already-verified resolution fix can be delivered as an explicitly narrower incremental release.

### 5.2 Recovery strategy

Recovery must preserve the reason for the original false-positive fixes:

- Keep each behavior change separate enough to revert its runtime/spec/golden unit coherently.
- For a new normalization or duration regression, first add a focused counterexample and narrow that change.
- If an urgent rollback is required, revert the specific faulty behavior package, rebuild, and run doctor; retain unrelated user work.
- Do not restore the obsolete 8K lexicon merely to roll back a later duration feature.
- Reverting source without rebuilding leaves the installed hook unchanged. Include binary rebuild in every rollback instruction.
- If a hook installation itself is broken, use the client's documented hook controls to disable the scoped entry while investigating. Do not replace the whole settings file.
- No automatic revert, destructive reset, or global configuration edit follows from this plan.

## 6. Concrete successor work and reasons for deferral

The following ideas are preserved because they may improve the product. They have explicit entry criteria so future work does not restart from a vague “consider later.”

### 6.1 HFR-002: model resolution ceilings

**Value:** Detect `kling2_6` plus a 4K claim when no resolution or dimensions are supplied. The current core intentionally remains silent without comparable supplied parameters.

**Entry criteria:** Current authoritative ceiling evidence; an owner/refresh mechanism; an explicit stale-data policy; and agreement to add a runtime data file.

**Required design:**

- Versioned snapshot with captured-at timestamp, exact model ids, source references, comparison units, and per-model evidence.
- Unknown models, invalid entries, unreadable files, and unsupported versions are silent.
- Do not infer ceilings from option labels whose pixel semantics are unclear.
- A stale maximum can produce false positives if a model improves. Drift does not inherently “degrade to silence.”
- Define a maximum evidence age in the spec before shipping; on expired/unverifiable evidence, suppress ceiling findings rather than merely warning in doctor.
- Time evaluation is an explicit new dependency: pass an evaluation instant into a pure loader/validator seam and test it with fixed time.
- Missing or invalid clock information cannot enable an uncertain ceiling.
- Reuse the re-grounded claim lexicon and quote exclusion.
- Emit at most one resolution finding per entry. A direct parameter contradiction takes priority over a ceiling fallback.
- Keep the retired negative-prompt-file guard and add a separate snapshot diagnostic; do not repoint the guard and accidentally permit retired data.
- Test exact ceiling equality, above ceiling, unknown model, stale timestamp, future timestamp, malformed snapshot, duplicate ids, and changed model capability.
- Define refresh as a read-only evidence capture followed by a reviewed repository change, never a hook-time network call.

No ceiling numbers from the dated research become new current promises automatically.

### 6.2 HFR-003: aspect-ratio claims

**Value:** Detect an unambiguous explicit prompt ratio that disagrees with a supplied ratio/dimensions.

**Entry criteria:** A complete parser contract and controls for the published dual-ratio prompt, clock syntax, decimal ratios, three-part tokens, and scene-content ratios.

**Required design:**

- Known image/video tools only; no word-to-ratio inference.
- Positive, nonzero ratio components with safe reduction/comparison.
- Explicit valid aspect_ratio takes priority; `auto` and other unsupported strings stay silent according to the chosen contract.
- State exactly when dimensions are a fallback; use positive dimensions and a defined sanity floor.
- Exclude quoted occurrences using the shared helpers.
- Detect unsupported ratio-shaped tokens before counting “exactly one” supported ratio.
- The published `2.35:1` plus `16:9` prompt must stay silent. A parser that ignores the decimal token then sees only 16:9 does not satisfy this condition.
- Reject partial matches from `2.35:1` and `60:30:10`; a bare word-boundary regex is insufficient.
- Distinguish clock tokens and explicitly label any context-window heuristic as engineering judgment.
- `4:3 television` remains a potential content false positive even though it parses perfectly. Include it as an accepted limitation or narrow the rule before release.
- Cover equivalent reduced ratios, duplicates, multiple distinct ratios, zero components, overflow, and malformed parameters.
- Append a new public id/order only after resolving naming with the existing contract.

The successor can reuse Task 3's text handling, but it does not require the model-ceiling snapshot.

### 6.3 HFR-006: negation density and retained filters

**Value:** Potentially advise on exclusion-heavy prompts without asserting that exclusions belong in another field.

**Entry criteria:** A representative consented prompt corpus, documented thresholds calibrated against official long prompts, and a measured precision benefit.

Keep `PLACEMENT_NEGATION_RE`, `PLACEMENT_STOPWORDS`, and `is_placement_noise` with their existing tests in this core release, because SPEC §5.3 explicitly retains them. They do not run on the hook path or load a model file.

If the density rule is formally abandoned or the filters become misleading, a later small cleanup may remove them along with their retention statement and obsolete tests. “Can recover from Git” is a valid maintenance option, not proof that immediate deletion satisfies the current spec.

Do not ship arbitrary `12 exclusions` or `8%` thresholds as first-party facts.

### 6.4 Negated resolution claims

**Current limitation:** `not 4k` can produce a finding. Keep an explicit limitation case with ideal silence and actual expected finding, so it is visible in metrics.

**Why not call `strip_negations` immediately:** Its current pattern consumes alphabetic/hyphenated words after negators. It does not consume numeric `4k` or `1080p`, and extending it globally could change the conflict rule.

**Successor contract:** Narrow literal negation spans for resolution claims, punctuation boundaries, preservation of a later affirmative occurrence, and original-offset/quote interactions. Require real examples or a separately accepted mechanical policy. A contradictory prompt containing both “not 4K” and a later affirmative 4K claim must not have every occurrence suppressed.

The same caution applies to negated durations: broad language intent is not resolved by the arithmetic parser.

### 6.5 HFR-004, HFR-005, and HFR-007

These catalog incompatibility checks remain deferred. They need stable first-party contracts and demonstrated value beyond repeating tool descriptions.

HFR-007 has the clearest connection to duration claims because clamping a supplied duration changes beat timing. If added, distinguish requested duration from effective duration and avoid issuing conflicting advice from two checks. Task 5 now explicitly uses minimal model/mode eligibility to avoid ignored-duration false positives; effective-duration calculation, full range validation and a generic capability engine remain deferred.

### 6.6 Optional operational extensions

- **CLI/other-tool adapter:** Official coding-agent guidance recommends CLI, which bypasses the existing MCP matcher. A separately requested adapter must consume structured requests, reuse pure lint logic, test quoting/file inputs and bypass boundaries, preserve fail-open behavior, and never pretend shell-command regexes are reliable request parsing. No adapter is implemented in this release.
- **User-global installation:** Requires the desired deployment scope, an absolute executable path, preservation of existing settings, duplicate-hook detection, and a doctor scope report. Verify from a different project. Not part of the core default.
- **Automatic rebuild hooks:** Opt-in only; preserve existing hooks, report failures, and avoid recursive invocation.
- **Recurring credit reminder:** Use fresh account evidence and the inclusive warning boundary; no scheduled task is created by this plan.
- **Project-intent profile:** A different product expansion that needs its own goals and evaluation.
- **Full NFKC/new crate:** Revisit only for unsupported equivalences demonstrated in real input; retain custom quote policy and source-offset safety.
- **Heartbeat, blocking gate, runtime network fetches, aesthetic scoring:** Remain rejected under the current product goals.

## 7. Executor checklist

- [x] Baseline and current HEAD recorded.
- [x] Full research read; canonical-contract matrix completed before dependent behavior changes.
- [x] Python-to-Rust migration recorded as already complete; no live Python oracle or fallback restored.
- [x] Task 1 implements the canonically amended SPEC, not the superseded universal short-side map.
- [x] Tool context extracted in the real process path; old loader removed.
- [x] Normalization fixes supported typography and spacing while preserving original claims.
- [x] Carrier support backed by current evidence and exact source-aware advice.
- [x] Duration parser is video-only, model/mode-eligible, supplied-value-only, over-claim-only, quote-aware, and robust against partial numbers.
- [x] Doctor proves positive/negative fixtures and freshness inputs without reading them on the hook path.
- [x] Golden changes reviewed label by label.
- [x] Research coverage mapped explicitly; known limitations/deferred rules not hidden in accuracy metrics.
- [x] Complexity protection measures production comparisons; timing reported honestly.
- [x] SPEC, README, schema statements, historical dates, and layout reconciled.
- [x] Final build/tests/doctor evidence captured.
- [x] Loaded-session verification reported at its actual evidence level.
- [x] Runtime changes delivered only when implementation was requested; no implied global deployment or publication.
