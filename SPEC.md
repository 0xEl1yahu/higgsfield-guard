# Higgsfield Lint — Spec

Replaces the Preflight Gate design (archived at
`docs/archive/2026-08-30-preflight-gate-v1-rejected.md`), which was cut after measurement.
§13 records what was cut and the condition that would bring it back.

A `PreToolUse` hook that reads Higgsfield generation calls before they run and tells the
model when the prompt argues with its own parameters. Advisory only. It never blocks.

---

## 1. Why this is small

This section records the original Mar–Aug 2026 account measurements, not a current
balance, entitlement or refund-policy guarantee. No billing state was refreshed during
the September implementation; canonical schemas do not by themselves prove charges.

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

For that measured account history, credits were not scarce: roughly ten times more
expired than were spent. All four observed failures were refunded. Those observations
motivated an advisory-only design; they do not establish universal future refunds.

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
   `"no text overlays"`) in seven of nine, while `kling2_6` was being supplied a
   `negative_prompt`. *(Submitted fields never proved model support; see §5.3.)*

These are historical observations, not proof of parameter effect. Canonical reconciliation
removed the first pattern's generic dimension inference. The current product comprises
model-grounded resolution comparisons, local textual consistency, and eligible video
supplied-duration over-claims (§5).

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

### 5.1 Resolution claim contradicts a declared output setting

Canonical basis: [2026-09-09 MCP reconciliation](docs/higgsfield-canonical-contract.md)
and its sanitized envelope/catalog capture. A permissive params object does not prove
that the selected model honors arbitrary fields. The old universal short-side map and
bare width/height inference are removed, including the historical Kling warning.

Require Image/Video tool context and an exact supported MCP model/field/option
association. Unknown tools/models, wrong model kind, unsupported settings, absent
settings, or unresolved precedence are silent. No defaults are synthesized.
Recognize standalone ASCII-case-insensitive `4k`, `1080p`, `720p`, `480p`,
using §5.2a normalization and paired-quote exclusions. Highest eligible claim wins.
`8k`, `uhd`, and `full hd` are deliberately dropped as ambiguous style vocabulary.

| Submitted source | Comparison domain | Contract |
|---|---|---|
| Declared resolution K tiers | nominal 1k < 1.5k < 2k < 4k | 4K vs lower K tier fires; no fixed pixel geometry |
| Declared video p tiers | 360p < 480p < 720p < 768p < 1080p < 2160p | Compare p claims directly; 4K vs sub-2160p is a local label convention |
| wan2_6 quality | 720p / 1080p | Same video-tier convention; advice names quality |
| soul_2, soul_cinematic, soul_v2 quality | 1.5k / 2k | Nominal K tiers only; CLI ids are not MCP aliases |
| kling3_0, kling_video_edit mode | 4k only | Matching 4K; std/pro have no inferred numeric size |
| topaz_image, topaz_image_generative output_width/output_height | positive JSON integers within i64 | Compare requested min(width,height) to local thresholds 4K→2160,1080p→1080,720p→720,480p→480; not guaranteed output |

The exact declared-resolution allowlist and option groups live in checks.rs and are
cross-checked against the saved catalog in tests. No hook-time network or data-file
loading. Naked Hailuo strings 512/768/1080 remain deferred. Mixed nominal K and p
claims are incomparable (except the explicitly stated video 4K convention).
For Topaz, positive tiny targets are actual requested pixel fields, not ratio hints:
the old 128px floor is unnecessary and is not carried over from bare dimensions.

No generic resolution→dimensions→quality priority. Stray undeclared dimensions cannot
override a verified carrier. Seedream stored width/height and Clipify processing caps
are excluded. marketing_studio_video with explicit width or height alongside resolution
is suppressed because precedence is unresolved. Seedance 2.0 fast with 1080p/4k is not
treated as a confirmed selection. Unknown explicit modes on mode-sensitive models are
silent rather than coerced to defaults.

Example message: `prompt says "4K" but resolution selects the "2k" output tier.
Choose a supported higher tier, or revise the claim.`
Topaz names requested output_width/output_height and their short side. Claims retain
their original spelling, including fullwidth letters. At most one resolution finding
per entry; finding never claims a successful render or suggests an absent setting.

Accepted limitations: genuine 8K claims, unknown models, missing settings, ambiguous
geometry, ceiling-only contradictions, and negated resolution wording are not generally
understood. A supported `not 4k` can still fire; keep it in limitation metrics.

