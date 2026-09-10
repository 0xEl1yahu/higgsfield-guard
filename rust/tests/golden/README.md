# `expectations.json`

This is the golden-output regression corpus `rust/tests/parity.rs` checks the compiled
binary against (`SPEC.md` §9 has the policy: entries are updated in the same change that
causes them to flip, individually, never bulk-regenerated). This file is the mechanism.

## The four fields on every entry

```json
{
  "label": "edge:negative_prompt_whitespace_only",
  "input_len": 61,
  "input_fnv1a64": "3a1c9e2b7f0d4468",
  "output": null
}
```

- **`label`** - must match the label the corpus generator in `rust/tests/parity.rs`
  produces for this case. A label the generator no longer produces is reported as an
  `ORPHANED GOLDEN ENTRY`; a generated label with no matching entry here is a `MISSING
  GOLDEN ENTRY`.
- **`input_len`** and **`input_fnv1a64`** - the byte length and FNV-1a hash of the exact
  stdin bytes the generator built for this label at capture time. These exist to catch
  drift in the *generator*, not the checks: if someone edits a sweep function so a label
  now produces different bytes without recapturing, that shows up as its own failure
  category, separate from a real behaviour change.
- **`output`** - the expected parsed-JSON stdout, or `null` if the hook must stay silent
  for this input.

## There is deliberately no regeneration tool

Nothing in this repository can bulk-rewrite this file from the binary's current output.
That absence is intentional: an `UPDATE_GOLDEN=1`-style escape hatch would be reached for
the moment a diff went red, which would turn this corpus from a reviewed record of
behaviour into a rubber stamp for whatever the binary happens to do today. Every entry
here is meant to be typed or pasted in by a human who has looked at the diff and decided
it is correct.

## Adding or changing an entry

There is no capture tool, so the workflow is: add a placeholder, run the test once, and
read the correct values out of the failure message.

1. Add (or edit) the corpus case in the relevant generator function in
   `rust/tests/parity.rs`, under whatever label it should carry.
2. Add an entry here for that label with placeholder values, e.g.
   `"input_len": 0, "input_fnv1a64": "0", "output": null`.
3. Run `cargo test --release --test parity -- --nocapture`. It will fail - either as a
   `MISSING GOLDEN ENTRY` (if you skipped step 2) or a `GOLDEN MISMATCH` (if you did step
   2) - and the failure message names the real `input_len`/`input_fnv1a64` the generator
   produced and shows the actual output.
4. Copy those real values into the entry, review the output for correctness (this is the
   step that makes it a deliberate, reviewed expectation rather than a rubber stamp), and
   re-run to confirm it now passes.

Removing a label from the corpus generator without removing its entry here leaves an
`ORPHANED GOLDEN ENTRY` - delete the entry in the same change.

## Canonical revision and semantic controls (2026-09-09)

The corpus has 538 distinct inputs: 347 preserved historical inputs and 191 added
canonical semantic cases. All original input lengths and hashes are unchanged. Of the
old outputs, 87 lose only unsupported resolution advice. The complete
[change ledger](../../../docs/golden-change-ledger.json) records each before/after value
and reason. No captured fixture was rewritten.

`semantic-cases.json` stores independently authored ids, exact context, class and source
references. Assertions do not derive semantic truth from golden output. Labels are
counted once per rule even if two batch entries produce that rule. Known limitations
record actual and ideal results separately. The test asserts 59 mandatory negatives
and reports positives, limitations and selected repeated-process determinism.

`audit_golden_changes` is an ignored, read-only mismatch inspector, not an updater.
Decide expected behavior from SPEC/evidence before using any actual output in review.

## Robot-mode CLI (2026-09-10)

One historical input left the corpus: `pathological:empty` (0 bytes). With no arguments
a 0-byte stdin is now the quick-start document (SPEC.md §6.1, §7) - Claude Code always
writes the event JSON, so it was never a hook input - and `rust/tests/cli.rs` owns that
expectation. Its replacement `pathological:whitespace_only` (a single newline, still
silent) pins the exact boundary of that branch. Count stays at 538; the
[change ledger](../../../docs/golden-change-ledger.json) records both entries.

## Research §12 mapping

