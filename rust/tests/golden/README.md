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
