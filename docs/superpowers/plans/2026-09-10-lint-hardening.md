# Lint Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the nine findings from the 2026-09-10 staff review — ship the unbuilt §8
reminder, make a dead hook detectable, give the lint an evidence trail it can learn from,
and fix the measured false positives and parity divergences.

**Architecture:** The hook stays a single fail-silent Rust binary. Three structural
changes: (1) the crate gains a `lib` target so a second, chatty read-only binary
(`higgsfield-lint-review`) can share its modules; (2) the hook appends one JSONL record
per call — metadata only, never prompt text — which turns every open question about
precision into a query; (3) freshness detection moves out of the generation path into a
`SessionStart` hook, the one place noise is affordable.

**Tech Stack:** Rust 2021 (`serde_json`, `regex`), Python 3 stdlib (parity oracle only),
pytest, GitHub Actions, POSIX sh.

**Spec:** `SPEC.md` (this plan amends §5.2, §6, §7, §10, §12.2, §12.3 — see Task 4)

## Global Constraints

Copied verbatim from `SPEC.md`; every task's requirements implicitly include these.

- **The hook never blocks.** It never returns `permissionDecision: "deny"` (§4).
- **The hook never exits non-zero and never writes to stderr on any path** (§7). Exit 2
  plus stderr is a blocking signal.
- **Any exception, malformed stdin, or missing field exits 0 with no output** (§7).
- **No network calls, no MCP access from the hook** (§7).
- **`panic = "unwind"` in `[profile.release]` must stay.** An abort profile makes panics
  uncatchable, and an uncaught panic writes to stderr and exits 101.
- **Dependencies stay at exactly two: `serde_json` and `regex`.** Do not add crates.
- **`src/higgsfield_lint.py` is never edited to make a parity comparison pass** (§12.2).
  Task 4 narrows this rule; until Task 4 lands, treat it as absolute.
- **Findings are one line each:** `<check_id>: <what was observed>. <mechanical fix>.` (§6)

## Baseline (measured 2026-09-10, before any task)

Re-run these before starting. If any differs, stop and reconcile — the plan assumes them.

| Command | Result |
|---|---|
| `cargo build --release` | clean, ~5s |
| `cargo test --release` | 46 unit + 2 fail_silent + 1 parity, all pass |
| `pytest tests/ -q` | 58 passed |
| `cargo fmt --check` | **FAILS** — drift in `checks.rs`, `lint.rs`, `main.rs`, both test files |
| `cargo clippy --release --bin higgsfield-lint` | **3 warnings** — `checks.rs:68`, `checks.rs:123`, `lint.rs:45` |
| `ls target/` | **absent on a fresh worktree** — this is Finding P0-1 |

## File Structure

**New files**

| File | Responsibility |
|---|---|
| `rust/src/lib.rs` | Crate root for the library target; declares `checks`, `lint`, `log`, `models`. Nothing else. |
| `rust/src/log.rs` | Best-effort append of one JSONL record per call. Owns the record schema and the kill-switch. Never returns an error. |
| `rust/src/bin/review.rs` | `higgsfield-lint-review`: reads the log, prints per-check fire rates and model-promotion candidates. Read-only, chatty, may exit non-zero. |
| `scripts/check-hook-freshness.sh` | `SessionStart` hook. Emits one `additionalContext` line when the binary is missing or older than `rust/src/*.rs`. |
| `rust/tests/behaviour.rs` | The three bounded CI gates: bad prompts, missing details, inaccurate or harmful advice. Table-driven; one `#[test]` per class so a red build names the class. |
| `.github/workflows/ci.yml` | Runs the three gates as named steps, then the supporting suites, then hygiene. |
| `requirements-dev.txt` | Declares pytest, which `tests/test_lint.py:7` imports undeclared today. |

**Modified files**

| File | Change |
|---|---|
| `rust/src/main.rs` | Becomes a thin bin over the lib; adds the `log::record` call. |
| `rust/src/models.rs` | `locate_data_file` generalised to `locate_repo_root`. |
| `rust/src/lint.rs` | `resolve_params` becomes `pub`. |
| `rust/src/checks.rs` | Unicode fold table (Task 5); tag-segment gate (Task 8). |
| `src/higgsfield_lint.py` | Tag-segment gate only (Task 8), under the Task 4 policy. |
| `rust/tests/parity.rs` | Sets the log kill-switch; adds Unicode and tag-segment corpora. |
| `rust/tests/fail_silent.rs` | Sets the kill-switch; adds logging-specific silence tests. |
| `Cargo.toml` | `[lib]` target, second `[[bin]]`. |
| `.claude/settings.json` | Adds the `SessionStart` hook. |
| `.gitignore` | Adds `data/findings.jsonl`. |
| `SPEC.md`, `README.md` | Amendments per Task 4. |

**Dependency order.** Task 4 (spec amendments) gates Task 6 and Task 8. Task 6 (the lib
refactor + log) gates Task 7. Everything else is independent.

---

### Task 1: Ship the credit-expiry reminder (SPEC §8)

`SPEC.md:160-167` states this is "worth more than every check in §5 combined" — 6,910
credits forfeited to expiry against ~732 ever spent. It has never existed as anything but
a prompt in `docs/credit-expiry-task.md`. No code. Do this first.

**Files:**
- Modify: `docs/credit-expiry-task.md`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Load the scheduling tool**

The hook cannot call MCP tools (§8), so this is a scheduled agent, not a hook. Load the
cron tool schema:

```
ToolSearch with query "select:CronCreate,CronList"
```

If `CronCreate` is unavailable in the session, use the `/schedule` skill instead. The
requirement is a weekly agent run with the Higgsfield MCP reachable — a cloud routine
without that MCP connected will fail, so prefer the local cron path.

- [ ] **Step 2: Create the weekly routine**

Schedule: `0 9 * * MON` (Mondays 09:00 local). Renewal falls between the 5th and 7th, so
a weekly cadence guarantees at least one warning inside the final week.

Prompt (verbatim from `docs/credit-expiry-task.md`, with the reporting requirement made
explicit so a run either produces a number or visibly fails):

```
Call the Higgsfield `balance` tool. Report the credit total and how many days remain
until the next monthly reset, assuming reset on the 6th. If more than 400 credits
remain and fewer than 10 days are left, say so plainly and note that unused credits
are wiped at reset rather than carried over. Always state the numeric credit balance
you read, even when no warning is warranted. If the `balance` tool is unavailable,
say so explicitly rather than estimating.
```

- [ ] **Step 3: Trigger one run immediately and verify**

Do not wait for Monday. Run the routine once now.

Expected: output contains a numeric credit balance read from the `balance` tool.
If it reports the tool is unavailable, the schedule is worthless — fix MCP reachability
for scheduled runs before continuing, or the task is not done.

- [ ] **Step 4: Record the routine in the doc**

Append to `docs/credit-expiry-task.md`:

```markdown
## Status

Scheduled 2026-09-10. Weekly, Mondays 09:00 local (`0 9 * * MON`).
Verified by a manual trigger on 2026-09-10, which returned a live balance.
List existing routines with `CronList` (or `/schedule`) to confirm it is still active.
```

- [ ] **Step 5: Commit**

```bash
git add docs/credit-expiry-task.md
git commit -m "feat: schedule the credit-expiry reminder (SPEC 8)"
```

---

### Task 2: Bounded CI — three behavioural gates

A generic "run everything" job tells you something broke, not what class of thing broke.
For an advisory lint there are exactly three classes worth gating on, and CI is bounded to
them:

1. **Bad prompts** — prompt content that is empty, degenerate, non-string or pathological.
   §7 requires silence on all of it, and the hook is in the path of every generation call.
2. **Missing details** — the call omits a field a check depends on: no `model`, no
   `params`, no dimensions, an empty `negative_prompt`. The lint must degrade to silence
   rather than guess. §5.3 is explicit that guessing produces false findings on models
   that silently ignore the field.
3. **Inaccurate or harmful advice** — it fires on a known-good call, which §9 calls a bug
   outright, or it advises a change that would do nothing or damage the shot (§5.3's
   quantifier idioms and manner adverbials).

Everything else CI runs — parity, fail-silent, pytest, clippy, fmt — stays, but as
supporting steps. The three gates are the contract.

Every case below was measured against the release binary on 2026-09-10 before being
written down, so this file passes the moment it is created. Add a case when a real call
proves one, never when one is imagined.

**Files:**
- Create: `requirements-dev.txt`, `rust/tests/behaviour.rs`, `.github/workflows/ci.yml`
- Modify: `Cargo.toml`, `rust/src/checks.rs:68`, `rust/src/checks.rs:123`, `rust/src/lint.rs:45`

**Interfaces:**
- Consumes: nothing.
- Produces: `cargo test --release --test behaviour` with three filterable test names
  (`gate_1_bad_prompts`, `gate_2_missing_details`, `gate_3_inaccurate_or_harmful_advice`),
  plus a green `cargo clippy -- -D warnings` and `cargo fmt --check` that later tasks must
  keep green.

- [ ] **Step 1: Declare the Python dev dependency**

`tests/test_lint.py:7` imports pytest with no requirements file anywhere in the repo.
Create `requirements-dev.txt`:

```
pytest>=8,<9
```

- [ ] **Step 2: Normalise formatting**

`cargo fmt --check` fails today across five files. Run the formatter once so CI can gate
on it afterwards:

```bash
cargo fmt
```

- [ ] **Step 3: Run the tests to confirm formatting changed nothing**

Run: `cargo test --release && pytest tests/ -q`
Expected: 46 + 2 + 1 pass, 58 pass. Formatting is not allowed to change behaviour; if
anything fails here, revert and investigate before continuing.

- [ ] **Step 4: Commit the formatting pass separately**

A pure-formatting commit keeps the next diff readable.

```bash
git add -A
git commit -m "style: cargo fmt across the tree"
```

- [ ] **Step 5: Fix the two doc-indentation warnings**

`rust/src/checks.rs:68` and `rust/src/lint.rs:45` each contain a doc-comment list item
whose continuation lines are not indented, so `clippy::doc_lazy_continuation` fires.
Indent the continuation lines of those list items by two spaces so the list renders as a
list. Do not reword the prose — it is accurate.

- [ ] **Step 6: Fix the case-comparison warning**

`rust/src/checks.rs:123` is the `eq` closure inside `find_term_match`. The lint is
`clippy::manual_ignore_case_cmp` — verified against clippy 1.98.0. Do not guess this
name: an unknown lint inside an `#[allow]` trips `unknown_lints`, which fails the
`-D warnings` step two steps below.

**Task 5 rewrites this closure entirely.** Do not restructure it here. Silence it for now,
with a comment that names its successor:

```rust
    // Replaced wholesale in the Unicode-fold task; clippy's `eq_ignore_ascii_case`
    // suggestion is the wrong shape because the fold is about to stop being ASCII-only.
    #[allow(clippy::manual_ignore_case_cmp)]
    let eq = |a: char, b: char| {
        if ignore_case {
            a.to_ascii_lowercase() == b.to_ascii_lowercase()
        } else {
            a == b
        }
    };
```

- [ ] **Step 7: Verify clippy is clean**

Run: `cargo clippy --all-targets --release -- -D warnings`
Expected: exit 0, no warnings. If it reports `unknown_lints`, the name in Step 6 is wrong.

- [ ] **Step 8: Write the three gates**

Create `rust/tests/behaviour.rs`. This is the rustfmt-normalised form - Step 12 gates on
`cargo fmt --check`, so pasting an unformatted version would fail the task's own final
verification:

