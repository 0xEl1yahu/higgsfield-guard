# Pre-Call Prediction & Agent Budgeting — Open Questions

**Status:** TODO — exploratory. Nothing here is decided, scoped, or scheduled.
**Raised:** 2026-08-30
**Premise:** Before issuing an LLM call, estimate its odds of success (or preview its
result), and execute only if the call is aligned and worth its cost. Goal is token
savings and principled agent budgeting.

---

## TODO: Foundational objection — is prediction self-defeating?

Any predictor accurate enough to tell you what a call would return has effectively
done the call's work. Previewing the *result* looks like a dead end.

- [ ] TODO: Confirm the asymmetry that rescues this — predicting properties of the
      outcome *distribution* (especially variance) is much cheaper than predicting its
      mean. "Will the model be all over the place on this?" is answerable from prompt
      features alone.
- [ ] TODO: Establish which cheap prompt features actually correlate with outcome
      variance (ambiguity markers, distinct entity count, verifiability of the goal).

## TODO: Is P(success) even well-defined without a verifier?

This is the central question. If you have a verifier good enough to score outcomes, you
often have enough to just retry cheaply and skip prediction altogether.

- [ ] TODO: The value of any gate is bounded by the cost of the verifier that trains and
      validates it. Work out whether that bound ever leaves room for profit.
- [ ] TODO: Decide whether "success" is defined per-call or only per-trajectory.

## TODO: Deterministic checks vs. learned prediction

Hypothesis: the cheapest wins are boring and deterministic, and they dominate the
learned-router savings. Ranked by cost of the check:

- [ ] TODO: **Preconditions** — path doesn't exist, auth missing, symbol not in repo.
      Free to check, P(success) genuinely ~0. Suspected to be most of the real waste.
- [ ] TODO: **Context sufficiency** — if the needed information isn't in the window, no
      model tier fixes it. How cheaply can retrieval adequacy be checked?
- [ ] TODO: **Duplicate/redundant call detection** — agents re-ask what they already
      know. Semantic dedup against the trajectory is embedding-cost, not inference-cost.
- [ ] TODO: **Empirical priors** — "failed this shape of task 3x in this repo in the last
      hour." Bookkeeping, not prediction.
- [ ] TODO: Only then evaluate learned routers (RouteLLM-style: predict whether a weak
      model suffices) and cascades (FrugalGPT: run cheap, score, escalate). Read both.

## TODO: Asymmetric loss — does gating ever pay?

A false *execute* costs tokens. A false *skip* costs a recovery loop, user trust, and
possibly a silently wrong final answer.

- [ ] TODO: Formalize the asymmetry. Does it push every viable gate back to the
      deterministic checks above?
- [ ] TODO: Test the reframe: use the same signal for **allocation** (model tier,
      reasoning effort, context size, number of parallel attempts) rather than go/no-go.
      Allocation errors degrade gracefully — worse answer, not no answer.

## TODO: Alignment is a separate axis from competence

Agents burn more budget succeeding at the wrong thing than failing at the right thing.
Direction errors compound; competence errors self-correct.

- [ ] TODO: Investigate checking a proposed action against the stated goal — likely a
      small-model or even structural check.
- [ ] TODO: Suspected strongest and least-explored version of the whole idea. Prioritize.

## TODO: Wrong granularity — per-call vs. per-trajectory

Waste isn't one bad call; it's a trajectory that should have been abandoned at step 4 and
ran to step 40. **Kill criteria may beat gate criteria.**

- [ ] TODO: Identify trajectory-level signals — no new information acquired in N steps,
      oscillation between states, cost curve outrunning progress curve.
- [ ] TODO: Compare budget captured by trajectory-level abandonment vs. per-call gating.

## TODO: Which regime — skip or prefetch?

Branch prediction doesn't exist to skip work; it exists to *prefetch*. At 60% confidence
the right move may be "run both, discard the loser," not "skip the 40%."

- [ ] TODO: Decide up front whether the target is token-bound or wall-clock-bound. The
      two regimes pull in opposite directions.

## TODO: Self-prediction calibration

- [ ] TODO: Models are poorly calibrated at predicting their own success, and the
      miscalibration is directional — overconfident on hard tasks, exactly where the
      signal is needed. Quantify before relying on any self-reported confidence.

---

## Prior art to read

- [ ] FrugalGPT (cascades: cheap call → score → escalate)
- [ ] RouteLLM (router trained on preference data to predict weak-model sufficiency)
- [ ] Abstention / "know what you don't know" calibration literature