### 5.2 Term in both prompt and negative_prompt

Fires when a phrase from a curated directive list appears in both fields. Restricted to a
phrase list rather than free tokens, because common words in both fields are normal and
would make this unusable.

**Recalibrated** (`HIGGSFIELD_PROMPT_RESEARCH.md` §10, "Rule 5.2 recalibration") after
three confirmed false positives (research §8.2), each verified against the shipped
binary:

- `{"prompt":"A cast-iron pan sizzles on the stove.","negative_prompt":"pan, zoom, camera movement"}`
  fired on `pan` — a common-noun homograph, not a camera directive.
- `{"prompt":"A poster with the text \"OPEN\" in bold.","negative_prompt":"misspelled text, distorted letters"}`
  fired on `text` — but quoting a literal in the prompt and suppressing typography
  artifacts in `negative_prompt` is exactly the pairing Nano Banana's own help article
  recommends. This is content, not conflict.
- `{"prompt":"sharp focus on the blade","negative_prompt":"soft focus, not sharp"}` fired
  on `sharp` — but `not sharp` in the negative field is *agreement* with the prompt, not a
  conflict with it. Negations were being stripped from the prompt only.

Four changes fix these without weakening genuine detections:

1. **Strip negations from both fields**, not just the prompt. `not sharp` in
   `negative_prompt` is a double negative and must not count as a hit.
2. **Exclude occurrences inside matched double quotes in the prompt.** A quoted literal
   (`The poster reads "OPEN"`) is Higgsfield-recommended content, not direction.
3. **Require a phrase to be ≥2 tokens, or a member of a strict, unambiguous single-token
   set with a 4-character minimum.** No stemming or fuzzy matching — an inflectional
   match (`blur` vs `blurry`) is exactly where a curated list turns into a fuzzy matcher,
   so that class of miss is an accepted cost.
4. **Drop the bare single-token entries with a confirmed or obvious homograph** — `pan`,
   `text`, `sharp`, `static`, `zoom`, `grain` — since a common-noun sense of each collides
   with normal prompt language. Where the directive sense still matters, a bigram takes
   its place instead.

**Accepted cost of change 1:** stripping negations from `negative_prompt` too means a
negated mention there no longer counts as containing the term at all —
`negative_prompt: "no close-up"`, `"avoid slow motion"`, `"not handheld"` now go silent
against an unnegated `close-up`/`slow motion`/`handheld` in the prompt, even though the
bare word is present in both fields. This is deliberate and matches the field's own
semantics (`negative_prompt` already means "avoid this"; a negation stacked on top of
that is genuinely ambiguous, not a coherent conflict signal), but it is a real false
negative class, not a free lunch — recorded here for the same reason item 3's
`blur`/`blurry` miss is.

Recalibrated list (multi-token entries kept from the seed list, plus the bigram
replacements and new entries):

`slow motion`, `slow-mo`, `motion blur`, `close-up`, `wide shot`, `handheld`, `blurred`,
`blurry`, `camera pan`, `whip pan`, `slow zoom`, `on-screen text`, `film grain`, `static
shot`, `lens flare`, `letterbox bars`, `watermark overlay`, `subtitles`.

The rule's one confirmed genuine historical hit, `slow motion`, is two tokens and
survives unchanged.

**Follow-up demotion (task-3 fix report, finding F3):** `letterbox` and `watermark`
shipped as bare single-token entries per research §10's recommended list, then turned up
the same homograph shape as `pan` once tested against ordinary prompt language:

- `{"prompt":"A red letterbox on a quiet village street at dawn.","negative_prompt":"letterbox, black bars, borders"}`
  fired on `letterbox` — a mail slot / postbox, not a camera-format directive.
- `{"prompt":"A close look at handmade paper with a visible watermark.","negative_prompt":"watermark, logo, signature"}`
  fired on `watermark` — a paper watermark (security paper, stationery, currency), not an
  image overlay.

Research §10's recommended list and its own criterion (change 3 above: "a strict,
unambiguous single-token set") disagree here — both terms are recommended as single
tokens, but both fail the unambiguity test that same recommendation demands. Where §10's
list conflicts with §10's own criterion, **the criterion governs**: both are demoted to
bigrams, `letterbox bars` and `watermark overlay`, carrying the same directive sense
without the homograph. This is the same standard, applied after the fact, that condemned
`pan` in the original recalibration.