All 65 research identifiers appear below. “Covered” includes explicitly identified
equivalent/representative inputs, not a claim that every abbreviated research example
has been captured verbatim. Full cowboy/drama templates remain uncollected; their
inline-exclusion risk is represented by the controls shown. Deferred rules are excluded
from shipped-rule positive metrics. None of the research identifiers is assigned to the
three additional accepted limitations (negated resolution, scene-duration language,
inch-mark quote pairing); those are separate semantic cases with actual/ideal ids.
The table is validated by the parity suite against corpus labels and named test functions.

| Research ID | Status | Test label or deferred rule | Scope / changed expectation |
|---|---|---|---|
| F-001 | covered | `fixture:seedance_tree_clean.json:single` | Captured input unchanged; batch counterpart also tested. |
| F-002 | covered | `fp_control:negation_placement:higgsfield_style_header` | Equivalent house-style/exclusion control, not a full 17-block prompt capture. |
| F-003 | covered | `unit:resolution_claim_domains_sources_and_exclusions` | Equivalent Nano Banana 4k equality control, not an exact Samurai text capture. Pure helper property; process controls cover dispatch separately. |
| F-004 | covered | `fp_control:negation_placement:seed_list_only_no_negative_prompt` | Equivalent inline-exclusion control, not a full drama-template capture. |
| F-005 | covered | `canonical:scope:audio` | Stronger audio skip control with all three would-be findings; batch also tested. |
| F-006 | covered | `canonical:duration:lens` | Equivalent clean technical prompt with no supported duration claim. |
| F-101 | covered | `fixture:kling2_6_4k_slowmo.json:single` | Reconciled: conflict only; stored dimensions no longer assert render size. |
| F-102 | deferred | `HFR-002` | Model-ceiling inference remains deferred. |
| F-103 | covered | `dims:1376x768:obj` | Equivalent bare-dimensions case; corrected expectation is silence, not an output guarantee. |
| F-201 | covered | `canonical:duration:total` | Total grammar with known eligible model added; raw model-less research input now silent. |
| F-202 | covered | `canonical:duration:compound` | Compound grammar with eligible model; not generic missing-model inference. |
| F-203 | covered | `canonical:duration:spans` | Multiple time ranges with eligible model; maximum endpoint. |
| F-301 | deferred | `HFR-003` | Aspect-ratio claim comparison deferred. |
| F-302 | deferred | `HFR-003` | Ratio inference from generic dimensions unproven and deferred. |
| F-401 | covered | `canonical:text:8k` | Supported model and resolution keep negative control discriminating. |
| F-402 | covered | `canonical:text:quoted` | Verified output tier replaces unsupported bare dimensions. |
| F-403 | covered | `fp_control:prompt_negative_conflict:cast_iron_pan` | Cookware homograph. |
| F-404 | covered | `fp_control:prompt_negative_conflict:sharp_focus_soft_focus` | Homograph drop only; handheld_survivor_negation_strip separately proves negative-side stripping. |
| F-405 | covered | `fp_control:prompt_negative_conflict:quoted_literal_text` | Bare text removed; quoted_survivor_term separately proves quote exclusion. |
| F-406 | covered | `fp_control:negation_placement:quoted_literal` | Placement rule disabled. |
| F-407 | covered | `fp_control:negation_placement:scene_description` | Placement rule disabled. |
| F-408 | covered | `fp_control:negation_placement:scene_description` | Equivalent scene-description control; placement disabled, no generic negation warnings. |
| F-409 | covered | `unit:is_placement_noise_flags_quantifier_idioms_and_without_gerund_only` | Retained quantifier helper only, not an active placement rule. Pure helper property; process controls cover dispatch separately. |
| F-410 | covered | `unit:is_placement_noise_flags_quantifier_idioms_and_without_gerund_only` | Retained gerund helper only, not an active placement rule. Pure helper property; process controls cover dispatch separately. |
| F-411 | covered | `gerund_directive:no flickering:model=Some("kling2_6")` | Corrected expectation is silence: disabled placement does not resume for directive verbs. |
| F-412 | deferred | `HFR-003` | Exactly-one-ratio guard belongs to deferred ratio rule. |
| F-413 | deferred | `HFR-003` | Potential ratio false positive documented, not a shipped-rule accepted finding. |
| F-414 | covered | `canonical:duration:under` | Equivalent explicit under-claim control. |
| F-415 | deferred | `HFR-003` | Three-part ratio token exclusion belongs to deferred ratio rule. |
| F-416 | covered | `canonical:duration:lens` | Lens/aperture numbers are not durations. |
| F-501 | covered | `unit:canonical_allowlists_match_saved_mcp_not_cli_catalog` | Seedance 4k option recognized without geometry inference; equality tested separately. Pure helper property; process controls cover dispatch separately. |
| F-502 | covered | `canonical:resolution:nano_banana_pro` | Lower nominal tier fires; exact 2k case also unit-tested. |
| F-503 | covered | `canonical:resolution:minimax_h3` | Uppercase 2K carrier fires. |
| F-504 | covered | `canonical:carrier:wan2_6` | quality source identified in message. |
| F-505 | covered | `canonical:carrier:kling3_0` | Matching 4k mode stays silent. |
| F-506 | deferred | `HFR-004` | Constraint finding deferred; canonical:excluded:fast suppresses unsupported output assertions now. |
| F-507 | deferred | `HFR-007` | Effective-duration/clamping validation deferred. |
| F-508 | covered | `canonical:excluded:unknown` | Unknown model cannot establish output control. |
| F-601 | covered | `edge:unicode_prompt` | Equivalent multilingual robustness. Bare dimensions now silent; canonical:text:fullwidth proves supported carrier separately. |
| F-602 | covered | `edge:unicode_prompt` | Unicode robustness representative; not a full bidi rendering claim. |
| F-603 | covered | `canonical:text:fullwidth` | Now supported, original spelling preserved. |
| F-604 | covered | `unit:has_term_resumes_scanning_past_a_rejected_match` | Boundary scanner rejects embedded tokens and continues scanning. Pure helper property; process controls cover dispatch separately. |
| F-605 | covered | `unit:repeated_quoted_prompt_is_silent_but_final_live_occurrence_fires` | Scaling functionality plus separate multi-MB benchmark; historical 250ms is not a portable assertion. Pure helper property; process controls cover dispatch separately. |
| F-606 | covered | `pathological:whitespace_only` | Whitespace-only stdin silence. A 0-byte stdin with no arguments is the robot quick start (SPEC.md §6.1), pinned in `rust/tests/cli.rs`, not a hook input. |
| F-607 | covered | `pathological:empty_object` | Missing envelope silence. |
| F-608 | covered | `pathological:tool_input_null` | Null envelope silence. |
| F-609 | covered | `edge:no_prompt_at_all` | Equivalent missing-params/prompt tolerance; explicit null also dispatch unit-tested. |
| F-610 | covered | `pathological:params_bare_string` | Invalid JSON string params silence. |
| F-611 | covered | `dims:1376x768:str` | Equivalent string params; corrected bare-dimensions silence, not parser failure. |
| F-612 | covered | `edge:prompt_non_string` | Malformed prompt silence. |
| F-613 | covered | `canonical:batch:empty-fallback` | Empty requests falls back to params inside tool_input, not outer payload params. |
| F-614 | covered | `canonical:batch:no-fallback` | Nonempty requests prevents fallback. |
| F-615 | covered | `canonical:batch:explicit-mixed` | 10/20/30 explicit indices; only 20 offends. |
| F-616 | covered | `canonical:batch:positional` | Positional request index. |
| F-617 | covered | `dims:asymmetric_bool` | Bare dimensions unmapped; canonical target-invalid:true is discriminating typed-carrier control. |
| F-618 | covered | `dims:16x9:obj` | Bare dimensions unmapped, not an active size floor. |
| F-619 | covered | `dims:1920x100:obj` | Now silent for bare dimensions; canonical:targets:topaz_image tests actual asymmetric target fields. |
| F-620 | covered | `dims:floats_excluded` | Bare dimensions unmapped; typed target float exclusion additionally unit-tested. |
| F-621 | covered | `pathological:deeply_nested_array` | Recursion-depth rejection; wide 4MB array benchmark separately tests allocation. |
| F-622 | covered | `pathological:invalid_utf8` | Invalid stdin encoding silence. |
| F-623 | covered | `fp_control:prompt_negative_conflict:whitespace_only_negative_prompt` | Empty companion control also retained. |
| F-624 | covered | `unit:an_internal_panic_is_still_silent_and_zero` | Named process integration test in fail_silent.rs, run in debug; release strips injection. |
| F-701 | covered | `canonical:overlap:handheld` | Complementary camera directions. |
| F-702 | covered | `canonical:overlap:grain` | Near-miss vocabulary. |
| F-703 | covered | `canonical:conflict:wide` | Genuine wide-shot conflict retained. |

Counts: 57 covered (including representative/helper coverage), 8 deferred, 0 accepted
research cases = 65. These are traceability counts, not overall product recall.
