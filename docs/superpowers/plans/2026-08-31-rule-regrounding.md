# Rule Re-grounding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Re-ground the three shipped rules on first-party Higgsfield evidence, removing the ones that fire on Higgsfield's own published house style, and add the best-evidenced rule we never built.

**Architecture:** The Python oracle is retired; the Rust becomes the specification. `rust/tests/parity.rs` converts from a differential harness into a golden-output regression corpus with recorded expectations. Every confirmed false positive from the research becomes a permanent negative control, added in the same change as the fix that closes it.

**Tech Stack:** Rust (existing crate, stdlib + `serde_json` + `regex`).

**Spec:** `SPEC.md`, and `HIGGSFIELD_PROMPT_RESEARCH.md` as the evidence base. **Cite the research by section rather than restating it** — it is 743 lines of sourced argument and duplicating it into this plan would create two copies to keep in sync.

**Predecessor:** `docs/superpowers/plans/2026-08-31-doctor-and-expiry.md`, Tasks 1-3 complete at `a8779c8`. Its Task 4 (documentation) is folded into Task 7 here, since `SPEC.md` is being substantially rewritten either way.

## Why this plan exists

`HIGGSFIELD_PROMPT_RESEARCH.md` audited all three shipped rules against Higgsfield's first-party guidance, running every probe against the real binary. Its findings, in severity order:

1. **`negation_placement` is contradicted by first-party guidance** (research §6). Higgsfield's help centre tells users to keep exclusions in the positive prompt; their Seedance 2.5 testing writeup calls inline exclusions the highest-yield part of a prompt. **No model in the ~63-entry catalog declares a `negative_prompt` parameter**, so the rule's gate treats "the caller supplied the field" as proof the field works. The rule fired on 7 of 9 historical calls, which `SPEC.md` §2 read as signal; the corpus was following Higgsfield best practice.
2. **`resolution_claim` fires on Higgsfield's own published style header** (§7, §8.1). `Style: 8K IMAX commercial` is a stylistic register, not a render assertion. Separately, its highest-value miss is a `4K` claim on `kling2_6`, which is silent because that model declares neither dimensions nor `resolution` — meaning the `1376×768` that seeded the rule are undeclared passthrough fields.
3. **`prompt_negative_conflict` has three confirmed false positives** (§8.2), all on single-token entries: `pan`, `text`, `sharp`.

## Two decisions taken before planning

**Rule 5.3 is disabled now and replaced later.** Its five false-positive classes become permanent negative controls in the same change. The volume-based replacement (research HFR-006) needs thresholds calibrated against the corpus and is explicitly out of scope here.

**The Python oracle is retired; the Rust becomes the specification.** `src/higgsfield_lint.py` existed to verify the port, and it did: 320 differential inputs with zero divergences, plus 4,200 randomized inputs from an independent reviewer. Keeping it would mean implementing every rule change twice and re-deriving parity each time. Task 1 retires it deliberately and records why, rather than letting it rot into a stale second implementation nobody updates.

## Global Constraints

Unchanged from the shipped contract:

- **Never block, never exit non-zero, never write to stderr — including on internal panic** — on the hook path. `doctor` remains the sole exception.
- **Fail silent** on malformed stdin, missing fields, wrong types.
- **No network, no filesystem writes.** Permitted reads: `data/negative_prompt_models.json` (until Task 2 retires it), `data/project-profile.json` if it ever exists, `.claude/settings.json` (doctor only).
- stdlib plus `serde_json` and `regex` only.
- **Precision is the product, in both directions.** A rule that fires on a known-good call is a bug; a rule that silently drops what it was built for is worse.

New to this plan:

- **Every confirmed false positive in research §8 becomes a permanent negative control**, added in the same task as the fix that closes it. None may be removed later without evidence.
- **`SPEC.md` changes first, then code.** A rule's specification is the authority; the code follows. Never edit a test to make a red diff go green.
- **The golden corpus records expectations, not the other way round.** When a change flips a golden expectation, that flip is reviewed as a behaviour change, not absorbed silently.

---

### Task 1: Retire the oracle, convert parity to a golden corpus

**Files:** Modify `rust/tests/parity.rs`, `Cargo.toml` if needed; move `src/higgsfield_lint.py` and `tests/` to `docs/archive/verified-port/`; modify `SPEC.md`, `README.md`.

This must come first. Every later task changes behaviour, and while the oracle stands, each change would need implementing twice.

- [ ] **Step 1: Capture the current golden output**

Before moving anything, run the existing 320-input corpus through the Rust binary and record each input's exact output to a checked-in file (`rust/tests/golden/expectations.json` or equivalent). This is the baseline that later tasks will deliberately flip.

- [ ] **Step 2: Convert `parity.rs`**

Replace the Python subprocess comparison with a comparison against the recorded expectations. Keep the corpus generation, the parsed-JSON comparison, the divergence reporting, and the `[[test]]` entry. The test's failure message must name the input and show expected versus actual.

- [ ] **Step 3: Archive the oracle**

