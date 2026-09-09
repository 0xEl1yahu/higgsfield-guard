# higgsfield-lint — Codebase Orientation

Written 2026-09-09 against commit `a7e0039` on `feat/doctor-and-expiry`. A standalone
map of what this project is, how it is built, and where the work stands. It cites the
authoritative documents rather than replacing them: `SPEC.md` is the design,
`HIGGSFIELD_PROMPT_RESEARCH.md` is the evidence base, and
`docs/superpowers/plans/2026-08-31-rule-regrounding.md` is the live plan.

Note: there is no `AGENTS.md` or `CLAUDE.md` in this repository. The only agent-facing
configuration is `.claude/settings.json`, which wires the hook. Operating rules live in
`SPEC.md` and the plans.

---

## 1. What it is

A Claude Code `PreToolUse` hook, shipped as a single Rust binary, that reads Higgsfield
`generate_image` / `generate_video` / `generate_audio` MCP calls (and their `_batch`
forms) before they run and reports when the prompt argues with its own parameters. The
finding goes back to the model as `hookSpecificOutput.additionalContext`.

Hook matcher (`.claude/settings.json`):

```
mcp__.*__generate_(image|video|audio)(_batch)?$
```

Command: `"$CLAUDE_PROJECT_DIR/target/release/higgsfield-lint"`.

### The contract (SPEC.md §4, §6, §7)

- **Advisory only.** Never returns `permissionDecision: "deny"`.
- **Never exits non-zero, never writes to stderr** on the hook path. Exit 2 plus
  stderr would block the call.
- **Fail silent.** Malformed stdin, missing fields, invalid UTF-8, or an internal panic
  all exit 0 with empty stdout. A silent panic hook plus `catch_unwind` enforce the
  panic case; `Cargo.toml` pins `panic = "unwind"` so this stays catchable.
- **No network, no MCP, no filesystem writes.**
- **Output shape:** one JSON object with `additionalContext` containing
  `higgsfield-lint: N finding(s)` followed by one `- <check_id>: <observed>. <fix>.`
  line per finding. No findings means no output at all.

### Why it is small (SPEC.md §1, §13)

A 990-line "Preflight Gate" MCP proxy (archived at
`docs/archive/2026-08-30-preflight-gate-v1-rejected.md`) was cut after measurement:

| Fact | Value |
|---|---|
| Credits per 30-day cycle | 1,200, unused balance wiped at renewal |
| Forfeited to expiry, Mar–Aug 2026 | ~6,910 |
| Spent generating, all history | ~732 |
| Failed generations | ~11%, every one refunded in full |
| Lifetime video calls | 9 |

Blocking had nothing to protect, so the whole verdict/promotion/holdout apparatus was
dropped. §13 lists the conditions that would bring it back.

---

## 2. The checks (SPEC.md §5)

Two live checks run in this fixed order (order is part of the output contract):

### 2.1 `resolution_claim`

Fires when the prompt claims a resolution the call cannot produce.