The overlap finding is a local textual consistency convention, not proof that a model
honors negative_prompt: none of the 79 MCP model details publishes that field.
Inline exclusions remain valid; do not advise moving them into an unsupported field.

### 5.2a Shared text normalization

| Input | Matching representation |
|---|---|
| U+201C/D/E/F | ASCII double quote |
| U+FF01–U+FF5E | Corresponding ASCII |
| U+00A0 / U+3000 | Space |
| Horizontal spaces/tabs | One space, never across newline |
| CRLF / CR | LF |
| Other scalars | Unchanged |

This is narrow folding, not NFKC. Do not remove punctuation, join hyphen-separated
tokens, or reinterpret apostrophes as double quotes. Normalize both conflict fields,
strip the existing both-field negations, lowercase, normalize resulting horizontal
spacing, then calculate positive quote spans on the exact scanned representation.
The negation regex's existing three-word scope is retained, including its accepted
over-stripping costs.

Pair double quotes positionally, first/second then third/fourth. Unpaired trailing
quotes do not exclude content. Doubled quotes and unrelated inch marks retain their
documented quirks. Resolution recovers source spelling by one transform replay after
choosing its match, with no full-input per-character source map. Duration reports a
number and needs no source mapping. Unsupported typography and newline-separated
phrases are accepted misses.

### 5.3 negation_placement — disabled

This rule fired when the positive prompt contained a negation (`no X`, `without X`,
`avoid X`) and the model was treated as accepting a `negative_prompt` (the field already
present and non-empty in this call, or the model listed in
`data/negative_prompt_models.json`, seeded as `["kling2_6"]`). It fired on 7 of 9
historical calls, which §2 read as signal.

`HIGGSFIELD_PROMPT_RESEARCH.md` §6 shows the opposite. Higgsfield's own help centre tells
users to keep exclusions **in the positive prompt**; its Seedance 2.5 testing writeup
calls inline exclusion the highest-yield part of a prompt; and no model across the
~63-entry catalog declares a `negative_prompt` parameter at all. The rule's gate — "the
caller supplied the field, or the model is on a seed list" — was never evidence that the
field does anything, and its remedy sentence asserted a mechanism Higgsfield's own
testing contradicts. Research §8.3 audits five confirmed false-positive classes on this
rule, including Higgsfield's own published style header; all five are now permanent
negative controls in `rust/tests/golden/expectations.json`.

The rule is disabled, not deleted: `check_negation_placement` is gone from
`rust/src/checks.rs`, but `PLACEMENT_NEGATION_RE`, its noise filter, and its stoplist
stay in place, unused, because research §10 (**HFR-006**) reuses that exact filter
unchanged in a volume-based replacement — counting how many exclusions a prompt carries,
rather than judging where they sit. That replacement is out of scope here; its
thresholds still need calibrating against the corpus before it ships.

`data/negative_prompt_models.json`, the evidence-seeded model list this rule's gate
depended on, is retired. The loader module and all invocation-time reads are removed.
Doctor separately checks that the retired file has not reappeared.

### 5.4 Tool context

Classify exact generate_image/video/audio names and their _batch forms, bare or the final
component of an MCP name. Case-sensitive; lookalikes, missing and non-string names are
Unknown. One outer kind applies to every batch entry.

| Kind | Resolution | Local conflict | Duration |
|---|---|---|---|
| Image | matching model only | run | skip |
| Video | matching model only | run | eligible model/mode only |
| Audio | skip before params parsing | skip | skip |
| Unknown | skip | run | skip |

### 5.5 Video supplied-duration over-claim

Public id: duration_claim, appended after resolution_claim and prompt_negative_conflict.
Require Video context, a known duration-capable MCP video model, a string prompt and
finite numeric supplied duration 1..600. No inferred default. Exclude Seedance 2.5 and
ad_multiplier video_edit (duration ignored), video_extension (total ambiguous), unknown
modes, and source-only workflows. video_deflicker is conservatively excluded as a
source-processing workflow despite its optional duration field; video_upscale, Clipify
and Kling video edit likewise do not establish eligible output-duration semantics here.