```rust
//! Bounded behavioural gates. CI runs these as three separately-named steps so a red
//! build names the class that broke rather than just "a test failed".
//!
//! An advisory lint has exactly three ways to become worth deleting:
//!
//!   1. It chokes or talks nonsense on a bad prompt (SPEC 7 - it is in the path of every
//!      generation call, so its own failure must be invisible).
//!   2. It mishandles a call that omits details, guessing where SPEC 5.3 requires silence.
//!   3. It gives advice that is inaccurate, or that would damage the shot if followed.
//!
//! Every case here was measured against the release binary before being written down.
//! Add a case when a real call proves one, not when one is imagined.

use std::io::Write;
use std::process::{Command, Stdio};

/// Feed one `tool_input` to the hook and return the check ids it emitted, in order.
/// An empty vec means the hook was silent.
///
/// Also asserts the two global constraints on every single case, for free: exit 0 and an
/// empty stderr. A gate case that broke either would otherwise pass unnoticed.
fn fired(tool_input: &str) -> Vec<String> {
    let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
    let mut child = Command::new(exe)
        // Harmless before the findings log exists; suppresses it afterwards. The gates
        // are not usage and must not land in the log.
        .env("HIGGSFIELD_LINT_NO_LOG", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    child
        .stdin
        .as_mut()
        .expect("piped stdin")
        .write_all(format!(r#"{{"tool_input":{tool_input}}}"#).as_bytes())
        .ok();
    let out = child.wait_with_output().expect("wait");
    assert_eq!(out.status.code(), Some(0), "the hook must always exit 0");
    assert!(
        out.stderr.is_empty(),
        "the hook must never write to stderr, got: {:?}",
        String::from_utf8_lossy(&out.stderr)
    );
    if out.stdout.is_empty() {
        return Vec::new();
    }
    let v: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("hook output must be valid JSON");
    v["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .expect("additionalContext must be a string")
        .lines()
        .skip(1) // the "higgsfield-lint: N finding(s)" header
        .filter_map(|l| {
            l.strip_prefix("- ")
                .and_then(|rest| rest.split(':').next())
                .map(str::to_string)
        })
        .collect()
}

/// Run every case in a gate and report ALL failures together. A gate that stops at the
/// first mismatch makes you re-run CI once per broken case.
fn run_gate(name: &str, cases: &[(&str, &str, &[&str])]) {
    let mut failures = Vec::new();
    for (label, tool_input, expected) in cases {
        let got = fired(tool_input);
        let want: Vec<String> = expected.iter().map(|s| (*s).to_string()).collect();
        if got != want {
            failures.push(format!(
                "  {label}\n    expected: {want:?}\n    got:      {got:?}"
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "gate '{name}': {} of {} case(s) failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

// ---------------------------------------------------------------------------------------
// Gate 1: bad prompts
// ---------------------------------------------------------------------------------------

#[test]
fn gate_1_bad_prompts() {
    let cases: &[(&str, &str, &[&str])] = &[
        (
            "empty prompt",
            r#"{"params":{"prompt":"","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "whitespace-only prompt",
            r#"{"params":{"prompt":"   ","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "punctuation-only prompt",
            r#"{"params":{"prompt":"!!! ??? ---","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "prompt is a number",
            r#"{"params":{"prompt":123,"width":1280,"height":720}}"#,
            &[],
        ),
        (
            "prompt is null",
            r#"{"params":{"prompt":null,"width":1280,"height":720}}"#,
            &[],
        ),
        // Characterisation, not endorsement. A prompt of bare negators matches the
        // negation pattern and yields a nonsense suggestion. Pinned so it cannot drift
        // silently; not worth a rule, because the input is already nonsense and no real
        // call looks like this.
        (
            "prompt is bare negators",
            r#"{"params":{"model":"kling2_6","prompt":"no no no no no","width":1280,"height":720}}"#,
            &["negation_placement"],
        ),
        // Real prompts are not always comma-delimited; newline-separated directives must
        // still be found.
        (
            "newline-separated directives",
            r#"{"params":{"model":"kling2_6","prompt":"a street\nno logos\nno text","width":1280,"height":720}}"#,
            &["negation_placement"],
        ),
    ];
    run_gate("bad prompts", cases);
}

// ---------------------------------------------------------------------------------------
// Gate 2: missing details
// ---------------------------------------------------------------------------------------

#[test]
fn gate_2_missing_details() {
    let cases: &[(&str, &str, &[&str])] = &[
        // The documented blind spot, pinned deliberately. With no `model` and no
        // `negative_prompt`, nothing proves this model accepts one, and SPEC 5.3 requires
        // silence over a guess. If this ever starts firing, someone has made the lint
        // infer negative-prompt support - which is exactly what 5.3 forbids.
        (
            "no model, no negative_prompt",
            r#"{"params":{"prompt":"a street, no logos, no text overlays","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "no model, negative_prompt present",
            r#"{"params":{"prompt":"a street, no logos","negative_prompt":"blurry","width":1280,"height":720}}"#,
            &["negation_placement"],
        ),
        // An all-whitespace negative_prompt is not evidence of support.
        (
            "negative_prompt is whitespace",
            r#"{"params":{"prompt":"a street, no logos","negative_prompt":"   ","width":1280,"height":720}}"#,
            &[],
        ),
        // A malformed negative_prompt is not evidence either - but the model is in the
        // known-accepts list, so the check still fires on that basis alone.
        (
            "negative_prompt is not a string, model is known",
            r#"{"params":{"model":"kling2_6","prompt":"a street, no logos","negative_prompt":123,"width":1280,"height":720}}"#,
            &["negation_placement"],
        ),
        ("params absent entirely", r#"{"model":"kling2_6"}"#, &[]),
        (
            "prompt absent",
            r#"{"params":{"width":1280,"height":720}}"#,
            &[],
        ),
        // No way to know what the call will actually produce, so the claim cannot be
        // contradicted.
        (
            "no dimensions and no resolution",
            r#"{"params":{"prompt":"cinematic 4k"}}"#,
            &[],
        ),
        (
            "batch entry missing params",
            r#"{"requests":[{"index":0}]}"#,
            &[],
        ),
    ];
    run_gate("missing details", cases);
}

// ---------------------------------------------------------------------------------------
// Gate 3: inaccurate or harmful advice
// ---------------------------------------------------------------------------------------

#[test]
fn gate_3_inaccurate_or_harmful_advice() {
    let cases: &[(&str, &str, &[&str])] = &[
        // SPEC 5.3 names these explicitly: moving any of them into negative_prompt would
        // do nothing, or damage the shot. Firing here is worse than staying quiet.
        (
            "quantifier idiom: no more than",
            r#"{"params":{"model":"kling2_6","prompt":"a room with no more than three people","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "quantifier idiom: no longer",
            r#"{"params":{"model":"kling2_6","prompt":"the sun no longer visible","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "quantifier idiom: no matter",
            r#"{"params":{"model":"kling2_6","prompt":"walking on, no matter the weather","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "manner adverbial: without moving",
            r#"{"params":{"model":"kling2_6","prompt":"a figure sitting without moving","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "narrative negation: has not yet risen",
            r#"{"params":{"model":"kling2_6","prompt":"the sun has not yet risen","width":1280,"height":720}}"#,
            &[],
        ),
        // The other half of the same contract: the guards above must not be over-applied.
        // "no flickering" and "avoid warping" name a defect to exclude and must fire, even
        // though the first also ends in "-ing".
        (
            "real defect: no flickering",
            r#"{"params":{"model":"kling2_6","prompt":"a steady shot, no flickering","width":1280,"height":720}}"#,
            &["negation_placement"],
        ),
        (
            "real defect: avoid warping",
            r#"{"params":{"model":"kling2_6","prompt":"a steady shot, avoid warping","width":1280,"height":720}}"#,
            &["negation_placement"],
        ),
        // A satisfied resolution claim must not be second-guessed: telling someone to
        // raise a resolution that is already correct is advice that costs attention and
        // buys nothing.
        (
            "4k claim at 3840x2160",
            r#"{"params":{"prompt":"cinematic 4k","width":3840,"height":2160}}"#,
            &[],
        ),
        (
            "4k claim with resolution=4k",
            r#"{"params":{"prompt":"cinematic 4k","resolution":"4k","width":100,"height":100}}"#,
            &[],
        ),
        (
            "720p claim at 1920x1080",
            r#"{"params":{"prompt":"shot in 720p","width":1920,"height":1080}}"#,
            &[],
        ),
        // An aspect-ratio pair is not a render size. Reading 16x9 as a 9px short side
        // would fire on essentially every call that expresses its shape this way.
        (
            "aspect ratio pair is not a render size",
            r#"{"params":{"prompt":"cinematic 4k","width":16,"height":9}}"#,
            &[],
        ),
    ];
    run_gate("inaccurate or harmful advice", cases);
}

#[test]
fn gate_3_known_good_fixtures_stay_silent() {
    // SPEC 9: "a check that fires on a known-good call is a bug". The seedance fixtures
    // are real captured calls that must produce nothing. They matter more than the
    // positives - the whole value of an advisory tool is that its advice is worth reading.
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let text = std::fs::read_to_string(dir.join("seedance_tree_clean.json"))
        .expect("seedance_tree_clean.json must exist");
    let got = fired(&text);
    assert!(
        got.is_empty(),
        "the known-good seedance fixture fired: {got:?}"
    );
}
```

- [ ] **Step 9: Register the test target**

Add to `Cargo.toml`, next to the existing `[[test]]` entries:

```toml
[[test]]
name = "behaviour"
path = "rust/tests/behaviour.rs"
```

- [ ] **Step 10: Run the gates**

Run: `cargo test --release --test behaviour`
Expected: 4 tests pass. Every case was measured against the current binary before being
written, so a failure here means the binary is not the one that was measured — rebuild
before debugging the test.

Then confirm each gate is independently selectable, which is what the workflow relies on:

```bash
cargo test --release --test behaviour gate_1
cargo test --release --test behaviour gate_2
cargo test --release --test behaviour gate_3
```

Expected: 1, 1 and 2 tests run respectively.

- [ ] **Step 11: Write the CI workflow**

Create `.github/workflows/ci.yml`. The three gates are named steps so a red build says
which class broke:

```yaml
name: ci

on:
  push:
  pull_request:

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt

      - uses: actions/setup-python@v5
        with:
          # Pinned, not floating: the parity oracle is a Python program and `re` semantics
          # are what parity is measured against. A divergence that appears only in CI is a
          # Python-version-dependent `re` behaviour and belongs in SPEC.md 12.3, not in a
          # silent toolchain bump. If local and CI disagree, reconcile the versions before
          # touching the code.
          python-version: '3.12'

      - run: python -m pip install -r requirements-dev.txt

      - run: cargo build --release

      # The contract: three bounded classes of failure, named individually.
      - name: "gate 1 - bad prompts"
        run: cargo test --release --test behaviour gate_1 -- --nocapture
      - name: "gate 2 - missing details"
        run: cargo test --release --test behaviour gate_2 -- --nocapture
      - name: "gate 3 - inaccurate or harmful advice"
        run: cargo test --release --test behaviour gate_3 -- --nocapture

      # Supporting suites.
      - name: "fail-silent on pathological input"
        run: cargo test --release --test fail_silent
      - name: "differential parity against the Python oracle"
        run: cargo test --release --test parity
      - name: "unit tests"
        run: cargo test --release --bin higgsfield-lint
      - name: "python oracle tests"
        run: pytest tests/ -q

      # Hygiene, last: a formatting nit should not mask a gate failure in the log.
      - run: cargo clippy --all-targets --release -- -D warnings
      - run: cargo fmt --check
```

- [ ] **Step 12: Verify the workflow locally**

CI cannot be verified without pushing, so run the same commands in the same order:

```bash
cargo build --release && \
cargo test --release --test behaviour gate_1 && \
cargo test --release --test behaviour gate_2 && \
cargo test --release --test behaviour gate_3 && \
cargo test --release --test fail_silent && \
cargo test --release --test parity && \
cargo test --release --bin higgsfield-lint && \
pytest tests/ -q && \
cargo clippy --all-targets --release -- -D warnings && \
cargo fmt --check
```

Expected: every command exits 0.

- [ ] **Step 13: Commit**

```bash
git add requirements-dev.txt rust/tests/behaviour.rs .github/workflows/ci.yml \
        Cargo.toml rust/src/checks.rs rust/src/lint.rs
git commit -m "ci: bound the gate to bad prompts, missing details and harmful advice"
```