- **Claimed** = highest-pixel lexicon term found in the prompt (case-insensitive, token
  boundaries, quoted in the message with the user's original casing).
- **Actual** = `params.resolution` if it is a recognised value, else `min(width, height)`
  when both are JSON integers and `max(width, height) >= 128` (the floor is keyed on the
  larger side so `16x9` aspect expressions are ignored but `1920x100` still fires).
- Silent when either side is absent.

**Spec/code divergence right now.** SPEC.md §5.1 was re-grounded in commit `a7e0039`
(task 4, step 1) but `rust/src/checks.rs` has not caught up:

| | SPEC.md §5.1 (current) | `checks.rs` (current) |
|---|---|---|
| Claim lexicon | `4k`, `1080p`, `720p`, `480p` | still has `8k`, `uhd`, `full hd` too |
| `RESOLUTION_PARAM` | adds `2160p`, `2k`, `1.5k`, `1k`, `768`, `512` | only `4k`, `1080p`, `720p`, `480p` |
| Quoted spans in prompt | excluded | not excluded |
| Message when actual came from width/height | "Increase the output size, or drop the claim from the prompt." | always "Raise resolution, …" |

### 2.2 `prompt_negative_conflict`

Fires when a curated directive phrase appears, unnegated, in both `prompt` and
`negative_prompt`. Recalibrated in task 3 after confirmed false positives (`pan`,
`text`, `sharp`, later `letterbox`, `watermark`):

1. Negations (`no|without|avoid|not` + up to three words) are stripped from **both**
   fields, so `not handheld` in `negative_prompt` is agreement, not conflict.
2. Occurrences inside matched double quotes in the prompt are excluded (a quoted literal
   is content, not direction).
3. Every lexicon entry is either a bigram or a single token of four or more characters
   judged unambiguous. No stemming; `blur` vs `blurry` is an accepted miss.

Current list: `slow motion`, `slow-mo`, `motion blur`, `close-up`, `wide shot`,
`handheld`, `blurred`, `blurry`, `camera pan`, `whip pan`, `slow zoom`, `on-screen text`,
`film grain`, `static shot`, `lens flare`, `letterbox bars`, `watermark overlay`,
`subtitles`.

Deferred: the research's NFKC/whitespace/punctuation normalisation step. The check only
lowercases.

### 2.3 `negation_placement` — disabled (SPEC.md §5.3)

Fired when the positive prompt contained `no X` / `without X` / `avoid X` and the model
was assumed to honour `negative_prompt`. Research §6 showed Higgsfield's own guidance
says to keep exclusions inline, and no model in the ~63-entry catalog declares a
`negative_prompt` parameter. Disabled with five permanent negative controls. Its regex,
stoplist, and noise filter are kept unused in `checks.rs` for HFR-006, a future
volume-based rule whose thresholds (`count >= 12` and `count / words >= 0.08`) are
explicitly uncalibrated. `data/negative_prompt_models.json` was retired with it.

---

## 3. Architecture

```
Cargo.toml                    bin + three [[test]] targets; panic = "unwind"
rust/src/main.rs              `doctor` dispatch before stdin; panic guard; hook path
rust/src/lint.rs              params resolution, batch path, render()
rust/src/checks.rs            lexicons, boundary scanner, quoted_spans, the two checks
rust/src/doctor.rs            seven install-chain checks; the ONLY path allowed to fail loudly
rust/src/models.rs            probes for the retired data file; always returns empty
rust/tests/fail_silent.rs     pathological stdin, panic silence, argument dispatch
rust/tests/parity.rs          golden-output regression corpus (347 inputs)
rust/tests/golden/            expectations.json + README (how to add an entry by hand)
rust/tests/doctor.rs          process-level doctor tests, incl. CLAUDE_PROJECT_DIR wiring
tests/fixtures/*.json         two real captured tool inputs (kling2_6 positive, seedance negative)
data/.gitkeep                 data/ must exist: doctor and models.rs walk up to find it
docs/archive/verified-port/   the retired Python oracle, read-only historical record
docs/superpowers/plans/       the five plans; rule-regrounding is live
HIGGSFIELD_PROMPT_RESEARCH.md first-party evidence audit (HFR-001..007, HFR-R1, G1..G20)
```

### Flow of a call

1. `main()` checks `argv[1] == "doctor"` before reading stdin. Any other argument,
   recognised or not, falls through to the hook path so nothing can leak a diagnostic
   dump into `additionalContext`.
2. `run()` reads stdin, parses JSON, takes `tool_input`. `tool_name` is currently
   ignored (task 6 will thread it through).
3. `lint::lint()` handles the batch shape first: if `requests` is a non-empty array, each
   dict entry's `params` is resolved and linted, findings are prefixed `request <index>:`
   (explicit `index` rendered Python-`str()` style, else positional), and the top-level
   `params` is **never** consulted. Otherwise top-level `params` is resolved.
4. `resolve_params` accepts an object or a JSON-encoded string that decodes to an object
   (all three tool schemas declare `params` as `anyOf: [object, string]`). Anything else
   is silence.
5. `lint_params` runs the two checks in order; `render()` builds the text block; `main`
   emits the JSON and explicitly flushes stdout.

### Notable implementation details in `checks.rs`

- The `regex` crate has no lookaround, so `has_term` / `find_original_case` are a manual
  boundary scan over `char_indices` (never byte offsets, so non-ASCII is safe). It never
  materialises the haystack; an earlier version hit 2.3 GB RSS on an 18 MB input.
- `has_term` is case-**sensitive**; `find_original_case` is case-insensitive. This
  mirrors the archived oracle deliberately. `check_prompt_negative_conflict` lowercases
  both fields itself before calling `has_term`.
- `quoted_spans` pairs `"` characters purely positionally (1st with 2nd, 3rd with 4th).
  Two documented edges: stray inch marks can swallow a live directive; `""term""` leaves
  the term live. Both are accepted, not special-cased.
- `is_excluded` is a `partition_point` binary search (fix F1). A linear scan made an
  adversarial prompt of repeated quoted segments take 9 s.

### `doctor`

Seven ordered checks: binary location, retired data file absent, binary freshness by
mtime against `rust/src/*.rs` / `Cargo.toml` / `data/*.json`, settings file parses with a
non-empty `PreToolUse`, matcher present (command resolves to this exact exe under the
real `CLAUDE_PROJECT_DIR` when set), matcher correctness (matches a real tool id, rejects
`generate_3d` and `show_generations`), and an in-process self-test on a known-offending
payload. Exit 1 on any FAIL; WARN does not fail.

The freshness check exists because SPEC.md §12.1's failure mode is real: a missing or
stale binary exits 127 or runs old logic, `PreToolUse` treats that as non-blocking, and
nothing distinguishes "lint didn't run" from "lint found nothing".

---

## 4. Testing and the golden corpus (SPEC.md §9, §12.2)

**The Rust is the specification.** The Python oracle proved the port at 320 inputs / 0
divergences plus 4,200 fuzzed inputs, then was archived read-only. `parity.rs` now
generates the same corpus (fixtures, pathological inputs, and sweeps over every lexicon
term, dimension pair, negation form, batch shape, plus the FP controls) and compares
parsed JSON against `rust/tests/golden/expectations.json`.

Each golden entry carries `label`, `input_len`, `input_fnv1a64`, and `output` (or
`null` for silence). The hash catches generator drift separately from behaviour drift.
Missing, orphaned, and byte-colliding labels each fail loudly.

**There is deliberately no regeneration tool.** A flipped expectation is a behaviour
change: update `SPEC.md` first, then the code, then edit the affected entries by hand in
the same commit, reviewed individually. `rust/tests/golden/README.md` has the procedure
(add a placeholder, run once, copy the real values out of the failure message).

Sweep constants in `parity.rs` (`RESOLUTION_TERMS`, `CONFLICT_PHRASES`,
`KNOWN_RESOLUTION_PARAM_VALUES`) mirror the lexicons in `checks.rs` and must move with
them.

Verified state at `a7e0039`:

| Command | Result |
|---|---|
| `cargo test --release` | 80 unit + 4 doctor + 4 fail_silent + 1 parity, all pass |
| golden corpus | 347 / 347 / 0 divergences |
| `target/release/higgsfield-lint doctor` | 7/7 PASS, exit 0 |
| binary vs sources | binary newer than every source |

---

## 5. Operating rules the plans impose

From `2026-08-31-rule-regrounding.md` (live) and its predecessors:

- **`SPEC.md` changes first, then code.** Spec and rule change are one unit.
- **Never edit a test or a golden entry to make a red diff green.** Every flip is
  justified in the task report.
- **Every confirmed false positive becomes a permanent negative control**, added in the
  same task as the fix, and must be shown to fail before the fix and pass after.
- **Precision in both directions.** A check that fires on a known-good call is a bug.
- **`doctor` must stay 7/7 throughout.**
- **The hook path is sacred.** `doctor` is the only code allowed to exit non-zero or
  write to stderr.
- **Never edit a fixture** to make a check pass.
- **The archived Python must never be edited** to match later behaviour.
- **Creating the credit-expiry scheduled task is a human action**, not the plan's.
- Model policy for the live plan: implementation `claude-sonnet-5`, review
  `claude-opus-5`, Task 7 docs `claude-haiku-4-5`.
- The `.superpowers/sdd/` ledgers (briefs, reports, review diffs, `progress.md`) are
  gitignored scratch, not a citable source.

---

## 6. Where the work stands

### Plans, in order

| Plan | State |
|---|---|
| `2026-08-30-higgsfield-lint.md` (Python) | done, then ported |
| `2026-08-30-project-intent-profile.md` | never executed; stack no longer exists; superseded |
| `2026-08-31-rust-port.md` | done |
| `2026-08-31-doctor-and-expiry.md` | tasks 1–3 done; task 4 (docs) absorbed into rule-regrounding task 7 |
| `2026-08-31-rule-regrounding.md` | **live**: tasks 1–3 done, task 4 in progress, 5–7 not started |

### Rule-regrounding task 4: re-ground `resolution_claim`

Done: step 1, spec text only (`a7e0039`, unpushed, spec-only diff). Remaining, per the
plan and `task-4-brief.md`:

1. Add the two FP controls to the golden corpus and unit tests: Higgsfield's
   `Style: 8K IMAX commercial … 8K detail.` with `resolution: "1080p"`, and the diegetic
   `The presenter says "shot in 4K" to the camera` with real dimensions. Both must be
   silent.
2. Run them, watch them fail (the 8K case fires today; the quoted case fires today).
3. Drop `8k`, `uhd`, `full hd` from `RESOLUTION_CLAIMS`.
4. Wire `quoted_spans` / `has_term_excluding` into `claimed_short_side`. The helper
   already exists; it is not yet called from the resolution path.
5. Extend `RESOLUTION_PARAM` with `2160p`, `2k`, `1.5k`, `1k`, `768`, `512`.
6. Condition the message on the source of `actual` (spec already words both variants).
7. Review every golden flip individually; update `parity.rs` sweep constants.

Expected corpus impact, for review planning:

- ~34 entries go silent: the `resolution:{8k,8K,uhd,UHD,full hd,FULL HD}:*` sweep and all
  `dims:*` cases. The `dims` sweep uses an `8k` claim on purpose (fix F2) so it would
  lose all discrimination; its prompt needs a surviving claim such as `4k`, which changes
  its bytes and hashes.
- `resolution_param:1k` and `resolution_param:2k` start firing (1024 and 2048 < 2160).
- Every width/height-sourced firing message changes wording (`batch:*`, `combined:*`,
  `fixture:kling2_6_*`, `edge:unicode_prompt`, and the `resolution:*:WxH` firings).
- `check_self_test` in `doctor.rs` and `lint.rs`'s multi-finding unit test use
  `1280x720` with a `4k` claim, so they will need the new message wording too.

### Tasks 5–7

- **Task 5** `duration_claim_conflict` (HFR-001, over-claim branch only, `generate_video`
  and `generate_video_batch` only). Trigger and non-trigger lists are in research §10.
- **Task 6** tool-name scoping: thread `tool_name` through `lint()`, skip prompt-text
  checks on `generate_audio*` where `prompt` is spoken text.
- **Task 7** documentation reconciliation (`SPEC.md` doctor section, credit-expiry
  section, §9's over-broad `anyOf` claim, evidence base, README).

### Loose ends observed

- Task 3's final scoped re-review was dispatched but no verdict is recorded in the
  ledger.
- `a7e0039` is local-only; origin is at `c9d1267`.
- The credit-expiry weekly task (`docs/credit-expiry-task.md`, Mondays 09:00, 30-day
  cycle anchored on the last `Subscription Credits Reset` row) is specified but not
  created.
- `SPEC.md` §9 says 325 corpus inputs; actual is 347.
- The F1 scaling regression test is wall-clock based and could flake under load.
- `expectations.json`'s `note` field stops at task 3 step 5; `captured_from_commit` is
  `8638390`.
- Several present-tense "port of the oracle" comments survive in `checks.rs` / `lint.rs`.
- `models.rs` still walks the filesystem on every invocation for a file that no longer
  exists.
