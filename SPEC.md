# Higgsfield Lint — Spec

Replaces the Preflight Gate design (archived at
`docs/archive/2026-08-30-preflight-gate-v1-rejected.md`), which was cut after measurement.
§13 records what was cut and the condition that would bring it back.

A `PreToolUse` hook that reads Higgsfield generation calls before they run and tells the
model when the prompt argues with its own parameters. Advisory only. It never blocks.

---

## 1. Why this is small

The predecessor design was 990 lines aimed at stopping contract violations before they
burned credits. Measurement killed the premise.

| Measurement | Value | Source |
|---|---|---|
| Monthly credit grant | 1,200 | `balance`, plan `ultimate` |
| Unused credits at renewal | **wiped** | `Subscription Credits Reset` rows |
| Credits forfeited to expiry, Mar–Aug | **6,910** | six `Reset` rows |
| Credits spent generating, all history | ~732 | sum of `spend` rows |
| Failed generations | 4 of 36 August calls (~11%) | `refund` rows |
| Credit cost of those failures | **0** | every failure refunded in full |
| Video generations, lifetime | **9** | `show_generations`, `next_cursor: null` |
| Seed rules that would have fired on those 9 | **0** | manual check against the archived §8 |

Two facts end the credit-saving argument. Credits are not scarce, since roughly ten times
more are forfeited to monthly expiry than are spent generating. And failures are already
free, because Higgsfield refunds them.

The archived `E[gain] = p_fail·(C_credits + C_latency + C_rework)`, evaluated at last:
`p_fail ≈ 0.11`, `C_credits = 0`. What remains is latency and rework on about four events
a month.

At 9 lifetime video calls, the archived promotion rule (30 clean firings) was also
unreachable by roughly two orders of magnitude.

## 2. What the evidence does support

Checking the same nine generations against the archived Class B sections, three patterns
fired on real usage:

1. **`"4K"` in the prompt against `width: 1376, height: 768`.** Three `kling2_6` calls.
2. **`"slow motion"` in both `prompt` and `negative_prompt`.** The same three calls.
3. **Negations in the positive prompt** (`"no text"`, `"no logos"`, `"no added text"`,
   `"no text overlays"`) in seven of nine, while `kling2_6` accepts a `negative_prompt`
   and was being given one at the same time.

Those three are the product. Everything else is cut.

## 3. Mechanism

A `PreToolUse` hook, verified against the Claude Code hook reference:

- Matchers accept regex over MCP tool names (`mcp__<server>__<tool>`).
- The hook receives `tool_name` and `tool_input` as JSON on stdin.
- It returns `hookSpecificOutput.additionalContext` to pass information to the model, or
  `permissionDecision: "deny"` with `permissionDecisionReason` to block.
- Exit 0 with no output means no decision, and the normal permission flow proceeds.

This supplies the archived design's whole verdict vocabulary natively. The MCP proxy,
the `rmcp` dependency, schema re-exposure, the crate layout, and the `BlockAuthority`
witness (which the pseudocode pass proved unbuildable as specified) are all unnecessary.

**Matcher:** `mcp__.*__generate_(image|video|audio)(_batch)?$`

Matching on tool shape rather than server id keeps the hook working if the Higgsfield
server is re-registered under a different id.

## 4. Authority

**Advisory only. The hook never returns `deny`.**

This follows from §1 rather than from caution. Blocking exists to prevent spend; spend is
not the constraint here. A false block would cost the one thing that is actually scarce,
which is the user's attention. Every finding goes back as `additionalContext` and the call
proceeds.

The consequence is that the archived promotion statistics, randomised holdout, confirm
budget, verifier, and outcome log all become unnecessary. Nothing needs to earn block
authority, because block authority is not on offer.

## 5. Checks

### 5.1 Resolution claim contradicts dimensions

Prompt asserts a resolution the call will not produce.

Lexicon maps a claim to its minimum short-side pixel count:

| Claim | Min short side |
|---|---|
| `8k` | 4320 |
| `4k`, `uhd` | 2160 |
| `1080p`, `full hd` | 1080 |
| `720p` | 720 |
| `480p` | 480 |

