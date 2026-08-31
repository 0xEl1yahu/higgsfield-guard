# Hook Verification and Credit Expiry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make it possible to tell whether the hook is actually installed and working, and stop forfeiting ~1,150 credits a month to silent monthly expiry.

**Architecture:** Two independent deliverables sharing one branch. A `doctor` subcommand added to the existing Rust binary self-checks every link in the install chain and exits non-zero on failure (safe, because it is never the hook path). A separate scheduled Claude Code task calls the Higgsfield `balance` MCP tool weekly and warns when credits are about to expire, because a hook cannot call MCP tools.

**Tech Stack:** Rust (existing crate, stdlib + `serde_json` + `regex`). A scheduled Claude Code task for the MCP half.

**Spec:** `SPEC.md` (§8 amended by Task 4)

**Predecessor:** `docs/superpowers/plans/2026-08-31-rust-port.md`, complete at `8b4a2a1`, 49 tests, parity 320/0.

## Why these two, and why now

**The hook has never been confirmed to load.** 49 tests and a 320-input parity harness prove the binary computes correctly; none of them prove Claude Code ever runs it. `target/` is gitignored, so a fresh clone has no binary and the hook exits 127 — which `PreToolUse` treats as non-blocking, so it fails *silently and indistinguishably from having nothing to say*. Everything built so far is theoretical until this link is verified.

**Credit expiry is measured, urgent, and larger than everything else combined.** From the transaction log: ~6,910 credits forfeited to expiry over six months against ~732 ever spent generating. Roughly ten times more thrown away than used.

**A correction this plan must carry.** `docs/credit-expiry-task.md` currently says to assume "reset on the 6th". That is wrong. The six observed resets are `2026-08-05, 07-06, 06-06, 05-07, 04-07, 03-08` — the gaps are **30, 30, 30, 30, 30 days**. It is a 30-day rolling cycle, not a calendar month, and the day-of-month drifts backwards about one day per month. A calendar-month assumption drifts out of correctness within two cycles and warns on the wrong day.

## Global Constraints

Carried from the predecessor plan, all still binding on the hook path:

- **Never block, never exit non-zero, never write to stderr — including on internal panic** — *for the hook path only*. `doctor` is a separate subcommand a human runs deliberately; it SHOULD exit non-zero and print to stderr on failure. Task 1 must prove the hook path is unchanged.
- **Fail silent** on malformed stdin in the hook path.
- **No network, no filesystem writes.** `doctor` may additionally `stat` files it is checking.
- stdlib plus `serde_json` and `regex` only.
- `src/higgsfield_lint.py` remains the read-only parity oracle. Do not modify or delete it.
- **All 49 existing tests must keep passing, and parity must stay at 320 inputs / 0 divergences.**

New to this plan:

- **`doctor` must not change hook behaviour on any input.** Argument dispatch happens before stdin is read; with no arguments the binary behaves exactly as it does today, byte for byte.
- **The scheduled task is not created by this plan.** Task 4 writes its specification and the exact command to run. Creating a recurring agent that spends tokens indefinitely is the human's decision.

---

## File Structure

| File | Responsibility |
|---|---|
| `rust/src/main.rs` | Argument dispatch: no args → hook (unchanged), `doctor` → diagnostics. |
| `rust/src/doctor.rs` | All install-chain checks and their reporting. New. |
| `rust/tests/doctor.rs` | Integration tests driving the real binary's `doctor` path. New. |
| `rust/tests/fail_silent.rs` | Extended: prove the hook path is untouched by argument dispatch. |
| `docs/credit-expiry-task.md` | Rewritten with correct 30-day cycle arithmetic. |
| `SPEC.md` | §8 rewritten; a `doctor` subsection added. |
| `README.md` | `doctor` documented as the first thing to run after building. |

---

### Task 1: `doctor` scaffold and hook-path isolation

**Files:**
- Modify: `rust/src/main.rs`
- Create: `rust/src/doctor.rs`
- Create: `rust/tests/doctor.rs`
- Modify: `Cargo.toml` (add the `[[test]]` entry), `rust/tests/fail_silent.rs`

**Interfaces:**
- Produces: `doctor::run() -> i32` (exit code); `enum Check { Pass(String), Warn(String), Fail(String) }`.

This task adds **no diagnostics**. Like Task 1 of the port, it proves the dangerous property first: that adding argument dispatch cannot alter the hook path.

- [ ] **Step 1: Write the failing tests**

`rust/tests/doctor.rs` — that `doctor` runs and exits 0 with a stub check, and that it writes to stdout.

Append to `rust/tests/fail_silent.rs` — the isolation guarantee:

```rust
#[test]
fn argument_dispatch_does_not_change_the_hook_path() {
    // Every pathological payload must behave identically when the binary is invoked
    // with no arguments, exactly as before `doctor` existed.
    let payload = br#"{"tool_input": {"params": {"prompt": "shot in 4k", "width": 1376, "height": 768}}}"#;
    let (code, out, err) = run(payload);
    assert_eq!(code, 0);
    assert!(err.is_empty());
    assert!(!out.is_empty(), "the hook path must still emit findings");
}

#[test]
fn an_unrecognised_argument_still_behaves_as_the_hook() {
    // PreToolUse invokes the command with no arguments. If anything ever passes a stray
    // argument, the safe behaviour is the hook path, never a diagnostic dump on stdout
    // that Claude Code would read as `additionalContext`.
    let exe = env!("CARGO_BIN_EXE_higgsfield-lint");
    let mut child = std::process::Command::new(exe).arg("--nonsense")
        .stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped()).spawn().expect("spawn");
    use std::io::Write;
    child.stdin.as_mut().unwrap()
        .write_all(br#"{"tool_input": {"params": {"prompt": "x"}}}"#).ok();
    let out = child.wait_with_output().expect("wait");
    assert_eq!(out.status.code().unwrap_or(-1), 0);
    assert!(out.stderr.is_empty());
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test --release`
Expected: FAIL — `doctor.rs` does not exist / the `[[test]]` entry is missing.

- [ ] **Step 3: Implement dispatch**

In `main.rs`, dispatch on `std::env::args().nth(1)` **before** reading stdin. `Some("doctor")` → `std::process::exit(doctor::run())`. Everything else, including unrecognised arguments, falls through to the existing hook path untouched. Keep the panic hook and `catch_unwind` wrapping the hook path exactly as they are.

`doctor.rs` gets the `Check` enum, a reporter, and a single stub check returning `Pass`.

**Remember the `[[test]]` entry in `Cargo.toml`** — the package root is the repo root, so `rust/tests/` is not auto-discovered and its absence produces a false green reporting "0 tests".

- [ ] **Step 4: Run**

Run: `cargo test --release`
Expected: PASS, 52 tests (49 + 1 doctor + 2 isolation). Confirm the count rose.

- [ ] **Step 5: Confirm parity is untouched**

