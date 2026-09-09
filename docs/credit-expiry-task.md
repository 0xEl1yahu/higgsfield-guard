# Credit expiry reminder

Status: proposed reminder; deployment/schedule not verified or modified on 2026-09-09.

## Historical problem, measured

From the Higgsfield `transactions` log: ~6,910 credits forfeited to monthly expiry across
six cycles, against ~732 ever spent generating. Unused credits are wiped at each renewal
(`Subscription Credits Reset` rows), not carried over. Plan: `ultimate`, 1,200 credits
granted per cycle.

| Cycle reset | Forfeited at reset |
|---|---|
| 2026-08-05 | 1,192 |
| 2026-07-06 | 1,200 |
| 2026-06-06 | 1,200 |
| 2026-05-07 | 1,200 |
| 2026-04-07 | 1,200 |
| 2026-03-08 | 918 |

## The cycle is 30 days, NOT calendar-monthly

Gaps between the six observed resets: 30, 30, 30, 30, 30 days. The day-of-month drifts
backwards roughly one day per month. **An earlier version of this document said to assume
"reset on the 6th"; that was wrong** and would warn on the wrong day within two cycles.

Compute the next reset as `last_known_reset + 30 days`, re-anchoring on the most recent
`Subscription Credits Reset` row each run rather than extrapolating from a fixed date.

Historical illustration: `2026-08-05` → `2026-09-04`. This is past as of 2026-09-09,
not the next reset now. Fetch fresh transactions before reporting a current deadline.

## The scheduled task prompt

> Call the Higgsfield `transactions` tool and find the most recent `Subscription Credits
> Reset` row; that timestamp is the last reset. The cycle is 30 days, so the next reset is
> that date plus 30 days. Then call `balance` for the current credit total.
>
> Report the credit total and the days remaining until the next reset. If more than 400
> credits remain and the reset is within 7 days inclusive (using exact timestamps and
> the configured local timezone), say so prominently and state that unused
> credits are wiped at reset rather than carried over.
>
> If the most recent reset is more than 31 days ago, the 30-day assumption has broken —
> say so rather than reporting a date computed from it.

## Proposed schedule

Weekly, Mondays 09:00 local. The inclusive seven-day boundary avoids a strict-window
gap, but outages can still miss delivery. Scheduling and delivery guarantees need
separate approval and verification.