---

### Task 3: SessionStart freshness guard

`.claude/settings.json:8` runs `$CLAUDE_PROJECT_DIR/target/release/higgsfield-lint`;
`.gitignore:9` ignores `target/`. On a fresh clone or worktree the binary does not exist,
the hook exits 127, `PreToolUse` treats that as non-blocking, and the result is
indistinguishable from a clean lint. `README.md:14-22` documents this instead of fixing
it. The fix cannot live in the hook — a missing binary cannot report itself — so it goes
in `SessionStart`, which is outside the generation path and can afford to be loud.

**Files:**
- Create: `scripts/check-hook-freshness.sh`
- Modify: `.claude/settings.json`, `README.md:11-22`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Write the freshness script**

Create `scripts/check-hook-freshness.sh`. Plain POSIX sh, no dependencies. It prints
nothing and exits 0 in the healthy case, so a correctly-built repo stays silent.

```sh
#!/bin/sh
# SessionStart hook: the PreToolUse lint is a compiled binary that is gitignored, so a
# fresh clone or worktree has no binary at all. A missing or stale binary fails silently
# and non-blockingly at PreToolUse time (SPEC 12.1) - indistinguishable from a clean
# lint. This is the one place that difference can be surfaced without risking the
# generation path, because SessionStart is not in it.
#
# Exits 0 always. Prints nothing when the binary is present and newer than every source
# file. Never writes to stderr.

set -u

root="${CLAUDE_PROJECT_DIR:-.}"
bin="$root/target/release/higgsfield-lint"
src="$root/rust/src"

[ -d "$src" ] || exit 0   # not this repo; say nothing

if [ ! -x "$bin" ]; then
    msg="higgsfield-lint: the PreToolUse hook binary is MISSING (target/release/higgsfield-lint). Higgsfield generate_* calls are NOT being linted right now, and a missing binary looks exactly like a clean lint. Run: cargo build --release"
elif [ -n "$(find "$src" -name '*.rs' -newer "$bin" -print 2>/dev/null | head -n 1)" ]; then
    msg="higgsfield-lint: the PreToolUse hook binary is STALE - rust/src/*.rs is newer than target/release/higgsfield-lint, so the lint running is not the lint in the tree. Run: cargo build --release"
else
    exit 0
fi

# The message is deliberately free of double quotes, backslashes and control characters,
# so it needs no JSON escaping.
printf '{"hookSpecificOutput":{"hookEventName":"SessionStart","additionalContext":"%s"}}' "$msg"
exit 0
```

- [ ] **Step 2: Make it executable and verify the missing-binary branch**

```bash
chmod +x scripts/check-hook-freshness.sh
mv target/release/higgsfield-lint /tmp/hl-backup
CLAUDE_PROJECT_DIR="$PWD" sh scripts/check-hook-freshness.sh
```

Expected: one line of JSON containing `is MISSING`. Verify it is valid JSON:

```bash
CLAUDE_PROJECT_DIR="$PWD" sh scripts/check-hook-freshness.sh | python3 -m json.tool
```

Expected: parses, `hookEventName` is `SessionStart`.

- [ ] **Step 3: Verify the stale branch**

```bash
mv /tmp/hl-backup target/release/higgsfield-lint
touch rust/src/checks.rs
CLAUDE_PROJECT_DIR="$PWD" sh scripts/check-hook-freshness.sh
```

Expected: one line of JSON containing `is STALE`.

- [ ] **Step 4: Verify the healthy branch is silent**

```bash
cargo build --release
CLAUDE_PROJECT_DIR="$PWD" sh scripts/check-hook-freshness.sh; echo "exit=$?"
```

Expected: no output at all, `exit=0`. A guard that talks on every healthy session gets
muted by the reader within a week, which would recreate the problem it exists to solve.

- [ ] **Step 5: Verify it is silent outside this repo**

```bash
CLAUDE_PROJECT_DIR=/tmp sh scripts/check-hook-freshness.sh; echo "exit=$?"
```

Expected: no output, `exit=0`.

- [ ] **Step 6: Register the hook**

Replace `.claude/settings.json` with:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "sh \"$CLAUDE_PROJECT_DIR/scripts/check-hook-freshness.sh\""
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "mcp__.*__generate_(image|video|audio)(_batch)?$",
        "hooks": [
          {
            "type": "command",
            "command": "\"$CLAUDE_PROJECT_DIR/target/release/higgsfield-lint\""
          }
        ]
      }
    ]
  }
}
```

- [ ] **Step 7: Verify end to end in a real session**

Delete the binary, start a fresh Claude Code session in this repo, and confirm the
warning reaches the model as context. Then rebuild and start another session and confirm
silence. This is the only step that proves the hook is actually wired; the shell tests
above only prove the script works.

If the warning does not appear, the `SessionStart` output contract is the suspect — check
the hook reference for whether `SessionStart` consumes `hookSpecificOutput.additionalContext`
or plain stdout, and adjust the final `printf` accordingly. Do not mark this task done on
the shell tests alone.

- [ ] **Step 8: Correct the README**

`README.md:11-22` currently ends on "**Nothing distinguishes 'the lint didn't run' from
'the lint ran and found nothing.'**" That sentence is now false. Replace that paragraph's
final two sentences with:

```markdown
A `SessionStart` hook (`scripts/check-hook-freshness.sh`) now closes this: it checks the
binary at the start of every session and injects a one-line warning when it is missing or
older than `rust/src/*.rs`. It stays silent when the build is current. That warning is the
only thing distinguishing "the lint didn't run" from "the lint ran and found nothing", so
do not remove it without replacing it.
```

- [ ] **Step 9: Commit**

```bash
git add scripts/check-hook-freshness.sh .claude/settings.json README.md
git commit -m "fix: detect a missing or stale hook binary at SessionStart"
```

---

### Task 4: Spec amendments — output contract, filesystem writes, oracle policy

Three documentation defects, all of which gate later tasks. Land them before the code
they authorise, so no later task has to argue with the spec while changing it.

**Files:**
- Modify: `SPEC.md:130-138` (§6), `SPEC.md` §7 bullet 3, `SPEC.md` §12.2

**Interfaces:**
- Consumes: nothing.
- Produces: the written policy that Task 6 (filesystem writes) and Task 8 (lockstep
  oracle edits) depend on.

- [ ] **Step 1: Fix the §6 output-contract drift**

The shipped code and the spec's example disagree. The code's wording is the later and
better one; the spec is stale. In the JSON example at `SPEC.md:136`, change:

- `Set resolution, or drop the claim from the prompt.` → `Raise resolution, or drop the claim from the prompt.`
- `Keep it in one.` → `Keep each term in one.`

Verify the spec now matches reality byte for byte:

```bash
cargo build --release
printf '%s' '{"tool_input":{"params":{"prompt":"cinematic 4K, slow motion","negative_prompt":"slow motion","width":1376,"height":768}}}' \
  | ./target/release/higgsfield-lint
```

Expected: the two finding lines in the output are character-identical to the two in
`SPEC.md:136`.

- [ ] **Step 2: Amend §7 to permit the findings log**

§7 currently reads "No network calls, no MCP access, no filesystem writes beyond reading
its own data file." Task 6 adds an append. Replace that bullet with:

```markdown
- No network calls and no MCP access. The hook reads its own data file and appends one
  metadata record per call to `data/findings.jsonl` (§9.1). That append is best-effort
  and load-bearing for nothing: every failure mode - unlocatable repo root, unwritable
  path, a record that would exceed the atomic-append size - is swallowed, and the lint's
  stdout is byte-identical whether the append succeeded or not. The invariant this bullet
  protects is not "no writes", it is "the hook's own failure is invisible"; a write that
  cannot fail visibly does not threaten it.
```

- [ ] **Step 3: Narrow the oracle read-only rule in §12.2**

Today §12.2 reads as an absolute ban, which would make every future check change
impossible without deleting the oracle. The rule's purpose is to stop the oracle being
edited to hide a port bug — not to freeze the lint's behaviour forever. Append to §12.2:

```markdown
**What "read-only" forbids, precisely.** The oracle is never edited to make a parity
comparison pass. A divergence discovered without a preceding spec change is a port bug,
and the Rust is what changes - always.

It is not a ban on the lint ever changing behaviour again. A deliberate change to a check
follows this order, and only this order:

1. Amend the relevant §5 subsection first, so the intended behaviour exists in writing
   before either implementation moves.
2. Change `src/higgsfield_lint.py` to match the amended spec.
3. Change `rust/src/checks.rs` to match.
4. Run `cargo test --release --test parity`. It must pass.

If step 4 fails, step 3 is wrong. The oracle is still never the thing that gets edited to
make step 4 green - it is edited only in step 2, from the spec, before the Rust moves at
all. A commit that touches the oracle and the Rust without touching §5 is the exact
failure this rule exists to prevent.
```

- [ ] **Step 4: Verify no code changed**

Run: `cargo test --release && pytest tests/ -q`
Expected: unchanged pass counts (46 + 2 + 1, and 58). This task is documentation only.

- [ ] **Step 5: Commit**

```bash
git add SPEC.md
git commit -m "docs: fix the 6 output contract, permit the findings log in 7, narrow the oracle rule in 12.2"
```

---

### Task 5: Unicode parity divergence in the boundary scan

`rust/src/checks.rs:86-92` uses `is_ascii_alphanumeric`; the oracle's `[a-z0-9]` under
`re.IGNORECASE` also matches four non-ASCII characters, and the same four literal-match
ASCII letters. This is a port bug, not a behaviour change: **the oracle does not move**
and no §5 amendment is needed.

Measured 2026-09-10 against Python 3.14 and the shipped release binary:

| Character | In `[a-z0-9]` under IGNORECASE | Literal-matches | Python | Rust |
|---|---|---|---|---|
| U+0130 `İ` | yes | `i` | blocks / matches | neither |
| U+0131 `ı` | yes | `i` | blocks / matches | neither |
| U+017F `ſ` | yes | `s` | blocks / matches | neither |
| U+212A `K` | yes | `k` | blocks / matches | neither |

Reproductions, both confirmed end to end:

- prompt `"a shot in ſ4k"` at 1280x720 → Python **silent** (the `ſ` blocks the boundary),
  Rust **fires**. Same for `ı4k`, `İ4k`, `4kſ`.
- prompt `"a shot in 4K"` (Kelvin sign) at 1280x720 → Python **fires**, quoting
  `"4K"`; Rust **silent**.

A third divergence lives in the regex paths: `(?i)[a-z-]` in the `regex` crate folds
`ſ`→`s` and `K`→`k` via Unicode simple folding and so already agrees, but does **not**
match `İ` or `ı`, which Python does. Confirmed: prompt `"a street, no ıce"` with
`model: kling2_6` → Python fires `negation_placement`, Rust silent. Same for `İce`.

`rust/tests/parity.rs:534-535` is the harness's only Unicode case and uses café, smart
quotes and naïve — none of the four characters. That is why 320 inputs found none of this.

**Files:**
- Modify: `rust/src/checks.rs:86-92`, `rust/src/checks.rs:121-127`, `rust/src/checks.rs:275-285`
- Test: `rust/tests/parity.rs` (new corpus), `rust/src/checks.rs` (unit tests)

**Interfaces:**
- Consumes: nothing.
- Produces: `fn fold_char(c: char) -> char` in `checks.rs` — private to the module.

- [ ] **Step 1: Write the failing parity cases**

Add this function to `rust/tests/parity.rs`:

```rust
/// The four characters Python's `re` folds into ASCII letters under `re.IGNORECASE`:
/// U+0130 I-with-dot, U+0131 dotless-i, U+017F long-s, U+212A KELVIN SIGN. Each is
/// exercised in all three roles the lint uses case-folding for: as a boundary character
/// abutting a lexicon term, as a substitute for the ASCII letter inside a term, and
/// inside the `[a-z-]` class of the two negation patterns.
fn unicode_fold_sweep(known: &str) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for (name, ch) in [("i_dot", '\u{130}'), ("dotless_i", '\u{131}'), ("long_s", '\u{17f}'), ("kelvin", '\u{212a}')] {
        // Boundary role: the character abuts "4k" and must block the claim, as it does
        // in Python. 1280x720 clears MIN_SANE_DIMENSION so a match would fire.
        out.push((
            format!("unicode:boundary_before:{name}"),
            wrap(TOOL_SINGLE, single(json!({"prompt": format!("a shot in {ch}4k"), "width": 1280, "height": 720}))),
        ));
        out.push((
            format!("unicode:boundary_after:{name}"),
            wrap(TOOL_SINGLE, single(json!({"prompt": format!("a shot in 4k{ch}"), "width": 1280, "height": 720}))),
        ));
        // Literal role: the character stands in for an ASCII letter inside a term.
        // Only kelvin/long_s/i-forms map onto letters that appear in the lexicons, so
        // build the substitution generically and let parity decide.
        out.push((
            format!("unicode:literal_4k:{name}"),
            wrap(TOOL_SINGLE, single(json!({"prompt": format!("a shot in 4{ch}"), "width": 1280, "height": 720}))),
        ));
        // Regex role: inside the `[a-z-]` class of PLACEMENT_NEGATION_RE.
        out.push((
            format!("unicode:negation:{name}"),
            wrap(TOOL_SINGLE, single(json!({"model": known, "prompt": format!("a street at dusk, no {ch}ce"), "width": 1280, "height": 720}))),
        ));
    }
    out
}
```

Then register it in `rust_matches_python_oracle_across_the_corpus`, after the existing
`extra_edge_sweep()` line (`rust/tests/parity.rs:625`):

```rust
    all.extend(extra_edge_sweep());
    all.extend(unicode_fold_sweep(&known));
