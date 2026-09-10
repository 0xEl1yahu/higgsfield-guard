# Credit expiry reminder

Weekly scheduled task. Renewal falls between the 5th and 7th of each month, per the
transaction log (`Subscription Credits Reset` rows).

Prompt:

> Call the Higgsfield `balance` tool. Report the credit total and how many days remain
> until the next monthly reset, assuming reset on the 6th. If more than 400 credits
> remain and fewer than 10 days are left, say so plainly and note that unused credits
> are wiped at reset rather than carried over.

## Status

Scheduled 2026-09-10 as a Superset automation, not a session cron job: `CronCreate` jobs
are session-only ("gone when Claude exits", 7-day expiry), which cannot carry a monthly
reminder. `superset automations` persists on the host.

- Automation: `higgsfield-credit-expiry`, id `8b80fc0e-f2bc-449b-9956-382529f8bbf4`
- Schedule: `FREQ=WEEKLY;BYDAY=MO;BYHOUR=9;BYMINUTE=0`, Europe/London ("Every Monday at 9:00 AM")
- Project: `7574e721-2b81-4a84-afc9-8aa40b7688aa` (higgsfield-preprocessor)

Verified by a manual trigger on 2026-09-10 (`superset automations run <id>`), which
reached the Higgsfield MCP and reported a live balance — so MCP is available to scheduled
runs, which was the open risk. Inspect with `superset automations get <id>`, list runs
with `superset automations logs <id>`, pause with `superset automations pause <id>`.

**That first run returned 9.52 credits, not the ~1,200 monthly grant SPEC §1 assumes.**
If that is the steady state rather than a one-off, §1's premise ("credits are not scarce,
roughly ten times more are forfeited to expiry than are spent") no longer holds, and
SPEC §13's first re-opening condition — "usage rises far enough that the monthly grant
binds" — has been met. Re-read §13 before assuming the small design is still the right one.