Actual is `params.resolution` when present, otherwise `min(width, height)`. Fires when
claimed exceeds actual. Silent when neither is present.

### 5.2 Term in both prompt and negative_prompt

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

### 5.3 Negation belongs in negative_prompt

Fires when the positive prompt contains a negation (`no X`, `without X`, `avoid X`) **and**
the model is known to accept a `negative_prompt`. Only these three forms count as a
directive; ordinary narrative negation (`"has not yet risen"`), quantifier idioms
(`"no more than three"`, `"no longer"`, `"no matter"`), and a bare gerund right after the
negator (`"without moving"`) are not treated as a misplaced directive, since moving any of
those into `negative_prompt` would do nothing or damage the shot.

Known-accepts is evidence-seeded, not inferred: a model qualifies when `negative_prompt` is
already present in this call **and non-empty**, or when it appears in
`data/negative_prompt_models.json`, which starts as `["kling2_6"]` because that is what the
generation history proves. The catalog does not publish negative-prompt support, so
guessing would produce false findings on models that silently ignore the field.

## 6. Output contract

Findings are returned as one `additionalContext` string. No finding means exit 0 with no
output.

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "additionalContext": "higgsfield-lint: 2 findings\n- resolution_claim: prompt says \"4K\" but width 1376 x height 768 gives a 768px short side. Raise resolution, or drop the claim from the prompt.\n- prompt_negative_conflict: \"slow motion\" appears in both prompt and negative_prompt. Keep each term in one."
  }
}
```

Each finding is one line: `<check_id>: <what was observed>. <mechanical fix>.`

## 7. Failure behaviour

The hook is in the path of every Higgsfield generation call, so its own failure must be
invisible.

- Any exception, malformed stdin, or missing field exits 0 with no output.
- The hook never writes to stderr on the success path, since exit 2 plus stderr would block.
- No network calls and no MCP access. The hook reads its own data file and appends one
  metadata record per call to `data/findings.jsonl` (§9.1). That append is best-effort
  and load-bearing for nothing: every failure mode - unlocatable repo root, unwritable
  path, a record that would exceed the atomic-append size - is swallowed, and the lint's
  stdout is byte-identical whether the append succeeded or not. The invariant this bullet
  protects is not "no writes", it is "the hook's own failure is invisible"; a write that
  cannot fail visibly does not threaten it.
- The shipped hook is a statically-linked Rust binary (`serde_json` and `regex` are the
  only two dependencies; see §12) rather than a standard-library-only script. The
  dependency-import failure this line originally guarded against doesn't apply the same
  way to a compiled binary: there is no import step at `PreToolUse` time, since both
  crates are compiled in at `cargo build --release` and become part of the single
  executable. What still must hold, and does: no dependency is fetched or resolved at
  run time, and a missing or stale binary is a build-and-deploy problem (§12.1), not a
  standard-library-only one.

## 8. Credit expiry reminder

Separate from the hook, because a hook cannot call MCP tools.

A weekly scheduled Claude Code task calls `balance` and reports the credit total against the
renewal date, which the transaction log places between the 5th and 7th of each month. This
is one call and a date comparison, and on the §1 numbers it is worth more than every check
in §5 combined.

## 9. Testing

Fixtures are real tool inputs reconstructed from `show_generations`, so the regression
corpus is actual usage rather than invented cases.

- **Positive fixtures:** the three `kling2_6` calls, which must produce findings 5.1 and 5.2.
- **Negative fixtures:** the six `seedance_2_5` calls, which must produce **no** findings
  for 5.1 and 5.2. These matter more than the positives; a check that fires on a known-good
  call is a bug, and the whole value of an advisory tool is that its advice is worth reading.
- **Malformed fixtures:** empty stdin, absent `params`, absent `prompt`. All must exit 0
  silently per §7. `params` as a string is not malformed: all three tool schemas declare
  `params` as `anyOf: [object, string]`, so a JSON-encoded string is parsed and linted like
  any other call. Only a string that fails to parse, or that parses to something other than
  an object, falls through to silence.

Since the port (§12), `tests/test_lint.py` verifies only the Python oracle. The Rust
binary is verified two ways:

- **Unit and integration tests** (`cargo test --release`), covering each check's ported
  logic directly against the same behaviour documented above.
- **The differential parity harness** (`rust/tests/parity.rs`), which is the actual
  verification instrument for the port: it runs an identical corpus of stdin payloads
  (fixtures, pathological inputs, and a generated sweep across every lexicon entry,
  dimension pair, and negation form - 320 distinct inputs at last count) through both
  `src/higgsfield_lint.py` and the compiled Rust binary, and asserts their outputs are
  equal as parsed JSON (see §3 note in §12 on why not as raw bytes). A single divergence
  fails the whole test, and the fix is always to change the Rust, never the oracle.
  Run it with `cargo test --release --test parity`.

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

## 10. Repository layout

```
SPEC.md                              this document
README.md                            setup/build steps and the oracle-retention note
Cargo.toml, Cargo.lock               Rust package manifest (serde_json, regex)
rust/src/*.rs                        the hook, compiled to a Rust binary (§12)
target/release/higgsfield-lint       the built hook; PreToolUse runs this directly (§12.1)
src/higgsfield_lint.py               retained parity oracle, stdlib only - NOT the hook (§12.2)
rust/tests/behaviour.rs              the three bounded CI gates
rust/src/lib.rs                      library target shared by both binaries
rust/src/log.rs                      the findings log (9.1)
rust/src/bin/review.rs               higgsfield-lint-review (9.2)
scripts/check-hook-freshness.sh      SessionStart staleness guard (12.1)
data/findings.jsonl                  local findings log, gitignored (9.1)
requirements-dev.txt                 pytest, for tests/test_lint.py
.github/workflows/ci.yml             build, both test suites, clippy, fmt
data/negative_prompt_models.json     evidence-seeded model list
tests/fixtures/*.json                real captured tool inputs
tests/test_lint.py                   pytest, verifies the Python oracle only
rust/tests/parity.rs                 differential parity harness, the port's real verification (§9)
rust/tests/fail_silent.rs            pathological-input and panic-silence tests
docs/archive/                        the rejected design and its pseudocode
docs/superpowers/plans/              implementation plan
```

## 11. Non-goals

- Blocking any call, for the reason in §4.
- Judging whether a prompt is good. Unchanged from the archived design: no ground truth,
  poor precision, and the fastest way to make advisory output that nobody reads.
- Duplicating what the tool docstrings already tell the model. The `generate_video` schema
  already covers the aspect-ratio default, the unlim count cap, the media-id rule, and the
  preset restriction. Those were seed rules in the archived design and are dead.
- Content moderation. Higgsfield does this and reports it.

## 12. Stack

The hook has shipped in two languages. This is the honest record of why, so nobody
re-litigates it from partial memory.

Rust was the original choice for the hook, justified on compile-time enforcement of the
archived design's spec invariants: constructors gated to a single privileged module, and a
witness type that proved a caller held a validated registry rather than an arbitrary one.
The pseudocode pass proved two of those invariants unbuildable as specified. Rust's privacy
is crate-scoped, not module-scoped across a workspace, so a constructor meant to be private
to `gate-core` was reachable from a separate `gate-eval` crate the moment the two lived in
different crates: nothing in the type system stopped it. And the `&Registry` guarantee went
vacuous once several call sites needed interior mutability, since a shared `&Registry`
behind a `RefCell` or `Mutex` proves nothing about what the caller actually validated.
Measurement then deleted the system those two invariants existed to protect (§1), so
whether they were fixable was never answered.

The replacement was written in Python. That decision was made without consulting the
person who chose Rust in the first place. That is a process error, independent of whether
Python was the right call, and it is recorded here so the next stack decision does not
repeat it.

This plan reverses the reversal. The three checks in §5 are ported from
`src/higgsfield_lint.py` to a compiled Rust binary, proven output-equivalent as parsed
JSON to the Python via a differential parity harness (`rust/tests/parity.rs`, 320
distinct inputs, zero divergences) rather than by hand-ported assertions. The two
implementations are not byte-identical: `serde_json::to_string` and Python's `json.dump`
differ in separator whitespace, and `serde_json`'s `Map` sorts keys where Python
preserves insertion order, so a raw `shasum` of the two outputs differs even when they
mean the same thing. The harness parses both sides back into structured values and
compares those, specifically because raw bytes cannot match; no consumer of this output
depends on either byte layout or key order.

What Rust actually buys the current shape, stated narrowly so it cannot be inflated later:

- **A single binary with no runtime interpreter dependency.** The Python hook depends on
  `python3` existing on `PATH` at the moment `PreToolUse` fires. Rust does not remove the
  failure mode that follows from that dependency breaking; it changes what triggers it.
  See §12.1.

What it costs, equally plainly:

- Two crate dependencies (`serde_json`, `regex`) where Python needed none.
- A build step (`cargo build --release`) that did not exist before.
- A new silent-failure mode, in §12.1.

Startup drops from a measured 25.1ms median (`python3` interpreter boot, 30 runs) to a
measured 2.3ms median (native binary exec, 30 runs). That is still a small fraction of a
generation call's latency and is explicitly **not** a reason this port happened.

### 12.1 New failure mode: stale or missing binary

The hook now runs `$CLAUDE_PROJECT_DIR/target/release/higgsfield-lint` directly. If that
file is missing, or was built against an older source tree, the hook exits 127 or runs
stale logic. `PreToolUse` treats a non-zero exit as non-blocking, the same way it treats
§7's fail-silent exit 0: the generation call proceeds either way, but the two outcomes are
not the same. Exit 127 means the lint did not run at all, and nothing distinguishes that
from "the lint ran and found nothing." Nothing in the current setup surfaces the
difference.

Consequence: **`cargo build --release` is a required setup step**, not a one-time
convenience. It must be rerun whenever `rust/src/*.rs` changes, and confirmed before the
binary is relied on for the first time.

### 12.2 `src/higgsfield_lint.py` after the port

`src/higgsfield_lint.py` is retained as the parity oracle: the fixed point that
`rust/tests/parity.rs` compares the Rust binary against. It is not a runtime fallback.
Nothing in the hook path calls it if the Rust binary fails; the Rust binary's own
fail-silent behaviour (§7) is what runs on any internal error, not the Python.

Deleting it is a separate, later decision, gated on the Rust binary having run against
real production traffic for a period the human decides. It is not part of this plan.

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

### 12.3 Known divergence: compound `index` values

`index` in a batch request is schema-typed as an integer. If a caller passes a JSON array
or object as `index` anyway, which only happens on a malformed call, the Python oracle's
`str()` renders it in Python repr form (`[1, 2]` with a space, `{'a': 1}` with single
quotes), while the Rust port's `serde_json` `Display` renders `[1,2]` and `{"a":1}`, with no
spaces and double quotes. This is a deliberate, accepted divergence, not a bug: emulating
Python's `repr()` for arbitrarily nested JSON in Rust, to cover a case that only arises from
a malformed call, would make the port look like parity without being it.
`rust/tests/parity.rs` deliberately generates scalar indices only, for this reason; see the
comment at `format_index` in `rust/src/lint.rs`.

## 13. What would bring the big design back

The archived design is not wrong in general, it is wrong at this volume and this billing
model. It becomes correct again if any of these hold:

- **Credits stop expiring**, or usage rises far enough that the monthly grant binds.
  Roughly 1,200 credits a month spent, against the current ~120.
- **Failures stop being refunded.** Every observed failure was refunded in full; if that
  changes, `C_credits` stops being zero and the whole calculation reopens.
- **Volume reaches the point where promotion is reachable.** A rule needs ~30 firings.
  At 9 lifetime video calls this is years away; at a few hundred calls a month it is weeks.
- **Rule authors and credit payers stop being the same person.** The type-enforced Class A
  and Class B separation exists to protect against a stranger's mistake. With one operator,
  deleting a bad line is the whole governance system.

Re-read the archive before rebuilding. Note that two of its central claims were found
false: the `BlockAuthority` mechanism cannot be built as written (Rust privacy is
crate-scoped), and §20's premise that resolved defaults eliminate unspecified parameters is
contradicted by its own §18 example.

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