```

- [ ] **Step 2: Run parity to verify it fails**

Run: `cargo test --release --test parity`
Expected: FAIL, reporting divergences on the `unicode:boundary_*` and `unicode:literal_4k`
cases (and `unicode:negation:i_dot` / `unicode:negation:dotless_i`).

- [ ] **Step 3: Add the fold table**

In `rust/src/checks.rs`, immediately above `is_boundary_char`, add:

```rust
/// The four non-ASCII characters Python's `re` treats as ASCII letters under
/// `re.IGNORECASE`. This is documented Python behaviour, not an accident: the `re` docs
/// state that `[a-z]`/`[A-Z]` with `IGNORECASE` match the 52 ASCII letters plus
/// U+0130, U+0131, U+017F and U+212A. Measured against Python 3.14 on 2026-09-10: each
/// is a member of `[a-z0-9]` under IGNORECASE, and each literal-matches exactly the
/// ASCII letter paired with it here.
///
/// The oracle gets this for free from `re`; the hand-rolled boundary scan below does not,
/// which is what made the two diverge. This table is the whole fix for that.
const UNICODE_ASCII_FOLDS: &[(char, char)] = &[
    ('\u{0130}', 'i'), // LATIN CAPITAL LETTER I WITH DOT ABOVE
    ('\u{0131}', 'i'), // LATIN SMALL LETTER DOTLESS I
    ('\u{017F}', 's'), // LATIN SMALL LETTER LONG S
    ('\u{212A}', 'k'), // KELVIN SIGN
];

/// Case-fold one character the way Python's `re` does under `IGNORECASE`: the four
/// entries above, then plain ASCII lowercasing. Non-ASCII characters with no entry are
/// returned unchanged, so they are neither boundary characters nor literal matches -
/// which is exactly what Python does with them.
fn fold_char(c: char) -> char {
    match UNICODE_ASCII_FOLDS.iter().find(|(from, _)| *from == c) {
        Some((_, to)) => *to,
        None => c.to_ascii_lowercase(),
    }
}
```

- [ ] **Step 4: Route the boundary check and the literal comparison through it**

Replace the body of `is_boundary_char`:

```rust
fn is_boundary_char(c: char, ignore_case: bool) -> bool {
    if ignore_case {
        fold_char(c).is_ascii_alphanumeric()
    } else {
        c.is_ascii_digit() || c.is_ascii_lowercase()
    }
}
```

The `ignore_case == false` arm is deliberately untouched: without `IGNORECASE`, Python's
`[a-z0-9]` matches only ASCII lowercase and digits, and `ſ` is not in it.

Replace the `eq` closure inside `find_term_match` — this also removes the `#[allow]`
added in Task 2:

```rust
    let eq = |a: char, b: char| {
        if ignore_case {
            fold_char(a) == fold_char(b)
        } else {
            a == b
        }
    };
```

- [ ] **Step 5: Widen the two negation character classes**

The `regex` crate's `(?i)` already folds `ſ` and `K` into `[a-z]`; it does not fold
`İ` or `ı`. Add those two explicitly to both patterns so the class matches Python's.

```rust
static NEGATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    // `\x{130}\x{131}` are added to the class because Python's `[a-z-]` under
    // re.IGNORECASE matches them and the regex crate's Unicode simple folding does not.
    // The other two folded characters (U+017F, U+212A) need no entry: simple folding
    // already maps them onto `s` and `k` inside `[a-z]`.
    Regex::new(r"(?i)\b(?:no|without|avoid|not)\s+(?:[a-z\x{130}\x{131}-]+\s+){0,2}[a-z\x{130}\x{131}-]+").unwrap()
});

static PLACEMENT_NEGATION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:no|without|avoid)\s+(?:[a-z\x{130}\x{131}-]+\s+){0,2}[a-z\x{130}\x{131}-]+").unwrap()
});
```

Keep the existing explanatory comments above each pattern; only the pattern string and
the added note change.

- [ ] **Step 6: Add unit tests for the fold**

Add to `rust/src/checks.rs`'s `mod tests`:

```rust
    #[test]
    fn folded_characters_block_a_boundary_like_their_ascii_equivalents() {
        // Python's [a-z0-9] under IGNORECASE contains all four of these, so each one
        // abutting "4k" blocks the match exactly as an ASCII letter would.
        for t in ["\u{17f}4k", "\u{212a}4k", "\u{130}4k", "\u{131}4k", "4k\u{17f}"] {
            assert!(find_original_case(t, "4k").is_none(), "should be blocked: {t:?}");
        }
    }

    #[test]
    fn folded_characters_match_their_ascii_letter_as_a_literal() {
        // KELVIN SIGN stands in for "k", so "4<KELVIN>" is a 4k claim in Python and
        // must be one here. The returned text is the ORIGINAL casing from the prompt.
        assert_eq!(find_original_case("a shot in 4\u{212a}", "4k").as_deref(), Some("4\u{212a}"));
    }

    #[test]
    fn folding_does_not_leak_into_the_case_sensitive_scan() {
        // has_term is case-SENSITIVE (it mirrors _has_term, which has no IGNORECASE).
        // Without IGNORECASE, Python's [a-z0-9] does not contain these characters and
        // the literal does not match them either.
        assert!(!has_term("a shot in 4\u{212a}", "4k"));
        assert!(has_term("a shot in \u{17f}4k", "4k"));
    }
```

- [ ] **Step 7: Run the unit tests**

Run: `cargo test --release --bin higgsfield-lint`
Expected: all pass, including the three new tests.

- [ ] **Step 8: Run parity**

Run: `cargo test --release --test parity`
Expected: PASS. Every `unicode:*` case now agrees.

If `unicode:negation:*` still diverges, the residual difference is in `\b` or in the
regex crate's folding of a character not in the table — measure it the same way (feed the
single input to both binaries and diff) and either extend the class or record it in
§12.3. Do not edit the oracle.

- [ ] **Step 9: Record the closed divergence in §12.3**

`SPEC.md` §12.3 documents exactly one accepted divergence. Add, so the next reader knows
this class was found and closed rather than never considered:

```markdown
### 12.4 Closed divergence: Unicode case folding

Python's `re` treats U+0130, U+0131, U+017F and U+212A as ASCII letters under
`IGNORECASE` - both as members of `[a-z0-9]` and as literal matches for `i`, `i`, `s` and
`k`. The Rust port's hand-rolled boundary scan was ASCII-only and diverged in both roles;
the `regex` crate's Unicode simple folding covered U+017F and U+212A but not U+0130 or
U+0131. Closed on 2026-09-10 by `UNICODE_ASCII_FOLDS` in `rust/src/checks.rs` and by
adding `\x{130}\x{131}` to both negation classes. `unicode_fold_sweep` in
`rust/tests/parity.rs` covers all four characters in all three roles.

Unlike §12.3 this was a genuine port bug, so the fix went entirely into the Rust and the
oracle was not touched.
```

- [ ] **Step 10: Commit**

```bash
git add rust/src/checks.rs rust/tests/parity.rs SPEC.md
git commit -m "fix(rust): match Python's Unicode case folding in the boundary scan"
```

---

### Task 6: Findings log

`SPEC.md` §4 deleted the outcome log because block authority was never on offer. The
side effect is that nothing in this system can answer "is this advice ever right" or
"what is the false-positive rate" — the two questions that decide whether §5 should exist
at all. One append per call fixes that, and Task 7 turns it into answers.

Design constraints that shape the code below:

- **Every call is logged, including clean ones.** A log of only the firings has no
  denominator and cannot produce a rate.
- **No prompt text, ever.** Check ids, model names and booleans only. That keeps records
  small, keeps user content out of a file on disk, and keeps every line under the atomic
  append size.
- **One `write_all` of one line under 4096 bytes**, opened `O_APPEND`. POSIX makes an
  append atomic against other writers, so concurrent hook processes never clobber each
  other's records.
- **Every failure is swallowed.** §7 as amended in Task 4.

**Files:**
- Create: `rust/src/lib.rs`, `rust/src/log.rs`
- Modify: `Cargo.toml`, `rust/src/main.rs`, `rust/src/models.rs:22-45`, `rust/src/lint.rs:26`, `.gitignore`
- Test: `rust/tests/fail_silent.rs`, `rust/tests/parity.rs:57-60`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `higgsfield_lint::models::locate_repo_root() -> Option<PathBuf>`
  - `higgsfield_lint::lint::resolve_params(raw: Option<&Value>) -> Option<Value>` (now `pub`)
  - `higgsfield_lint::log::record(payload: &Value, tool_input: &Value, findings: &[Finding])`
  - Record schema, one JSON object per line:
    `{"ts": u64, "tool": String, "models": [String], "neg": bool, "batch": usize, "fired": [String]}`

- [ ] **Step 1: Add the library target**

Two binaries cannot share private modules, and Task 7 adds a second binary. Create
`rust/src/lib.rs`:

```rust
//! Library target. The hook binary (`rust/src/main.rs`) and the review binary
//! (`rust/src/bin/review.rs`) both build on these modules; neither may duplicate them.
//!
//! Nothing here reads stdin, writes stdout, or decides an exit code - those are the
//! binaries' jobs, and they have opposite obligations. The hook must be silent on every
//! path (SPEC 7); the review tool is allowed to talk.

pub mod checks;
pub mod lint;
pub mod log;
pub mod models;
```

Add to `Cargo.toml`, above `[[bin]]`:

```toml
[lib]
name = "higgsfield_lint"
path = "rust/src/lib.rs"
```

- [ ] **Step 2: Reduce main.rs to a bin over the lib**

Replace the three `mod` declarations at `rust/src/main.rs:4-6` with:

```rust
use higgsfield_lint::{lint, log};
```

`checks.rs` and `lint.rs` need no edits: their `crate::checks` / `crate::lint` paths now
resolve inside the library, which is where they live.

- [ ] **Step 3: Verify the refactor changed nothing**

Run: `cargo test --release`
Expected: 46 + 2 + 1 pass, exactly as the baseline. A pure restructuring must not move a
single test.

- [ ] **Step 4: Commit the refactor separately**

```bash
git add Cargo.toml rust/src/lib.rs rust/src/main.rs
git commit -m "refactor(rust): add a lib target so a second binary can share the modules"
```

- [ ] **Step 5: Generalise the repo-root locator**

