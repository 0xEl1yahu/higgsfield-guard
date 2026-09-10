# Main-branch regression gates

`Lint CI` runs on every pull request targeting `main`, every push to `main`, merge
queue groups, and manual dispatch. There are no path filters: docs, fixtures and
workflow edits can affect the contract too. No production credentials or generation
calls are used. Jobs have read-only repository access, checkout does not retain its
credential, and third-party actions are pinned to commit SHAs.

## What “bad” and “good” mean

This is an advisory lint, not a request validator or blocking gateway. A bad supported
claim must produce its specified findings; a supported clean counterexample must be
silent. Both exit 0 with empty stderr. Malformed/unusable inputs must fail silently;
one malformed field must not hide an independent valid finding. A finding never
contains `permissionDecision: deny`. Blocking bad requests would be a separate product
contract change, not something CI should silently introduce.

The suite is not happy-path-only. Each positive witness is paired with an equality,
negation, quote, unsupported-context or malformed counterexample so unconditional
silence and indiscriminate warnings both fail.

## Gates

| Job | Failure modes covered |
|---|---|
| Format, Clippy and workflow lint | Formatting drift, compiler warnings, invalid workflow expressions/syntax |
| Contracts (Ubuntu 24.04 and macOS 14) | Full release suite, 538 golden inputs, independent semantic classes, exact envelopes, research mapping, doctor, robot-mode CLI contract, debug panic injection and hostile process scenarios |
| Tests detect deliberately broken lint | Nine isolated production mutations must fail their specific named tests; compile errors, no selected tests, and command timeouts do not count |
| CI required | Fails unless every prerequisite succeeds, including all OS matrix legs; skipped/cancelled jobs cannot become green |

Rust 1.98.0 is pinned in the workflow; all Cargo build/test/lint commands use the lockfile.
Ubuntu and macOS are the supported CI platforms here; Windows behavior is not claimed.
No build cache is restored, avoiding a timestamp-based doctor false pass/failure on
reused binaries. The local verification script also cleans only this package's release
artifacts before rebuilding, because Cargo may otherwise reuse a binary after a
test-only Cargo.toml change. No source or fixture is deleted.

## Adversarial scenarios

`rust/tests/adversarial.rs` independently checks the real process, without deriving
expected rule ids from the golden file:

- Bad/good pairs across image/video resolution, quality, explicit target dimensions,
  negative-side negation and duration equality.
- Prompt text containing fake permission JSON, instruction overrides, shell syntax,
  control characters, bidi markers and Unicode cannot control output. A shell-expansion
  sentinel in a unique temporary directory must remain absent (no command execution).
- Quoted/fullwidth claims, glued tokens, malformed signed/exponent/comma/decimal/range
  timing and quoted dialogue stay silent; a later independent valid claim still fires.
- Audio, wrong tool family, lookalike names and unknown/malformed models cannot invent
  output controls, while local conflicts remain independently observable where allowed.
- A 7×7 invalid resolution/duration cross-product must still detect an independent
  conflict. Invalid UTF-8, truncated/concatenated JSON, nonfinite and deeply nested JSON
  stay silent.
- Malformed batch entries cannot mask a valid neighbor or trigger top-level fallback;
  request numbering and the empty-list exception remain explicit.
- Large quote-heavy Unicode prompts stay silent until an unquoted live claim is added.

The new process helper drains stdin/stdout/stderr concurrently and kills/reaps its own
child after 20 seconds. This is a generous hang guard, not a performance SLO. Existing
golden/doctor processes also have the enclosing CI job timeout. Wall-clock percentiles
remain measured evidence, not fragile cross-runner assertions.

## Robot-mode CLI contract

`rust/tests/cli.rs` spawns the release binary with piped stdio, an agent's exact
environment, and pins SPEC.md §6.1: every command in both output formats, every exit
class, structured errors, the three `lint` document shapes, the 0-byte quick start and
the one-byte boundary that keeps a lone newline on the silent hook path. Its debug run
also proves a CLI panic becomes exit 4 with an `internal` error. Three doctor
assertions need the release binary path that `.claude/settings.json` names and are
compiled out of debug builds, matching `ci/verify.sh`.

## Mutation evidence and corpus integrity

Nine regressions: always-silent output, removed audio scope, inverted duration comparison,
ignored resolution quotes, missing negative-side negation stripping, ignored batch
branch, linear quote lookup, noisy panic hook, and uncaught panic.

`ci/mutation_checks.py` is a Python-standard-library **CI driver only**. It is not a
restored Python hook or an independent behavior oracle. It copies the relevant sources
and fixtures into an owned temporary directory, verifies each selected test passes
unmodified, substitutes one uniquely matching source fragment, and requires exactly
that Rust test to fail. The outcome classifier self-tests rejection of compile errors,
wrong test names and empty test selections. Temporary mutations never touch the checkout.
Refactors that invalidate an anchor fail the gate and require explicit probe updates.

Semantic minimum counts prevent deleting/reclassifying controls into a vacuous green
suite: 191 cases, 59 negatives, 92/9/16 distinct-input positives by rule, 17 robustness
cases, three known limitations, and 13 determinism controls. Golden coverage cannot
silently shrink below 538. Expected-message and input-hash comparisons remain mandatory;
there is no auto-update switch. New cases may increase these minima; deliberate
reductions require review. Known limitations stay labeled, not counted as successes.

Logs (including named failures for killed mutants) and a machine-readable mutation
summary are uploaded even on job failure and retained for seven days. No live provider
requests are made. Nine selected mutation operators are useful regression evidence,
not a claim of exhaustive fuzzing, complete mutation coverage, or schema validation.

## Run locally

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
bash ci/verify.sh   # ends with `doctor --text`: a piped stdout would otherwise select JSON
python3 ci/mutation_checks.py --output target/ci-mutations
```

Use Rust 1.98.0 to reproduce the CI toolchain. The mutation driver requires Python 3.9+;
workflow validation uses actionlint 1.7.12 (installed by Go in the Ubuntu quality job).

## Main protection and rollout

The workflow takes effect for PRs immediately and for `main` after merging it. The
stable check to require in a main-branch ruleset is **CI required**. Merely creating
the workflow does not enforce merge blocking: `main` was unprotected when this change
was prepared. This implementation does not merge its own PR, change repository
visibility, or rewrite branch-protection settings. Required-check enforcement needs
the repository owner to enable that check; no passing check should be claimed before
the first hosted run completes.

GitHub references: [workflow events](https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows)
and [secure workflow use](https://docs.github.com/en/actions/reference/security/secure-use).