`git mv src/higgsfield_lint.py docs/archive/verified-port/` and the Python `tests/` alongside it. Add a short `docs/archive/verified-port/README.md` recording: what it was, that it verified the Rust port at 320/0 plus 4,200 independent randomized inputs, the commit at which that was true, and that it is a historical record which must not be treated as current behaviour.

- [ ] **Step 4: Update `SPEC.md` and `README.md`**

Remove the "retained parity oracle" language. State that the Rust is the specification and the golden corpus is the regression instrument.

- [ ] **Step 5: Verify and commit**

`cargo test --release` green; golden corpus passing at its recorded count; `doctor` still all-Pass.

---

### Task 2: Disable `negation_placement`, lock its false positives

**Files:** Modify `rust/src/checks.rs`, `rust/src/lint.rs`, `rust/tests/`, `data/negative_prompt_models.json`, `SPEC.md`.

Research §6 is the evidence; §8.3 lists the five confirmed false-positive classes.

- [ ] **Step 1: Add the five FP classes as negative controls, asserting silence**

From research §8.3: Higgsfield's own style header (`No jitter, no music, no subtitles, no readable text`); scene description (`A road with no cars`); quoted literal (`A sign reading "NO PARKING"`); span spill (the unpunctuated case yielding `"no logos a beautiful"`); and `kling2_6` firing with no `negative_prompt` in the call at all. Each asserts **no finding**.

- [ ] **Step 2: Run them and watch them fail**

They must fail now — that is the bug. Record the output.

- [ ] **Step 3: Remove the check**

Delete `check_negation_placement` from `CHECKS`. Keep `PLACEMENT_NEGATION_RE`, `_is_placement_noise` and the stoplist in place with a comment pointing at HFR-006 — research §"Keep" is explicit that the noise filters are correct and carry over to the replacement.

Retire `data/negative_prompt_models.json`. Update `doctor`'s data-file check accordingly, or repoint it if another data file takes its place.

- [ ] **Step 4: Verify**

The five controls now pass. `seedance_tree_clean` still silent. Golden corpus: every expectation that changes must be a `negation_placement` finding disappearing — **review each flip individually** and record the count in the report.

- [ ] **Step 5: Update `SPEC.md` §5.3**

Replace it with a short record of why the rule was removed, citing research §6, and a forward pointer to HFR-006.

---

### Task 3: Recalibrate `prompt_negative_conflict`

**Files:** Modify `rust/src/checks.rs`, tests, `SPEC.md`.

Research §8.2 for the false positives, §10 "Rule 5.2 recalibration" for the specified thresholds.

- [ ] **Step 1: Add the three confirmed FPs as negative controls**