`models.rs` walks up from `current_exe()` to find `data/negative_prompt_models.json`. The
log needs the same walk to find a file that does not exist yet, so the walk must return
the root. Replace `locate_data_file` with:

```rust
/// Locate the repository root at RUNTIME by walking up from the running executable until
/// a directory containing `data/negative_prompt_models.json` is found.
///
/// The data file is the anchor rather than the target: callers that need a file which
/// does not exist yet (`data/findings.jsonl` on a first run) need the directory, not the
/// file. See the note below on why this is resolved at run time and not baked in at
/// compile time.
///
/// Python's oracle resolves its `DATA_FILE` via `Path(__file__).resolve().parent.parent`
/// - relative to the script's own location on disk, evaluated every time it runs, so it
/// keeps working if the whole repo is moved. Baking the path in at Rust *compile* time
/// (e.g. via `env!("CARGO_MANIFEST_DIR")`) would defeat that. The binary is expected to
/// live under `<repo>/target/{debug,release}/higgsfield-lint` (or one level deeper under
/// `deps/` for `cargo test` binaries), so walking upward finds `<repo>` regardless of
/// which it is and regardless of where `<repo>` sits on disk.
pub fn locate_repo_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent();
    while let Some(d) = dir {
        if d.join("data").join("negative_prompt_models.json").is_file() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

/// Port of `load_models`: fail silent on any error - executable path unavailable, data
/// file not found by walking up from it, unreadable, malformed JSON, or JSON that isn't a
/// list of strings - by returning an empty set, exactly as the Python's bare
/// `except Exception: return frozenset()` does.
pub fn load_models() -> HashSet<String> {
    locate_repo_root()
        .and_then(|root| fs::read_to_string(root.join("data").join("negative_prompt_models.json")).ok())
        .and_then(|contents| serde_json::from_str::<Vec<String>>(&contents).ok())
        .map(|list| list.into_iter().collect())
        .unwrap_or_default()
}
```

- [ ] **Step 6: Expose `resolve_params`**

At `rust/src/lint.rs:26`, change `fn resolve_params` to `pub fn resolve_params`. The log
needs the same `anyOf: [object, string]` handling the lint uses; reimplementing it would
guarantee the two drift.

- [ ] **Step 7: Write the failing logging tests**

Each test writes to its own file via `HIGGSFIELD_LINT_LOG`. Cargo runs integration tests
in parallel threads, so tests that asserted on line-count deltas against the one real
`data/findings.jsonl` would race each other and pollute the developer's actual log. The
override removes both problems and lets each test assert exact contents instead of deltas.

Add to `rust/tests/fail_silent.rs`:

```rust
use std::path::PathBuf;

/// A private log file for one test, named after the test and this process.
fn temp_log(name: &str) -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("higgsfield-lint-test-{name}-{}.jsonl", std::process::id()));
    let _ = std::fs::remove_file(&p);
    p
}

/// Run with logging ON, directed at `log`, whatever the harness default is.
fn run_logging(input: &[u8], log: &PathBuf) -> (i32, Vec<u8>, Vec<u8>) {
    let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
    let mut child = Command::new(exe)
        .env_remove("HIGGSFIELD_LINT_NO_LOG")
        .env("HIGGSFIELD_LINT_LOG", log)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().expect("spawn");
    child.stdin.as_mut().unwrap().write_all(input).ok();
    let out = child.wait_with_output().expect("wait");
    (out.status.code().unwrap_or(-1), out.stdout, out.stderr)
}

fn lines_of(path: &PathBuf) -> Vec<String> {
    std::fs::read_to_string(path)
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

#[test]
fn a_clean_call_is_logged_too() {
    // The denominator is the entire point: a log of only the firings cannot produce a
    // rate. A call with no findings must still leave a record, with `fired` empty.
    let log = temp_log("clean");
    let (code, out, err) = run_logging(
        br#"{"tool_name":"mcp__higgsfield__generate_video","tool_input":{"params":{"prompt":"a cat on a windowsill","width":100,"height":100}}}"#,
        &log,
    );
    assert_eq!(code, 0);
    assert!(err.is_empty());
    assert!(out.is_empty(), "a clean call must produce no stdout");

    let lines = lines_of(&log);
    assert_eq!(lines.len(), 1, "a clean call must still be logged");
    let rec: serde_json::Value = serde_json::from_str(&lines[0]).expect("valid JSON record");
    assert_eq!(rec["fired"], serde_json::json!([]));
    assert_eq!(rec["tool"], "mcp__higgsfield__generate_video");
    assert_eq!(rec["neg"], false);
    let _ = std::fs::remove_file(&log);
}

#[test]
fn a_record_never_contains_prompt_text() {
    // SPEC 9.1: metadata only. This is the test that fails loudly if someone later adds
    // a "prompt" field for debugging convenience.
    let log = temp_log("noprompt");
    let (_code, _out, _err) = run_logging(
        br#"{"tool_name":"mcp__higgsfield__generate_video","tool_input":{"params":{"model":"kling2_6","prompt":"a very distinctive canary phrase","negative_prompt":"blurry","width":1280,"height":720}}}"#,
        &log,
    );
    let lines = lines_of(&log);
    assert_eq!(lines.len(), 1);
    assert!(
        !lines[0].contains("canary"),
        "prompt text leaked into the log: {}",
        lines[0]
    );
    // The metadata that IS allowed must still be there.
    let rec: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
    assert_eq!(rec["models"], serde_json::json!(["kling2_6"]));
    assert_eq!(rec["neg"], true);
    let _ = std::fs::remove_file(&log);
}

#[test]
fn the_kill_switch_suppresses_the_append() {
    let log = temp_log("killswitch");
    let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
    let mut child = Command::new(exe)
        .env("HIGGSFIELD_LINT_NO_LOG", "1")
        .env("HIGGSFIELD_LINT_LOG", &log)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn().expect("spawn");
    child.stdin.as_mut().unwrap()
        .write_all(br#"{"tool_name":"mcp__higgsfield__generate_video","tool_input":{"params":{"prompt":"shot in 4k","width":1280,"height":720}}}"#)
        .ok();
    let out = child.wait_with_output().expect("wait");
    assert_eq!(out.status.code(), Some(0));
    assert!(out.stderr.is_empty());
    assert!(!out.stdout.is_empty(), "the finding itself must still be emitted");
    assert!(lines_of(&log).is_empty(), "HIGGSFIELD_LINT_NO_LOG must suppress the write");
}

#[test]
fn concurrent_appends_never_interleave() {
    // The atomicity claim is structural: O_APPEND plus a single write_all of a line
    // POSIX makes the seek-to-end and the write atomic against other writers, so
    // appends never clobber each other. A short write could in principle still split a
    // line; it does not happen for small writes to a local filesystem (measured: 40
    // concurrent appenders, 40/40 well-formed). This is the test that would catch it if
    // that ever stopped being true.
    let log = temp_log("concurrent");
    let input = br#"{"tool_name":"mcp__higgsfield__generate_video","tool_input":{"params":{"model":"kling2_6","prompt":"cinematic 4k, no logos","negative_prompt":"blurry","width":1280,"height":720}}}"#;
    let mut kids = Vec::new();
    for _ in 0..20 {
        let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
        let mut child = Command::new(exe)
            .env_remove("HIGGSFIELD_LINT_NO_LOG")
            .env("HIGGSFIELD_LINT_LOG", &log)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
            .spawn().expect("spawn");
        child.stdin.as_mut().unwrap().write_all(input).ok();
        kids.push(child);
    }
    for k in kids {
        let out = k.wait_with_output().expect("wait");
        assert_eq!(out.status.code(), Some(0));
        assert!(out.stderr.is_empty());
    }
    let lines = lines_of(&log);
    assert_eq!(lines.len(), 20, "every process must have appended exactly one line");
    for (i, line) in lines.iter().enumerate() {
        serde_json::from_str::<serde_json::Value>(line)
            .unwrap_or_else(|e| panic!("line {i} is not valid JSON ({e}): {line}"));
    }
    let _ = std::fs::remove_file(&log);
}
```

Also set the kill-switch on the existing pathological harness so its corpus writes
nothing. In `rust/tests/fail_silent.rs`'s original `run`, add
`.env("HIGGSFIELD_LINT_NO_LOG", "1")` to the `Command`.

- [ ] **Step 8: Set the kill-switch in the parity harness**

`rust/tests/parity.rs:57-60`. The harness spawns the binary ~320 times; without this it
pollutes the log with synthetic traffic that Task 7 would then report as real usage.

```rust
fn run_rust(input: &[u8]) -> Outcome {
    let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
    let mut cmd = Command::new(exe);
    // The harness is not usage. Without this, ~320 synthetic calls per test run land in
    // data/findings.jsonl and every rate the review tool reports becomes fiction.
    cmd.env("HIGGSFIELD_LINT_NO_LOG", "1");
    run(cmd, input)
}
```

- [ ] **Step 9: Run the tests to verify they fail**

Run: `cargo test --release --test fail_silent`
Expected: FAIL — `higgsfield_lint::log` does not exist yet.

- [ ] **Step 10: Write the log module**

Create `rust/src/log.rs`:

```rust
//! One JSONL record per linted call, appended best-effort. SPEC 9.1.
//!
//! This exists because SPEC 4 deleted the outcome log, which left the project unable to
//! answer the only two questions that decide whether the checks in 5 should exist: is
//! the advice ever right, and how often does it fire on a good call. Both are queries
//! over this file.
//!
//! Three properties are load-bearing:
//!
//! - **Metadata only.** Check ids, model names and booleans. No prompt text ever reaches
//!   this file - that keeps user content off disk and keeps every record far under the
//!   atomic-append budget below.
//! - **Every call, not every finding.** A clean call is logged with an empty `fired`.
//!   Without the clean calls there is no denominator and no rate.
//! - **Silent on every failure.** SPEC 7. An unlocatable root, an unwritable path, a
//!   serialisation failure and an oversized record all return quietly. The lint's stdout
//!   is identical whether the append happened or not.

use serde_json::Value;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use crate::lint::{resolve_params, Finding};
use crate::models::locate_repo_root;

/// Records are metadata only and run to a couple of hundred bytes; anything larger is a
/// bug upstream and is dropped rather than written.
///
/// Note on atomicity, because the obvious citation is wrong: `PIPE_BUF` governs pipes and
/// FIFOs, not regular files, and it is 512 on macOS rather than 4096. What POSIX actually
/// guarantees for `O_APPEND` on a regular file is that the seek-to-end and the write
/// happen atomically with respect to other writers - so concurrent hook processes never
/// clobber each other. A short write could in principle still split a line; for a
/// sub-kilobyte write to a local filesystem it does not (measured: 40 concurrent
/// appenders, 40/40 well-formed lines). This cap keeps records far inside that regime.
const MAX_RECORD_BYTES: usize = 4000;

/// Set `HIGGSFIELD_LINT_NO_LOG` to any value to suppress the append. The test harnesses
/// set it: ~320 synthetic parity calls per run would otherwise be indistinguishable from
/// real usage in every rate the review tool reports.
fn logging_disabled() -> bool {
    std::env::var_os("HIGGSFIELD_LINT_NO_LOG").is_some()
}

/// Where the record goes. `HIGGSFIELD_LINT_LOG` overrides the default so each test can
/// own a private file: cargo runs integration tests in parallel threads, and tests that
/// shared the one real log would race each other and pollute it. Returns `None` when the
/// repo root cannot be found and no override is set - one more silent no-op.
fn log_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("HIGGSFIELD_LINT_LOG") {
        return Some(PathBuf::from(p));
    }
    Some(locate_repo_root()?.join("data").join("findings.jsonl"))
}

/// Every `params` object this call will actually lint, in either shape. Mirrors `lint`'s
/// batch-before-top-level precedence closely enough for counting; it is not part of the
/// lint's output and is not held to parity.
fn linted_params(tool_input: &Value) -> Vec<Value> {
    let mut out = Vec::new();
    if let Some(Value::Array(requests)) = tool_input.get("requests") {
        for entry in requests {
            if let Some(p) = entry
                .as_object()
                .and_then(|e| resolve_params(e.get("params")))
            {
                out.push(p);
            }
        }
        if !requests.is_empty() {
            return out;
        }
    }
    if let Some(p) = resolve_params(tool_input.get("params")) {
        out.push(p);
    }
    out
}

/// Append one record. Returns nothing: there is no failure a caller could act on, and
/// SPEC 7 forbids reporting one.
pub fn record(payload: &Value, tool_input: &Value, findings: &[Finding]) {
    if logging_disabled() {
        return;
    }
    let Some(path) = log_path() else {
        return;
    };

    let params = linted_params(tool_input);
    let mut models: Vec<&str> = Vec::new();
    let mut neg = false;
    for p in &params {
        if let Some(m) = p.get("model").and_then(Value::as_str) {
            if !models.contains(&m) {
                models.push(m);
            }
        }
        if p.get("negative_prompt")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.trim().is_empty())
        {
            neg = true;
        }
    }

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let fired: Vec<&str> = findings.iter().map(|f| f.check_id.as_str()).collect();

    let rec = serde_json::json!({
        "ts": ts,
        "tool": payload.get("tool_name").and_then(Value::as_str).unwrap_or(""),
        "models": models,
        "neg": neg,
        "batch": tool_input.get("requests").and_then(Value::as_array).map_or(0, |r| r.len()),
        "fired": fired,
    });

    let Ok(mut line) = serde_json::to_string(&rec) else {
        return;
    };
    if line.len() >= MAX_RECORD_BYTES {
        return;
    }
    line.push('\n');

    let _ = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| f.write_all(line.as_bytes()));
}
```

