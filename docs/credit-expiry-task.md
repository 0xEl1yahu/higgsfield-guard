# Credit expiry reminder

Weekly scheduled task. Renewal falls between the 5th and 7th of each month, per the
transaction log (`Subscription Credits Reset` rows).

Prompt:

> Call the Higgsfield `balance` tool. Report the credit total and how many days remain
> until the next monthly reset, assuming reset on the 6th. If more than 400 credits
> remain and fewer than 10 days are left, say so plainly and note that unused credits
> are wiped at reset rather than carried over.
