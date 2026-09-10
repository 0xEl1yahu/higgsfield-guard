# Lint precision review

**Scheduled:** one-time, 2026-10-08 09:00 Europe/London, as Superset automation
`6f69c31b-046a-439d-a62f-07d541ca805d`. Instrument: `higgsfield-lint-review` (SPEC 9.2).

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

## How to get precision

The review tool reports fire **rates**, not precision - it reads a log that deliberately
holds no prompt text (SPEC §9.1). Precision needs a human judging each firing against the
prompt that caused it:

1. Take the `ts` of each firing from `data/findings.jsonl`.
2. Pull the matching calls with the Higgsfield `show_generations` tool.
3. Judge each firing by hand.

That is the same method SPEC §2 used to derive all three checks from nine real
generations. It is not a fallback; it is the primary instrument, and the log exists to
tell you which calls are worth pulling.

