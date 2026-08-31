# The retired Python oracle

This directory is a **historical record**, not live code. It must not be treated as
current behaviour, and must not be edited to match anything that changes in `rust/` after
the date below - including reverting an edit made "to fix parity," since there is no
parity check running against it any more.

## What this was

`higgsfield_lint.py` was the original implementation of the `higgsfield-lint` hook,
later ported to Rust. After the port, it was kept on deliberately as a read-only
**oracle**: `rust/tests/parity.rs` ran both implementations against an identical corpus
of stdin payloads and asserted their parsed-JSON outputs were equal, on the theory that
a hand-written Python-to-Rust port is exactly the kind of change most likely to
introduce silent, subtle divergences. `test_lint.py` was its pytest suite, exercising the
Python implementation's checks directly.

## What it verified, and when

- **320 distinct inputs, 0 divergences** - the deterministic differential corpus in
  `rust/tests/parity.rs` (real fixtures, pathological inputs, and a generated sweep
  across every lexicon entry, dimension pair, and negation form). This was the harness's
  standing regression gate throughout the Rust port's development and review.
- **4,200 independently-generated randomized inputs, 0 divergences (849 firing)** - a
  one-time fuzz run performed by an independent Opus-tier reviewer during the Rust
  port's final whole-branch review, alongside panic-injection through both the
  single-call and batch paths and 57 out-of-corpus edge-case probes (13 divergences,
  all traced to schema-invalid inputs, not real bugs). Documented in
  `.superpowers/sdd/2026-08-31-rust-port/progress.md`, "Final whole-branch review"
  section. This fuzzer itself was never checked into the repository, so this 4,200
  figure is a reported, one-time verification event, not something `cargo test` can
  re-run.

**Commit this was true at:** `a8779c8` (`test(doctor): cover the CLAUDE_PROJECT_DIR
wiring site with subprocess tests`) is the last commit of the predecessor
`doctor-and-expiry` plan on this branch, and the differential result was re-confirmed
directly, by hand, at this task's actual starting commit,
`77df1603e072731a46b7036e8155008e5880d136` (two documentation-only commits past
`a8779c8`, neither of which touched `src/higgsfield_lint.py` or the parity corpus):

```
$ cargo test --release --test parity -- --nocapture
parity harness: compared 320 distinct inputs against the Python oracle - 0 divergence(s)
test rust_matches_python_oracle_across_the_corpus ... ok
```

## Why it was retired

By the commit above, the port was finished and proven: 320/0 plus the reviewer's 4,200/0.
The plan that retired it (`docs/superpowers/plans/2026-08-31-rule-regrounding.md`)
deliberately changes rule *behaviour* in every task after this one. Keeping a second,
hand-maintained implementation around would mean either implementing every behaviour
change twice (in Rust and in Python) or letting this file silently rot into a stale
"oracle" that no longer describes anything real. Neither is acceptable, so it was
archived instead of deleted - the history stays inspectable, but nothing reads it as
ground truth any more.

`rust/tests/parity.rs` was converted in the same change to compare the compiled Rust
binary against recorded expectations in `rust/tests/golden/expectations.json` (captured
from this exact Python-verified state) instead of against a second live process. **The
Rust is now the specification.** `SPEC.md` and `README.md` at the repository root have
been updated accordingly.

## What is NOT here

`tests/fixtures/*.json` (the real captured tool inputs) did **not** move here. They are
general-purpose fixture data, not Python-specific: `rust/tests/parity.rs`'s corpus
generator reads them directly from `tests/fixtures/` to build part of the golden
corpus, and several of this plan's later tasks reference them by that same path. Moving
them into this archive would have broken the Rust test suite for no benefit - only the
Python-specific files (`higgsfield_lint.py`, `test_lint.py`) are archived here.