- [ ] **Step 11: Call it from the hook**

In `rust/src/main.rs`'s `run()`, after the `lint::lint` call and before the emptiness
check:

```rust
    let models = lint::load_models();
    let findings = lint::lint(tool_input, &models);
    // Logged before the early return, so clean calls are recorded too - they are the
    // denominator for every rate in `higgsfield-lint-review`.
    log::record(&payload, tool_input, &findings);
    if findings.is_empty() {
        return None;
    }
```

`run()` currently binds `tool_input` from `payload.get("tool_input")?`, which borrows
`payload`; both are still in scope here, so no restructuring is needed.

- [ ] **Step 12: Ignore the log file**

Append to `.gitignore`:

```
# Findings log - local evidence, not source. See SPEC 9.1.
data/findings.jsonl
```

- [ ] **Step 13: Run the tests**

Run: `cargo test --release`
Expected: all pass, including the three new `fail_silent` tests. Then confirm the parity
harness left no trace:

```bash
git status --porcelain data/
```

Expected: empty — `data/findings.jsonl` is ignored, and the harness wrote nothing anyway.

- [ ] **Step 14: Verify by hand that stdout is unaffected**

```bash
rm -f data/findings.jsonl
printf '%s' '{"tool_name":"mcp__higgsfield__generate_video","tool_input":{"params":{"prompt":"cinematic 4K","width":1376,"height":768}}}' | ./target/release/higgsfield-lint
echo
cat data/findings.jsonl
```

Expected: the finding on stdout exactly as before, and one record with
`"fired":["resolution_claim"]`.

- [ ] **Step 15: Document the log in SPEC §9**

Add a subsection after §9:

```markdown
### 9.1 Findings log

The hook appends one JSON object per call to `data/findings.jsonl` (gitignored). Metadata
only - no prompt text ever reaches this file:

    {"ts":1757462400,"tool":"mcp__higgsfield__generate_video","models":["kling2_6"],
     "neg":true,"batch":0,"fired":["resolution_claim","negation_placement"]}

Clean calls are logged with `"fired":[]`. That is deliberate: a log of only the firings
has no denominator, and every question worth asking of this file is a rate.

The append is best-effort and silent on every failure (7 as amended). It is a single
`write_all` of one line under 4096 bytes to an `O_APPEND` handle, so concurrent hook
processes cannot interleave.

Two environment variables control it, both for testing only. `HIGGSFIELD_LINT_NO_LOG`
suppresses the append entirely; both test harnesses set it, because ~320 synthetic parity
calls per run would otherwise be indistinguishable from real usage. `HIGGSFIELD_LINT_LOG`
redirects the append to a given path, so a test can own a private file rather than racing
other tests over the real one.

`higgsfield-lint-review` (9.2) is the only reader.
```

- [ ] **Step 16: Commit**

```bash
git add rust/src/log.rs rust/src/main.rs rust/src/models.rs rust/src/lint.rs \
        rust/tests/fail_silent.rs rust/tests/parity.rs .gitignore SPEC.md
git commit -m "feat(rust): append one metadata record per call to data/findings.jsonl"
```

---

### Task 7: The review tool

`data/negative_prompt_models.json` contains exactly one entry and nothing grows it.
`rust/src/checks.rs:358-359` gates `check_negation_placement` on that file or on a
`negative_prompt` the caller already supplied — so the check can tidy a field you already
used, but can never tell you to use one. Confirmed: `seedance_2_5` with
`"no logos, no text overlays"` and no `negative_prompt` is silent.

The fix is **not** to have the hook mutate that file. A read-modify-write from inside a
process that must never fail visibly, racing other instances of itself, is the wrong
shape — and SPEC §5.3 is explicit that the list is *evidence-seeded, not inferred*. So the
hook only observes (Task 6), and promotion stays a deliberate human edit informed by a
report.

It reports fire **rates**, not precision. Precision needs a human judging each firing
against the prompt that caused it, and this log deliberately holds no prompt text - so
Task 9 gets precision by joining the log's `ts` against `show_generations`, which is the
same method SPEC 2 used to derive all three checks in the first place. Do not let the
rate be mistaken for a precision number.

**Files:**
- Create: `rust/src/bin/review.rs`
- Modify: `Cargo.toml`, `SPEC.md` §9, §10, `README.md`

**Interfaces:**
- Consumes: `models::locate_repo_root`, the §9.1 record schema.
- Produces: the `higgsfield-lint-review` binary. Nothing depends on it in code.

- [ ] **Step 1: Register the second binary**

Add to `Cargo.toml` after the existing `[[bin]]`:

```toml
[[bin]]
name = "higgsfield-lint-review"
path = "rust/src/bin/review.rs"
```

- [ ] **Step 2: Write the review tool**

Create `rust/src/bin/review.rs`:

```rust
//! `higgsfield-lint-review` - the only reader of `data/findings.jsonl` (SPEC 9.1).
//!
//! This binary is the opposite of the hook in every way that matters: it is never on the
//! generation path, it is allowed to print, and it may exit non-zero. It never writes.
//!
//! It answers two questions the project could not previously ask:
//!
//! 1. **Is the advice worth reading?** Per-check fire rates over real traffic. SPEC 9
//!    states that a check firing on a known-good call is a bug; this is the instrument
//!    that finds those.
//! 2. **Which models actually accept a negative_prompt?** SPEC 5.3 requires that list to
//!    be evidence-seeded rather than guessed. Every call carrying a non-empty
//!    negative_prompt is evidence. This reports the candidates and prints the exact line
//!    to add - it does not edit the file, because promotion is a judgement and a commit,
//!    not a side effect of a hook run.

use serde_json::Value;
use std::collections::BTreeMap;

use higgsfield_lint::models::locate_repo_root;

/// A model needs this many distinct calls carrying a non-empty `negative_prompt` before
/// it is worth proposing. Five is a judgement call, not a measurement: low enough to be
/// reachable at this volume (SPEC 1 counts nine lifetime video calls), high enough that
/// one experiment does not rewrite the lexicon. Raise it if a promotion turns out wrong.
const PROMOTION_THRESHOLD: usize = 5;

fn main() {
    let Some(root) = locate_repo_root() else {
        eprintln!("higgsfield-lint-review: could not locate the repo root from the running binary");
        std::process::exit(1);
    };
    let log = root.join("data").join("findings.jsonl");
    let Ok(text) = std::fs::read_to_string(&log) else {
        println!("No findings log at {}.", log.display());
        println!("The hook writes one on its first Higgsfield generate_* call.");
        return;
    };

    let mut total = 0usize;
    let mut with_findings = 0usize;
    let mut per_check: BTreeMap<String, usize> = BTreeMap::new();
    let mut neg_calls: BTreeMap<String, usize> = BTreeMap::new();
    let mut first_ts = u64::MAX;
    let mut last_ts = 0u64;
    let mut malformed = 0usize;

    for line in text.lines() {
        let Ok(rec) = serde_json::from_str::<Value>(line) else {
            malformed += 1;
            continue;
        };
        total += 1;
        if let Some(ts) = rec.get("ts").and_then(Value::as_u64) {
            first_ts = first_ts.min(ts);
            last_ts = last_ts.max(ts);
        }
        let fired: Vec<&str> = rec
            .get("fired")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        if !fired.is_empty() {
            with_findings += 1;
        }
        for id in fired {
            *per_check.entry(id.to_string()).or_insert(0) += 1;
        }
        if rec.get("neg").and_then(Value::as_bool) == Some(true) {
            if let Some(models) = rec.get("models").and_then(Value::as_array) {
                for m in models.iter().filter_map(Value::as_str) {
                    *neg_calls.entry(m.to_string()).or_insert(0) += 1;
                }
            }
        }
    }

    let pct = |n: usize| if total == 0 { 0.0 } else { n as f64 * 100.0 / total as f64 };

    println!("higgsfield-lint review - {}", log.display());
    println!();
    println!("calls logged:           {total}");
    if total > 0 {
        println!("  span:                 {first_ts} .. {last_ts} (unix seconds)");
    }
    if malformed > 0 {
        println!("  malformed lines:      {malformed}  (skipped)");
    }
    println!("calls with >=1 finding: {with_findings}  ({:.1}%)", pct(with_findings));
    println!();
    println!("per-check fire rate");
    if per_check.is_empty() {
        println!("  (nothing has fired yet)");
    }
    for (id, n) in &per_check {
        println!("  {id:<26} {n:>5}  ({:.1}%)", pct(*n));
    }
    println!();

    let listed = higgsfield_lint::models::load_models();
    let mut candidates: Vec<(&String, &usize)> = neg_calls
        .iter()
        .filter(|(m, n)| **n >= PROMOTION_THRESHOLD && !listed.contains(*m))
        .collect();
    candidates.sort_by(|a, b| b.1.cmp(a.1));

    println!("promotion candidates for data/negative_prompt_models.json");
    println!(
        "  (models seen with a non-empty negative_prompt on >= {PROMOTION_THRESHOLD} calls, not already listed)"
    );
    if candidates.is_empty() {
        println!("  (none)");
    }
    for (model, n) in &candidates {
        println!("  {model:<26} {n:>5} calls");
    }
    if !candidates.is_empty() {
        let mut all: Vec<String> = listed.into_iter().collect();
        all.extend(candidates.iter().map(|(m, _)| (*m).clone()));
        all.sort();
        println!();
        println!("  To promote, edit data/negative_prompt_models.json to:");
        println!("  {}", serde_json::to_string(&all).unwrap_or_default());
        println!("  Then re-run `cargo test --release` and commit the change with the");
        println!("  evidence above in the commit message (SPEC 5.3: evidence-seeded).");
    }
}
```

- [ ] **Step 3: Build and verify it runs against an empty log**

```bash
rm -f data/findings.jsonl
cargo build --release
./target/release/higgsfield-lint-review
```

Expected: "No findings log at ..." and exit 0. A review tool that panics on a fresh
checkout will never be run twice.

- [ ] **Step 4: Verify against synthetic traffic**