`pan` (cast-iron pan vs a `pan` camera directive), `text` (quoted literal `"OPEN"` with `misspelled text` in the negative — the pairing Nano Banana's docs recommend), `sharp` (`sharp focus` vs `not sharp`, which is agreement). Plus `not sharp` in the negative field specifically, and a `negative_prompt` of `""` and of whitespace.

- [ ] **Step 2: Run them, watch them fail**

- [ ] **Step 3: Implement the four changes from research §10**

Strip negations from **both** fields, not just the prompt. Exclude occurrences inside matched double quotes in the prompt. Require ≥2 tokens or membership in a strict single-token set with a 4-character minimum. Drop the bare `pan`, `text`, `sharp`, `static`, `zoom`, `grain`; add the bigrams where the directive sense matters (`camera pan`, `whip pan`, `slow zoom`, `on-screen text`, `film grain`, `static shot`) and the new multi-token entries (`lens flare`, `letterbox`, `watermark`, `subtitles`).

- [ ] **Step 4: Verify the one real historical hit survives**

`slow motion` on the `kling2_6` fixture must still fire — it is two tokens and survives the new rule. If it does not, the recalibration went too far.

- [ ] **Step 5: Golden flips reviewed individually; `SPEC.md` §5.2 updated first**

---

### Task 4: Re-ground `resolution_claim`'s lexicon

**Files:** Modify `rust/src/checks.rs`, tests, `SPEC.md`.

Research §7 for the stylistic-register argument, §8.1 for the probes, HFR-R1 for the `8k` carve-out.

- [ ] **Step 1: Add the FP controls**

Higgsfield's `Style: 8K IMAX commercial, 16:9 widescreen. … 8K detail.` with `resolution: "1080p"` — must be silent. A quoted/diegetic case: `The presenter says "shot in 4K" to the camera` with real dimensions — must be silent.

- [ ] **Step 2: Run them, watch them fail**

- [ ] **Step 3: Drop `8k`, `uhd`, `full hd` from the claim lexicon**

Per HFR-R1: `8k` is the only entry with no attainable target anywhere in the catalog and is Higgsfield's own house style, so every firing is either a false positive or an unactionable truth. `uhd` and `full hd` appear in no official prompt and have never fired.

- [ ] **Step 4: Exclude quoted spans from claim detection**

Same mechanism as Task 3's quoted-span exclusion; factor it into one shared helper rather than implementing it twice.

- [ ] **Step 5: Extend `RESOLUTION_PARAM` to the image and `minimax` vocabularies**

Add `1k`, `1.5k`, `2k`, `512`, `768`, `2160p` per research §7's taxonomy. Without this the rule is dead on every image model, which is ~88% of real activity.

- [ ] **Step 6: Fix the message on models with no `resolution` parameter**

"Raise resolution" is wrong advice on `kling2_6`, `wan2_6`, `kling3_0`. Either soften it or condition it on the parameter existing.

- [ ] **Step 7: Golden flips reviewed; `SPEC.md` §5.1 updated first**

---

### Task 5: Add `duration_claim_conflict` (HFR-001, over-claim branch only)

**Files:** Modify `rust/src/checks.rs`, `rust/src/lint.rs`, tests, `SPEC.md`.

Research HFR-001 has the full algorithm, trigger set, and non-trigger set. **Follow it exactly** — including the decision to ship the over-claim branch only, which trades one false-negative class for the removal of the rule's main false-positive risk (a within-scene duration read as a total).

- [ ] **Step 1: Write the tests from HFR-001's trigger and non-trigger lists**

Must fire: `Total: 15s / 6 shots / 16:9` with `duration: 5`; `A 12-second cinematic sportswear ad` with `duration: 5`; timecode spans `0–3s … 12–15s` with `duration: 8`.
Must not fire: `a 5 second pause between lines` with `duration: 15`; `shot on a 50mm`; `15 shots`; duration absent; duration non-numeric.

- [ ] **Step 2: Run, watch fail, implement, run**

Scope to `generate_video` and `generate_video_batch` only. Not image, not audio.

- [ ] **Step 3: Add to `CHECKS` in the documented order and update `SPEC.md` §5**

---

### Task 6: Tool-name scoping

**Files:** Modify `rust/src/lint.rs`, `rust/src/checks.rs`, tests, `SPEC.md`.

Research §8.3 and the "Add" list: on `generate_audio*` the `prompt` **is the spoken text**, so prompt-text checks are meaningless there.

`Ctx`-style plumbing already exists in spirit — `main.rs` reads `tool_name` but `lint()` discards it. Thread it through and skip text checks on audio tools.

- [ ] **Step 1: Test that a `generate_audio` call with a resolution word and negations produces nothing**
- [ ] **Step 2: Run, watch fail, implement, run, golden flips reviewed**

---

### Task 7: Documentation reconciliation

**Files:** `SPEC.md`, `README.md`, `docs/credit-expiry-task.md` (verify only).

This absorbs the unfinished Task 4 of the predecessor plan.

- [ ] **Step 1: `doctor` section in `SPEC.md`**

What each check proves; that `doctor` deliberately exits non-zero and writes to stderr, safe because it is never on the hook path; and **explicitly what it cannot prove** — that Claude Code has loaded the hook. That still requires `/hooks` in an interactive terminal and no code here substitutes for it.

- [ ] **Step 2: Credit-expiry section**

Mirror `docs/credit-expiry-task.md`'s corrected 30-day arithmetic into `SPEC.md` §8, with the same anchor (`2026-08-05`) and cycle. State that the scheduled task is specified but not created, and why.

- [ ] **Step 3: The two accuracy defects research found**

`SPEC.md` §9's `anyOf` claim is over-broad — batch entries are object-only. §6's sample message says "Set resolution" while the code emits "Raise resolution".

- [ ] **Step 4: Record the evidence base**

A short section citing `HIGGSFIELD_PROMPT_RESEARCH.md` as the grounding for the rule set, with its evidence-class convention (A first-party explicit, A′ machine-readable catalog, B official examples, D engineering judgment) so future rule proposals are held to the same standard.

- [ ] **Step 5: `README.md`** — build step, `doctor` first, oracle retired.

---

## Explicitly out of scope

Deferred to a second wave, with reasons:

- **HFR-002 `resolution_ceiling_conflict`** — needs a dated `data/model_resolution_ceilings.json` snapshot and a refresh discipline. High value (it is `resolution_claim`'s largest false negative and the honest way to catch the `kling2_6` 4K case) but it introduces a staleness surface that deserves its own design.
- **HFR-003 `aspect_ratio_claim_conflict`** — class A and mechanical, but wants the exactly-one-ratio guard so Higgsfield's dual-ratio prompts stay silent. Straightforward once Task 4's quoted-span helper exists.
- **HFR-006 `negation_density`** — the replacement for the rule Task 2 removes. Its thresholds are class D and must be calibrated against the corpus first; Higgsfield's own 800-word prompts carry 15-30 exclusions and must stay silent.
- **HFR-004, HFR-005, HFR-007** — per research §14 these sit closer to the docstring non-goal; HFR-007's value is mostly its interaction with HFR-001's timecodes.

## Notes for the executor

- **Cite the research, do not restate it.** `HIGGSFIELD_PROMPT_RESEARCH.md` is the evidence base; this plan is the work order.
- **Every golden-corpus flip is reviewed individually.** A changed expectation is a behaviour change and must be justified in the task report, never absorbed as noise.
- This project has shipped **five** tests that passed while proving nothing. For every negative control you add, satisfy yourself it fails before the fix and passes after — and say so in your report.
- `doctor` must stay all-Pass throughout. If a task changes the data files it reads, update the check in the same task.
