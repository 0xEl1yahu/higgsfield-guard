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
    "additionalContext": "higgsfield-lint: 2 findings\n- resolution_claim: prompt says \"4K\" but width 1376 x height 768 gives a 768px short side. Set resolution, or drop the claim from the prompt.\n- prompt_negative_conflict: \"slow motion\" appears in both prompt and negative_prompt. Keep it in one."
  }
}
```

Each finding is one line: `<check_id>: <what was observed>. <mechanical fix>.`

## 7. Failure behaviour

The hook is in the path of every Higgsfield generation call, so its own failure must be
invisible.

- Any exception, malformed stdin, or missing field exits 0 with no output.
- The hook never writes to stderr on the success path, since exit 2 plus stderr would block.
- No network calls, no MCP access, no filesystem writes beyond reading its own data file.
- Standard library only. A dependency that fails to import would break every generation call.

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

## 10. Repository layout

```
SPEC.md                              this document
src/higgsfield_lint.py               the hook, stdlib only
data/negative_prompt_models.json     evidence-seeded model list
tests/fixtures/*.json                real captured tool inputs
tests/test_lint.py                   pytest
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

## 12. What would bring the big design back

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