```bash
rm -f data/findings.jsonl
# five clean seedance calls carrying a negative_prompt - a promotion candidate
for i in 1 2 3 4 5; do
  printf '%s' '{"tool_name":"mcp__higgsfield__generate_video","tool_input":{"params":{"model":"seedance_2_5","prompt":"a calm street","negative_prompt":"blurry","width":1920,"height":1080}}}' \
    | ./target/release/higgsfield-lint
done
# one call that fires two checks
printf '%s' '{"tool_name":"mcp__higgsfield__generate_video","tool_input":{"params":{"model":"kling2_6","prompt":"cinematic 4K, no logos","negative_prompt":"blurry","width":1376,"height":768}}}' \
  | ./target/release/higgsfield-lint > /dev/null
./target/release/higgsfield-lint-review
```

Expected: 6 calls logged, 1 with findings (16.7%), `resolution_claim` and
`negation_placement` at 1 each, and `seedance_2_5` listed as a promotion candidate with 5
calls, followed by the exact JSON line to paste.

- [ ] **Step 5: Clean up the synthetic log**

```bash
rm -f data/findings.jsonl
```

The file is gitignored, but leaving synthetic traffic in it would corrupt the first real
review.

- [ ] **Step 6: Verify the whole suite still passes**

Run: `cargo test --release && cargo clippy --all-targets --release -- -D warnings && cargo fmt --check`
Expected: all exit 0.

- [ ] **Step 7: Document it**

Add to `SPEC.md` after §9.1:

```markdown
### 9.2 Review tool

`higgsfield-lint-review` is the only reader of the findings log. It is read-only, is never
on the generation path, and may print and exit non-zero - the opposite of the hook's
obligations in 7.

    cargo build --release && ./target/release/higgsfield-lint-review

It reports per-check fire rates over real traffic, which is the instrument 9 asks for
("a check that fires on a known-good call is a bug"), and lists models observed with a
non-empty negative_prompt on at least five calls that are not yet in
`data/negative_prompt_models.json`.

It never edits that file. 5.3 requires the list to be evidence-seeded rather than
inferred, so promotion stays a human judgement and a commit; the tool supplies the
evidence and prints the exact line to paste.
```

Add to §10's repository layout, in the existing style:

```
rust/tests/behaviour.rs              the three bounded CI gates
rust/src/lib.rs                      library target shared by both binaries
rust/src/log.rs                      the findings log (9.1)
rust/src/bin/review.rs               higgsfield-lint-review (9.2)
scripts/check-hook-freshness.sh      SessionStart staleness guard (12.1)
data/findings.jsonl                  local findings log, gitignored (9.1)
requirements-dev.txt                 pytest, for tests/test_lint.py
.github/workflows/ci.yml             build, both test suites, clippy, fmt
```

Add to `README.md` under a new heading after "Running the tests":

````markdown
## Reviewing what the lint has actually done

```
cargo build --release && ./target/release/higgsfield-lint-review
```

The hook appends one metadata record per call to `data/findings.jsonl` - no prompt text,
clean calls included. This prints per-check fire rates over that log and lists models that
have been seen accepting a `negative_prompt` but are not yet in
`data/negative_prompt_models.json`. It never writes; promoting a model is a deliberate
edit and a commit. See SPEC.md §9.1 and §9.2.
````

- [ ] **Step 8: Commit**

```bash
git add Cargo.toml rust/src/bin/review.rs SPEC.md README.md
git commit -m "feat: add higgsfield-lint-review for fire rates and promotion candidates"
```

---

### Task 8: Tag-segment gate for the conflict check

`rust/src/checks.rs:255-270` lists bare tokens. Measured against the shipped binary and
the oracle, all three of these fire and all three are wrong:

| prompt | negative_prompt | fires |
|---|---|---|
| `a cast iron pan of sizzling garlic, a woman reading a text message` | `pan, text, blurry` | `"text", "pan"` |
| `a field of grain at golden hour` | `grain, noise` | `"grain"` |
| `a static caravan in a field` | `static, jitter` | `"static"` |

`SPEC.md:71` §4 names the user's attention as the only scarce resource. This check spends
it on cookware.

**Do not prune the phrase list.** The distinguishing feature is not the word, it is how it
is used. Real prompts are comma-delimited tag soup — the kling fixture's positive prompt
carries `slow motion` as its own two-word tag — while every false positive above has the
term buried in a prose clause of six or seven words. So: on the positive side only, a term
counts when it occupies a short comma-delimited segment. The negative side is unchanged,
because a `negative_prompt` is a tag list by construction.

This trades recall for precision (a genuine directive written as prose is now missed).
That is the trade `SPEC.md` §9 already asks for: "a check that fires on a known-good call
is a bug, and the whole value of an advisory tool is that its advice is worth reading."

This is a deliberate behaviour change, so it follows Task 4's §12.2 order exactly: spec,
then oracle, then Rust, then parity.

**Files:**
- Modify: `SPEC.md` §5.2, then `src/higgsfield_lint.py:157-172`, then `rust/src/checks.rs:319-351`
- Test: `tests/test_lint.py`, `rust/src/checks.rs` unit tests, `rust/tests/parity.rs`

**Interfaces:**
- Consumes: `has_term` / `_has_term`.
- Produces:
  - Python: `_appears_as_tag(text: str, term: str) -> bool`, `_MAX_TAG_WORDS = 4`
  - Rust: `fn appears_as_tag(text: &str, term: &str) -> bool`, `const MAX_TAG_WORDS: usize = 4`

- [ ] **Step 1: Amend SPEC §5.2 first**

Per §12.2 as amended in Task 4, the spec moves before either implementation. Replace §5.2's
body with:

```markdown
Fires when a phrase from a curated directive list appears in both fields. Restricted to a
phrase list rather than free tokens, because common words in both fields are normal and
would make this unusable.

Seed list: `slow motion`, `slow-mo`, `static`, `blurred`, `blurry`, `sharp`, `text`,
`motion blur`, `close-up`, `wide shot`, `zoom`, `pan`, `handheld`, `grain`.

**The positive side must read as a tag, not as prose.** Half that list is a homograph:
`pan` is cookware, `grain` is a crop, `text` is a message, `static` is a caravan. Measured
false positives on all four. The discriminator is not the word but its use - real prompts
are comma-delimited tag soup, and a directive occupies its own short segment, while a
homograph sits inside a clause. So a term on the positive side counts only when it appears
within a comma-, period-, semicolon- or newline-delimited segment of at most four words.
Negations are stripped before segmentation, as before.

The negative side is unchanged: a `negative_prompt` is a tag list by construction, so no
segment rule applies to it.

Four words, not three or five: `slow motion` is two and `extreme close-up shot` is three,
while the measured false positives ran six and seven. The consequence is a deliberate
false negative - a directive written out in prose is no longer caught - which is the trade
9 asks for.
```

- [ ] **Step 2: Write the failing Python tests**

Add to `tests/test_lint.py`:

```python
def test_homograph_in_prose_does_not_conflict():
    """Measured false positives, 2026-09-10. Each has the term buried in a prose clause
    on the positive side while the negative_prompt uses it as a directive."""
    for prompt, negative in [
        ("a cast iron pan of sizzling garlic, a woman reading a text message", "pan, text, blurry"),
        ("a field of grain at golden hour", "grain, noise"),
        ("a static caravan in a field", "static, jitter"),
    ]:
        params = {"prompt": prompt, "negative_prompt": negative, "width": 1280, "height": 720}
        assert hl.check_prompt_negative_conflict(params, frozenset()) is None, prompt


def test_directive_as_a_tag_still_conflicts():
    """The real kling2_6 fixture behaviour must survive: "slow motion" is its own
    two-word comma-delimited tag on both sides."""
    params = {
        "prompt": "cinematic, ultra-clean commercial style, slow motion, warm highlights",
        "negative_prompt": "transitions, slow motion, bad quality",
        "width": 1376,
        "height": 768,
    }
    finding = hl.check_prompt_negative_conflict(params, frozenset())
    assert finding is not None
    assert '"slow motion"' in finding.message


def test_tag_segment_boundary_is_four_words():
    long_tag = {"prompt": "a very wide shot of a valley", "negative_prompt": "wide shot", "width": 1280, "height": 720}
    short_tag = {"prompt": "a valley, wide shot, dawn", "negative_prompt": "wide shot", "width": 1280, "height": 720}
    assert hl.check_prompt_negative_conflict(long_tag, frozenset()) is None
    assert hl.check_prompt_negative_conflict(short_tag, frozenset()) is not None
```

- [ ] **Step 3: Run them to verify they fail**

Run: `pytest tests/test_lint.py -q -k "homograph or tag"`
Expected: FAIL — the three homograph cases currently produce findings.

- [ ] **Step 4: Change the oracle**

In `src/higgsfield_lint.py`, add above `check_prompt_negative_conflict`:

```python
# A directive occupies its own short comma-delimited tag; a homograph sits inside a prose
# clause. Four words admits "slow motion" (2) and "extreme close-up shot" (3) while
# excluding the measured false positives, which ran six and seven. See SPEC 5.2.
_MAX_TAG_WORDS = 4


def _appears_as_tag(text: str, term: str) -> bool:
    """True when `term` occurs inside a short delimited segment - used as a directive tag
    rather than as a noun inside a clause."""
    for segment in re.split(r"[,.;\n]", text):
        words = segment.split()
        if len(words) <= _MAX_TAG_WORDS and _has_term(" ".join(words), term):
            return True
    return False
```

Then change the `hits` line inside `check_prompt_negative_conflict` (line 164) to:

```python
    hits = [p for p in CONFLICT_PHRASES if _appears_as_tag(positive, p) and _has_term(negative_low, p)]
```

- [ ] **Step 5: Run the Python tests**

Run: `pytest tests/ -q`
Expected: all pass — the three new tests plus the existing 58. If a pre-existing test now
fails, it encoded the old behaviour; check it against amended §5.2 before touching it.

- [ ] **Step 6: Run parity to watch it diverge**

Run: `cargo test --release --test parity`
Expected: FAIL. This is the correct intermediate state — the oracle has moved and the Rust
has not. Do not skip this step: it is the only proof that the parity harness would have
caught an un-ported change.

- [ ] **Step 7: Write the failing Rust unit tests**

Add to `rust/src/checks.rs`'s `mod tests`:

```rust
    #[test]
    fn homograph_in_prose_does_not_conflict() {
        for (prompt, negative) in [
            ("a cast iron pan of sizzling garlic, a woman reading a text message", "pan, text, blurry"),
            ("a field of grain at golden hour", "grain, noise"),
            ("a static caravan in a field", "static, jitter"),
        ] {
            let params = json!({"prompt": prompt, "negative_prompt": negative, "width": 1280, "height": 720});
            assert!(
                check_prompt_negative_conflict(&params).is_none(),
                "should not fire: {prompt}"
            );
        }
    }

    #[test]
    fn directive_as_a_tag_still_conflicts() {
        let params = json!({
            "prompt": "cinematic, ultra-clean commercial style, slow motion, warm highlights",
            "negative_prompt": "transitions, slow motion, bad quality",
            "width": 1376, "height": 768
        });
        let finding = check_prompt_negative_conflict(&params).expect("should fire");
        assert!(finding.message.contains("\"slow motion\""), "got: {}", finding.message);
    }

    #[test]
    fn tag_segment_boundary_is_four_words() {
        let long_tag = json!({"prompt": "a very wide shot of a valley", "negative_prompt": "wide shot", "width": 1280, "height": 720});
        let short_tag = json!({"prompt": "a valley, wide shot, dawn", "negative_prompt": "wide shot", "width": 1280, "height": 720});
        assert!(check_prompt_negative_conflict(&long_tag).is_none());
        assert!(check_prompt_negative_conflict(&short_tag).is_some());
    }
```

Then add the same three cases to gate 3 in `rust/tests/behaviour.rs`, which is where
"inaccurate advice" is contractually gated — a unit test alone would leave the CI gate
silent about the exact class of defect this task exists to fix:

```rust
        // Homographs: the term is a directive in the negative_prompt and an ordinary noun
        // in the positive prompt. Measured false positives, 2026-09-10, fixed by the
        // tag-segment rule in SPEC 5.2.
        (
            "homograph: pan and text in prose",
            r#"{"params":{"prompt":"a cast iron pan of sizzling garlic, a woman reading a text message","negative_prompt":"pan, text, blurry","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "homograph: grain in prose",
            r#"{"params":{"prompt":"a field of grain at golden hour","negative_prompt":"grain, noise","width":1280,"height":720}}"#,
            &[],
        ),
        (
            "homograph: static in prose",
            r#"{"params":{"prompt":"a static caravan in a field","negative_prompt":"static, jitter","width":1280,"height":720}}"#,
            &[],
        ),
        // The other half of the contract: the same term as its own short tag on both
        // sides is still a real conflict and must still fire.
        (
            "directive as a tag still conflicts",
            r#"{"params":{"prompt":"a valley, slow motion, dawn","negative_prompt":"transitions, slow motion","width":1280,"height":720}}"#,
            &["prompt_negative_conflict"],
        ),
```

- [ ] **Step 8: Run them to verify they fail**

Run: `cargo test --release --bin higgsfield-lint homograph`
Expected: FAIL.

Run: `cargo test --release --test behaviour gate_3`
Expected: FAIL, naming the three homograph cases. The fourth case (the tag) passes
already — it is there to catch an over-correction.

- [ ] **Step 9: Port the change to Rust**

In `rust/src/checks.rs`, above `check_prompt_negative_conflict`, add:

```rust
// A directive occupies its own short comma-delimited tag; a homograph sits inside a prose
// clause. Four words admits "slow motion" (2) and "extreme close-up shot" (3) while
// excluding the measured false positives, which ran six and seven. See SPEC 5.2.
const MAX_TAG_WORDS: usize = 4;

/// Port of `_appears_as_tag`. True when `term` occurs inside a short delimited segment -
/// used as a directive tag rather than as a noun inside a clause.
///
/// Python splits on the character class `[,.;\n]` and joins each segment's whitespace-
/// separated words with single spaces before scanning; `split_whitespace` plus `join(" ")`
/// is the same normalisation, which matters because `has_term`'s boundary rule is
/// sensitive to the character immediately either side of the term.
fn appears_as_tag(text: &str, term: &str) -> bool {
    text.split([',', '.', ';', '\n']).any(|segment| {
        let words: Vec<&str> = segment.split_whitespace().collect();
        words.len() <= MAX_TAG_WORDS && has_term(&words.join(" "), term)
    })
}
```

Then change the `hits` filter inside `check_prompt_negative_conflict`:

```rust
    let hits: Vec<&str> = CONFLICT_PHRASES
        .iter()
        .filter(|p| appears_as_tag(&positive, p) && has_term(&negative_low, p))
        .copied()
        .collect();
```

- [ ] **Step 10: Run the Rust tests**

Run: `cargo test --release --bin higgsfield-lint`
Expected: all pass.

- [ ] **Step 11: Add parity coverage for the new rule**

Add this function to `rust/tests/parity.rs`:

```rust
/// The tag-segment rule (SPEC 5.2): the same term is a finding as a short tag and not a
/// finding inside a prose clause. Both sides must agree on the boundary, so this sweeps
/// segment lengths either side of MAX_TAG_WORDS for every conflict phrase.
fn tag_segment_sweep() -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    for phrase in CONFLICT_PHRASES {
        // As its own tag: must fire on both sides.
        out.push((
            format!("tag:bare:{phrase}"),
            wrap(TOOL_SINGLE, single(json!({
                "prompt": format!("a valley at dawn, {phrase}, warm light"),
                "negative_prompt": format!("{phrase}, bad quality"),
                "width": 1280, "height": 720
            }))),
        ));
        // Buried in a seven-word clause: must be silent on both sides.
        out.push((
            format!("tag:prose:{phrase}"),
            wrap(TOOL_SINGLE, single(json!({
                "prompt": format!("a quiet room where the {phrase} was left alone"),
                "negative_prompt": format!("{phrase}, bad quality"),
                "width": 1280, "height": 720
            }))),
        ));
        // Exactly at the boundary, four and five words.
        out.push((
            format!("tag:four_words:{phrase}"),
            wrap(TOOL_SINGLE, single(json!({
                "prompt": format!("a valley, one {phrase} here, dawn"),
                "negative_prompt": phrase.to_string(),
                "width": 1280, "height": 720
            }))),
        ));
        out.push((
            format!("tag:five_words:{phrase}"),
            wrap(TOOL_SINGLE, single(json!({
                "prompt": format!("a valley, one more {phrase} over here, dawn"),
                "negative_prompt": phrase.to_string(),
                "width": 1280, "height": 720
            }))),
        ));
    }
    out
}
```

Then register it in `rust_matches_python_oracle_across_the_corpus`, next to the sweep
added in Task 5:

```rust
    all.extend(extra_edge_sweep());
    all.extend(unicode_fold_sweep(&known));
    all.extend(tag_segment_sweep());
```

The harness asserts the de-duplicated corpus holds at least 300 distinct inputs
(`rust/tests/parity.rs:637`); both new sweeps only grow it.

- [ ] **Step 12: Run parity**

Run: `cargo test --release --test parity`
Expected: PASS. If it fails, the Rust is wrong — the oracle was changed from the spec in
Step 4 and is not to be touched again (§12.2 as amended).

- [ ] **Step 13: Confirm the real fixtures still behave**

```bash
cargo build --release
python3 -c "import json,sys; sys.stdout.write(json.dumps({'tool_input': json.load(open('tests/fixtures/kling2_6_4k_slowmo.json'))}))" \
  | ./target/release/higgsfield-lint
```

Expected: still three findings, including `prompt_negative_conflict` on `"slow motion"`.
`SPEC.md` §9 makes the six seedance negative fixtures matter more than the positives; run
the pytest fixture tests to confirm they are still silent:

```bash
pytest tests/ -q
```

- [ ] **Step 14: Full green**

Run: `cargo test --release && pytest tests/ -q && cargo clippy --all-targets --release -- -D warnings && cargo fmt --check`
Expected: all exit 0.

- [ ] **Step 15: Commit**

```bash
git add SPEC.md src/higgsfield_lint.py rust/src/checks.rs tests/test_lint.py rust/tests/parity.rs
git commit -m "fix: require a directive to read as a tag, not prose, before flagging a conflict"
```

The commit touches §5, the oracle and the Rust together, in that order — the shape
§12.2 requires for a deliberate behaviour change.

---

### Task 9: Precision review checkpoint

Not a code task. The findings log is worthless if nobody reads it, and every earlier task
is a hypothesis about what helps.

**Files:**
- Modify: `docs/credit-expiry-task.md` (or a new `docs/lint-review-task.md`)

**Interfaces:**
- Consumes: `higgsfield-lint-review` from Task 7.
- Produces: a decision.

- [ ] **Step 1: Schedule the checkpoint**

Using the same mechanism as Task 1, schedule a **one-time** run for 2026-10-08 (four weeks
out) with this prompt:

```
Run `cargo build --release && ./target/release/higgsfield-lint-review` in the
higgsfield-preprocessor repo and report its output verbatim. Then answer three questions
from that output alone, without speculating: (1) how many calls were logged, (2) which
checks fired and at what rate, (3) whether any model is a promotion candidate for
data/negative_prompt_models.json. If fewer than 20 calls were logged, say that the sample
is too small to conclude anything and recommend deferring another four weeks.
```

- [ ] **Step 2: Write the decision rule down now, before the data exists**

Create `docs/lint-review-task.md`:

```markdown
# Lint precision review

**Scheduled:** one-time, 2026-10-08. Instrument: `higgsfield-lint-review` (SPEC 9.2).

The rule is written before the data arrives, so the data cannot be argued with after the
fact.

- **Fewer than 20 calls logged.** Too small. Defer four weeks. At the volume in SPEC 1
  (nine lifetime video calls) this is the expected outcome, and it is itself a finding:
  a lint on traffic this thin is not earning its maintenance.
- **A check that fired on a call the human judges good.** That is a bug per SPEC 9, not a
  tuning opportunity. Fix or delete the check.
- **A check under roughly 70% precision on the calls reviewed.** Delete it. SPEC 4 makes
  attention the scarce resource; a check wrong a third of the time is spending it.

  Precision is not a number the review tool prints - it reports fire rates, and the log
  holds no prompt text. Get it the way SPEC 2 got its evidence: take the `ts` of each
  firing from `data/findings.jsonl`, pull the matching calls with `show_generations`, and
  judge each one by hand. At roughly 36 calls a month (SPEC 1) a month's firings is a
  short enough list to read.
- **A check that never fired.** Not evidence of anything at this volume. Leave it.
- **A promotion candidate.** Promote it by hand per SPEC 5.3, with the evidence in the
  commit message.

Deleting a check is a normal outcome of this review, not a failure. SPEC 13 already names
the conditions under which the larger design comes back; this is the equivalent gate in
the other direction.
```

- [ ] **Step 3: Commit**

```bash
git add docs/lint-review-task.md
git commit -m "docs: schedule the precision review and fix its decision rule in advance"
```

---

## Self-Review

**Spec coverage.** Every review finding maps to a task: P0-1 missing binary → Task 3;
P0-2 unbuilt §8 → Task 1; P1-3 homographs → Task 8; P1-4 dead model list → Tasks 6 and 7;
P1-5 Unicode divergence → Task 5; P2-6 contract drift → Task 4 Step 1; P2-7 CI → Task 2;
P2-9 observability → Tasks 6, 7 and 9.

**P2-8 is deliberately not scheduled.** `rust/src/checks.rs:202-212` short-circuits
width/height when `resolution` is recognised, so `resolution: "4k"` alongside
`width: 1376, height: 768` is silent. This is what §5.1 specifies ("Actual is
`params.resolution` when present, otherwise `min(width, height)`"), and changing it means
deciding which field Higgsfield actually honours when both are present — a question the
catalog does not answer and nine lifetime calls cannot settle. It stays a known blind spot
until the log in Task 6 shows a call that carries both and contradicts itself. Revisit at
the Task 9 checkpoint.

**Ordering.** Task 4 gates Tasks 6 and 8. Task 6 gates Task 7. Task 5 touches only the
Rust and needs no spec change. Tasks 1, 2 and 3 are independent and ship first because
Task 1 carries the most value per §1 and Tasks 2-3 protect everything after them.

**Corrections applied after the 2026-09-10 plan review.** Three statements in the first
draft were false and are fixed above; they are recorded here so the same mistakes are not
re-introduced:

1. The clippy lint was named `manual_case_insensitive_comparison`, which does not exist.
   The real name is `clippy::manual_ignore_case_cmp` (clippy 1.98.0). An unknown lint
   inside an `#[allow]` trips `unknown_lints`, so Task 2's own verification step would
   have failed on the fix it had just applied. Verified before re-writing.
2. The findings log justified its atomicity with `PIPE_BUF`. That is wrong twice:
   `PIPE_BUF` governs pipes and FIFOs rather than regular files, and it is 512 on macOS,
   not 4096. The behaviour is fine - measured at 40 concurrent appenders, 40/40
   well-formed lines - but the reason is `O_APPEND`'s ordering guarantee, not `PIPE_BUF`.
3. Task 9's decision rule consumed a precision figure that Task 7 cannot produce: the
   review tool reports fire rates, and the log deliberately holds no prompt text. Task 9
   now says how precision is actually obtained - joining the log's `ts` against
   `show_generations`, the same method SPEC 2 used to derive all three checks.

**CI is bounded, not exhaustive.** Task 2 gates on three named classes - bad prompts,
missing details, inaccurate or harmful advice - because those are the only three ways an
advisory lint becomes worth deleting. Parity, fail-silent, pytest, clippy and fmt still
run, but as supporting steps beneath those gates. Every gate case was measured against the
release binary before being written into the plan, so the file passes on creation and any
later failure is a real regression rather than an aspirational test.

**Risk.** The only irreversible-feeling step is Task 6's `[lib]` refactor, which is why it
is committed on its own after a full test run with zero expected diff in results. The only
step that cannot be verified from this session is Task 3 Step 7, which needs a real
session restart; it is marked as not-done-without-it rather than glossed.