Supported grammar: Total: 15s / Total: 15 seconds; 12-second / 12 second / 12 seconds;
15s total; 0–3s / 12-15s / 12—15s; 3s to 7s. Spaces/tabs may separate tokens but
newlines cannot manufacture phrases. Unsigned ASCII decimal syntax only, with digits
on both sides of a decimal point. Fullwidth digits fold first. Reject signed numbers,
exponents, thousands separators, malformed decimals, embedded alphanumeric tokens,
clock tails, reversed spans, and any candidate overlapping a paired quote.
A rejected candidate must not suppress a later independent valid claim or allow a
partial tail to be rescued. Parse finite values only; keep the maximum eligible claim
without collecting all matches. Prompt claims may exceed 600 (e.g. 900s).

Fire only if maximum claimed seconds > supplied duration. No under-claim warning or
percentage tolerance. Video MCP duration is integer; fractional supplied values are
deliberate schema-invalid tolerance tests, not canonical accepted inputs.
Message: `prompt describes 15s of action but the supplied duration is 5s. Check the
model's supported duration or retime the beats.` Never promise effective duration or
guaranteed truncation: the backend may clamp/adjust. Format finite numbers without
unchecked integer casts; bound huge finite displays.

Accepted limitations: worded numbers, frames, colon-less totals, secs, default/inherited
duration, semantic scene-vs-total distinctions and negated timing. "holds for 12 seconds"
against 5 can fire; track this as a known contextual ambiguity.

## 6. Output contract

Findings are returned as one `additionalContext` string. No finding means exit 0 with
empty stdout. For each entry the order is resolution, local conflict, duration.
Batch findings retain entry order and prefix their messages with `request <index>:`.