Run: `cargo test --release --test parity -- --nocapture`
Expected: `compared 320 distinct inputs ... 0 divergence(s)`. The hook path did not change, so this must be unchanged.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml rust/
git commit -m "feat(doctor): argument dispatch with the hook path provably unchanged"
```

---

### Task 2: The install-chain checks

**Files:**
- Modify: `rust/src/doctor.rs`, `rust/tests/doctor.rs`

**Interfaces:**
- Consumes: `Check`, `run()` from Task 1.
- Produces: one function per check, each returning `Check`.

Each check answers one question that, answered wrongly, makes the hook silently do nothing. Order them so the earliest failure is the most fundamental.

| # | Check | Fails when |
|---|---|---|
| 1 | Binary location | `current_exe()` is not under a directory containing `data/` — the layout `load_models` depends on |
| 2 | Data file | `data/negative_prompt_models.json` missing, unparseable, or not a JSON array of strings |
| 3 | Binary freshness | any `rust/src/*.rs`, `Cargo.toml`, or `data/*.json` is **newer** than the running executable — the stale-binary case that looks identical to a working one |
| 4 | Settings file | `.claude/settings.json` missing, unparseable, or lacking a `hooks.PreToolUse` array |
| 5 | Matcher present | no `PreToolUse` entry whose `hooks[].command` resolves to this executable |
| 6 | Matcher correctness | the matcher does not match `mcp__ada4cfac-…__generate_video` (a real tool name), or wrongly matches `mcp__x__generate_3d` / `mcp__x__show_generations` |
| 7 | Self-test | lint a known-offending payload in-process and confirm exactly the three expected `check_id`s come back |

Check 3 is the one that cannot be discovered any other way: a stale binary runs old logic and is indistinguishable from a current one at runtime.

- [ ] **Step 1: Write the failing tests**

For each check, one passing case and one failing case, driven through the real binary with a temp directory standing in for the repo where the check reads the filesystem. At minimum: a missing data file reports Fail; a data file containing `{"a":1}` (not an array) reports Fail; a settings file with a matcher that misses `generate_video` reports Fail; the self-test check reports Pass on the current build.

- [ ] **Step 2: Run to verify they fail**, then **Step 3: implement**, then **Step 4: run**

Expected after Step 4: all green, count risen. `doctor` exits 0 when every check passes, 1 when any Fail is present, and 0 when the worst result is a Warn.

- [ ] **Step 5: Run `doctor` against the real checkout and paste the output**

Run: `./target/release/higgsfield-lint doctor`
Expected: every check Pass. Paste it verbatim in the report — this is the first evidence the install chain is sound.

- [ ] **Step 6: Confirm parity untouched**, then commit

```bash
git add rust/
git commit -m "feat(doctor): install-chain checks including the stale-binary case"
```

---

### Task 3: Credit expiry — correct the cycle arithmetic

**Files:**
- Rewrite: `docs/credit-expiry-task.md`

**No code.** This task produces the specification the scheduled task will follow. Creating that task is Step 3 and belongs to the human.

- [ ] **Step 1: Rewrite `docs/credit-expiry-task.md`**

It must correct the error in the current version and carry the evidence:

```markdown
# Credit expiry reminder

## The problem, measured

From the Higgsfield `transactions` log: ~6,910 credits forfeited to monthly expiry across
six cycles, against ~732 ever spent generating. Unused credits are wiped at each renewal
(`Subscription Credits Reset` rows), not carried over. Plan: `ultimate`, 1,200 credits
granted per cycle.

| Cycle reset | Forfeited at reset |
|---|---|
| 2026-08-05 | 1,192 |
| 2026-07-06 | 1,200 |
| 2026-06-06 | 1,200 |
| 2026-05-07 | 1,200 |
| 2026-04-07 | 1,200 |
| 2026-03-08 | 918 |

## The cycle is 30 days, NOT calendar-monthly

Gaps between the six observed resets: 30, 30, 30, 30, 30 days. The day-of-month drifts
backwards roughly one day per month. **An earlier version of this document said to assume
"reset on the 6th"; that was wrong** and would warn on the wrong day within two cycles.

Compute the next reset as `last_known_reset + 30 days`, re-anchoring on the most recent
`Subscription Credits Reset` row each run rather than extrapolating from a fixed date.

Anchor: `2026-08-05` → next `2026-09-04`.

## The scheduled task prompt

> Call the Higgsfield `transactions` tool and find the most recent `Subscription Credits
> Reset` row; that timestamp is the last reset. The cycle is 30 days, so the next reset is
> that date plus 30 days. Then call `balance` for the current credit total.
>
> Report the credit total and the days remaining until the next reset. If more than 400
> credits remain and fewer than 7 days are left, say so prominently and state that unused
> credits are wiped at reset rather than carried over.
>
> If the most recent reset is more than 31 days ago, the 30-day assumption has broken —
> say so rather than reporting a date computed from it.

## Schedule

Weekly, Mondays 09:00 local. Weekly is sufficient because the warning window is 7 days;
a daily task would spend tokens for no additional coverage.
```

- [ ] **Step 2: Commit**

```bash
git add docs/credit-expiry-task.md
git commit -m "docs: correct the credit-expiry cycle to 30 days, not calendar-monthly"
```

- [ ] **Step 3: HUMAN ACTION — create the scheduled task**

**Do not perform this step.** It creates a recurring agent on the human's account that spends tokens indefinitely. Report it as the outstanding action, quoting the prompt from Step 1 and the Monday 09:00 weekly schedule, so they can create it with the `schedule` skill in one step.

---

### Task 4: Document both, and the verification gap that remains

**Files:**
- Modify: `SPEC.md`, `README.md`

- [ ] **Step 1: Amend `SPEC.md` §8**

Replace the current §8 with the corrected arithmetic from Task 3 (30-day cycle, the anchor date, the forfeiture table), and state plainly that the scheduled task is specified but not created, and why.

- [ ] **Step 2: Add a `doctor` subsection to `SPEC.md`**

Cover: what each of the seven checks proves; that `doctor` deliberately exits non-zero and writes to stderr, which is safe **because it is never on the hook path**; and — stated explicitly — **what `doctor` still cannot prove**: that Claude Code has actually loaded the hook. `doctor` verifies every link that can be checked from outside the running process. Confirming the hook is registered in a live session still requires `/hooks` in an interactive terminal, and no code in this repo can substitute for it.

- [ ] **Step 3: Update `README.md`**

`cargo build --release` then `./target/release/higgsfield-lint doctor` as the first two commands after cloning, with a one-line note that a green `doctor` plus `/hooks` showing the entry is the complete verification.

- [ ] **Step 4: Final verification**

Run: `cargo test --release` (all green), the parity harness (320/0), and `./target/release/higgsfield-lint doctor` (all Pass). Paste all three.

- [ ] **Step 5: Commit**

```bash
git add SPEC.md README.md
git commit -m "docs: document doctor and the corrected expiry cycle"
```

---

## Notes for the executor

- The hook path is sacred. Any change that makes the no-argument invocation behave differently on any input is a defect, regardless of how convenient it is.
- Run the parity harness after every task, not just at the end. It is the regression net for the whole port and this plan touches `main.rs`.
- `doctor` is the only place in this codebase permitted to exit non-zero or write to stderr. Do not let that permission leak into the hook path.
- The predecessor plan shipped two bugs its own suite passed, both because no test covered the class. When you add a check, add the test that would catch it silently passing.