This example is the reconciled historical Kling fixture (no output-size assertion):

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "additionalContext": "higgsfield-lint: 1 finding\n- prompt_negative_conflict: \"slow motion\" appears in both prompt and negative_prompt. Keep each term in one."
  }
}
```

Each finding is one line: `<check_id>: <observed contradiction>. <advice>.`

### 6.1 Command-line interface (robot mode)

Added 2026-09-10; design in
[the robot-mode spec](docs/superpowers/specs/2026-09-10-robot-mode-cli-design.md).
The same binary is a CLI for humans and AI coding agents. Dispatch is decided before
stdin is touched:

| argv | stdin | path |
|---|---|---|
| any argument, recognised or not | not consulted | CLI |
| none | a terminal | quick start, text, exit 0 |
| none | not a terminal, 0 bytes | quick start (JSON when stdout is piped), exit 0 |
| none | not a terminal, 1 byte or more | hook (§6, §7), unchanged |

Claude Code always writes the event JSON to a hook's stdin, so the 0-byte row is never
a hook invocation. Any argument is the CLI: a stray argument in `.claude/settings.json`
would exit 2 and block every call, which is why doctor's matcher-present check fails on
a command that carries one.

Commands: `lint [FILE|-] --tool image|video|audio`, `doctor`, `checks`, `version`,
`help [COMMAND]`; aliases `-h`/`--help`, `-V`/`--version`. Output is JSON when stdout
is not a terminal or `--json` is given, text when stdout is a terminal or `--text` is
given; both flags together is a usage error. No ANSI, ever. JSON mode writes one
newline-terminated document per invocation to stdout and nothing to stderr, errors
included; text mode writes errors to stderr. JSON key order is not significant.

Exit codes: 0 ok (clean lint, every doctor check passed, informational commands);
1 negative result (lint findings, doctor FAIL); 2 usage; 3 input; 4 internal (a caught
panic). The hook path stays exit 0 unconditionally.

Every document carries `ok` and `exit`. Errors are
`{"ok":false,"exit":N,"error":{"code","message","suggestions":[...]}}`. Codes by exit
class: `unknown_command`, `unknown_flag`, `unexpected_argument`, `conflicting_flags`,
`missing_value`, `missing_input`, `missing_tool`, `unknown_tool` (2); `file_not_found`,
`read_failed`, `invalid_utf8`, `invalid_json`, `unsupported_shape` (a non-object document,
or a `tool_input` that is not an object) (3); `internal` (4).

`lint` reads FILE, `-`, or stdin (a terminal on stdin with no FILE is `missing_input`,
not a hang). `--tool T` (or `--tool=T`) takes `image`, `video`, `audio` or a tool name in
either style §3's matcher accepts, and overrides a payload `tool_name`. Three document shapes are
accepted, tested in this order and echoed back as `shape`: `event` (has `tool_input`;
tool from `--tool`, else `tool_name`), `tool_input` (has `params` or `requests`; needs
`--tool`), `params` (any other object; needs `--tool`). Output is
`{ok, exit, tool, shape, ran, findings:[{check, message, request?}], note?}`: `ran`
lists the checks that apply to the tool kind, in §6 order, so an empty `findings` reads
as "these checks passed" (audio runs none and says so in `note`); `message` is the §6
finding line without its `- <check_id>: ` prefix; `request` is the batch index the hook
would prefix. Text mode prints the hook's rendering, including the
`higgsfield-lint: 0 findings` header when clean, where the hook itself prints nothing.

`doctor` text output is unchanged; its JSON is `{ok, exit, checks:[{name, status:
pass|warn|fail, detail}], summary:{pass, warn, fail}, note}`. `checks` describes each
live check (`id, tools, fires, fix, silent_when`) plus the conflict phrase list and the
duration model list. `help` alone is the quick start, held to about 100 tokens by test;
`help COMMAND` renders usage, arguments, flags, input, output, exit codes and examples
from one table in both formats. `rust/tests/cli.rs` pins all of this at process level.

## 7. Failure behaviour

Normal hook execution exits 0 and writes no stderr, including malformed JSON, invalid
UTF-8, missing/unusable fields and caught Rust panics. No finding means no stdout.
Missing one field can suppress its rule without suppressing other valid rules.
One deliberate exception since 2026-09-10: with no arguments and 0 bytes on a
non-terminal stdin, which Claude Code never produces, the binary prints the §6.1 quick
start and still exits 0 with empty stderr. Any non-empty stdin, a lone newline included,
stays on the silent path.

The hook makes no network/MCP calls, application data-file reads, or filesystem writes.
The two direct crate dependencies, serde_json and regex, are compiled into the executable;
no runtime interpreter or dependency fetching is required. Platform system libraries
may still be dynamically linked. The CLI (§6.1), doctor included, is a separate explicit
path that reads local files, prints reports and can exit nonzero. OS termination, out-of-memory
abort, broken output pipes and a missing executable cannot be turned into a universal
successful-execution guarantee by catch_unwind.

## 8. Credit expiry reminder

Separate proposed operational workflow; no reminder was created or changed by this
implementation. See [credit-expiry task](docs/credit-expiry-task.md).
The historical six resets are 30 days apart, not fixed calendar-month dates.
Re-anchor any reminder on fresh transactions, then query balance; the old August
balance/reset examples are not current account state.

## 9. Testing and authority

SPEC defines intended local behavior; the dated MCP capture defines documented provider
fields/options. Rust is the sole executable implementation. Goldens record reviewed
behavior, not independent proof that provider semantics are correct. Archived Python
is historical evidence, never a live oracle or runtime fallback.

Captured inputs remain unchanged: the Kling fixture produces only the local
prompt_negative_conflict; Seedance stays silent. Confirmed model-scoped synthetic
controls separately prove each active rule, avoiding fixture expectations based on
undeclared dimensions.

The six captured tool schemas differ: the three single-generation tools accept
`params` as object or JSON-encoded string; batch entries declare object params.
The hook tolerates JSON-object strings in batches as an explicit fail-open extension,
not a claim that the server schema accepts them. A nonempty requests list prevents
top-level fallback; an empty list may fall back to params within tool_input.

Run `cargo test --release`, plus `cargo test --test fail_silent` for genuine debug panic
injection. `rust/tests/cli.rs` pins the §6.1 CLI contract at process level, in both
output formats and every exit class; its debug run also proves a CLI panic becomes
exit 4. Release contains no magic-field panic behavior. Unit, doctor and process
tests verify exact messages, scopes, source-aware carriers, normalization, duration
grammar, malformed inputs, fixture loading and build freshness. A deterministic
comparison-count guard tests production quote lookup, separately from latency.

The golden corpus currently contains 538 distinct inputs with input lengths and
FNV-1a hashes, compared as parsed output JSON plus exact exit/status/stderr requirements.
346 of the 347 historical inputs retain their original bytes (`pathological:empty` became
`pathological:whitespace_only` on 2026-09-10, §6.1 and §7); 87 deliberately revised outputs
lose only unsupported resolution findings. The 191 new semantic cases have independently
authored expected ids and messages, mandatory-negative/positive/robustness/limitation
classes, and selected repeated-output checks. See the
[golden guide and research mapping](rust/tests/golden/README.md),
[per-label change ledger](docs/golden-change-ledger.json), and
[implementation report](docs/rule-regrounding-implementation-report.md).
Adding a case requires deciding its expected behavior before reviewing actual output.
There is no bulk acceptance/regeneration command. The optional audit test is read-only.

## 10. Repository layout

```
SPEC.md                              this document
README.md                            setup/build steps
Cargo.toml, Cargo.lock               Rust package manifest (serde_json, regex)
rust/src/*.rs                        the hook, compiled to a Rust binary (§12)
rust/src/cli.rs                      robot-mode CLI: lint, doctor, checks, version, help (§6.1)
target/release/higgsfield-lint       the built hook; PreToolUse runs this directly (§12.1)
data/.gitkeep                        placeholder; negative_prompt_models.json retired here (§5.3)
tests/fixtures/*.json                real captured tool inputs
rust/tests/parity.rs                 golden-output regression corpus, the port's real verification (§9)
rust/tests/golden/expectations.json  recorded expectations rust/tests/parity.rs checks against (§9)
rust/tests/golden/README.md          how to add/change a golden entry (§9)
rust/tests/fail_silent.rs            pathological-input and panic-silence tests
rust/tests/cli.rs                    robot-mode CLI contract tests (§6.1)
docs/archive/                        the rejected design, its pseudocode, and the retired Python port
docs/archive/verified-port/          archived Python port + pytest suite - historical record (§12.2)
docs/superpowers/plans/              implementation plans
docs/superpowers/specs/              design specs (robot-mode CLI)
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

The completed migration reversed that decision. The then-existing checks were ported from
`docs/archive/verified-port/higgsfield_lint.py` to a compiled Rust binary, proven output-equivalent as parsed
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
from "the lint ran and found nothing" on the silent hook path. The opt-in doctor
now detects missing/stale local build inputs and runs fixture-backed checks. It cannot
prove that a separate already-open Claude session loaded this hook.

Consequence: **`cargo build --release` is a required setup step**, not a one-time
convenience. Rerun it whenever any recursive rust/src source, Cargo.toml or Cargo.lock changes,
then run `target/release/higgsfield-lint doctor`. Freshness is an mtime heuristic, not
a content-hash proof. Inspect `/hooks` and a target-session tool trace before claiming
that the hook is loaded there; this implementation did not establish that session proof.

### 12.2 `docs/archive/verified-port/higgsfield_lint.py` is archived, not deleted

The port was verified: 320 differential inputs / 0 divergences, plus 4,200
independently-fuzzed inputs from a separate reviewer (`docs/archive/verified-port/
README.md` has the full record and the commit it was true at). That job is finished,
and every rule change from here on would otherwise need implementing twice - once in
Rust, once in a Python file kept alive for nothing but comparison. So
the Python port and its pytest suite were moved to
`docs/archive/verified-port/`, retained as a **historical record**, not deleted.

It was never a runtime fallback - nothing in the hook path ever called it, and the Rust
binary's own fail-silent behaviour (§7) is what runs on any internal error, not the
Python - so archiving it changes nothing about the hook itself. What changes is the test
harness: `rust/tests/parity.rs` no longer runs a second live process at all. It compares
the compiled binary's output against recorded expectations in
`rust/tests/golden/expectations.json`, captured from the Rust binary at the moment the
Python comparison last passed. Rust is the sole implementation; SPEC and canonical evidence govern intended behavior.
The golden file, not archived Python, is the regression instrument going forward (§9).

The archived files must not be edited to match anything that changes here afterward;
doing so would misrepresent them as still-verified current behaviour, which they are not.

### 12.3 Known divergence: compound `index` values (historical)

`index` in a batch request is schema-typed as an integer. If a caller passes a JSON array
or object as `index` anyway, which only happens on a malformed call, the archived
Python's `str()` rendered it in Python repr form (`[1, 2]` with a space, `{'a': 1}` with
single quotes), while the Rust port's `serde_json` `Display` renders `[1,2]` and
`{"a":1}`, with no spaces and double quotes. This was a deliberate, accepted divergence
from the Python, not a bug: emulating Python's `repr()` for arbitrarily nested JSON in
Rust, to cover a case that only arises from a malformed call, would have made the port
look like parity without being it.

The comparison this reasoning came from is historical now that the Python is archived
(§12.2), but the corpus design it produced is still current: `rust/tests/parity.rs`
deliberately generates scalar indices only, and `rust/tests/golden/expectations.json`
records expectations only for those. See the comment at `format_index` in
`rust/src/lint.rs`.

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
