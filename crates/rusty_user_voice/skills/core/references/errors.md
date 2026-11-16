---
name: errors
description: Reason tokens, envelope statuses, exit codes, and the recovery to run for each failure.
---

# Errors

## Reason tokens

A failed CLI command exits non-zero and prints a reason token. In machine mode
the token arrives as JSON on stderr and stdout stays empty.

| Token | Meaning | Recovery |
|---|---|---|
| `missing_value` | A required value was absent | Pass the flag, the env var, or the piped key |
| `validation_failed` | A value has the wrong shape | Fix the value. Nothing was saved |
| `not_logged_in` | No usable credentials | Run `uservoice-cli login` |
| `io_error` | Reading or writing failed locally | Check the path and the permissions |
| `confirmation_required` | An existing config would be overwritten | Re-run with `--force`. Exits 3 |
| `unknown_skill` | The requested skill is not in the binary | Run `uservoice-cli skills list` |
| `unknown_reference` | The skill has no such reference | Run `uservoice-cli skills get <name> --full` |

The token is stable. Branch on it rather than on the message text.

```json
{"ok": false, "reason": "not_logged_in", "detail": "...", "hint": "..."}
```

## Envelope statuses

Every response, successful or not, carries a status.

| Status | Meaning | Recovery |
|---|---|---|
| `ok` | Every requested section is present | Continue |
| `partial` | Some sections failed | Use what arrived. Retry only when the response says to |
| `no_results` | The read matched nothing | Widen the filter. Do not retry unchanged |
| `conflict` | Prior state disagrees with the request | Re-read, then preview again |

`partial` is not an error. The tool loop continues.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Failure |
| 2 | Usage error, produced by the argument parser |
| 3 | A confirmation is required |

## Recovery recipes

**The preview expired.** The Change ID lives 15 minutes. Re-read the Idea,
preview again, apply with the new Change ID.

**Apply says the Idea changed.** Someone else edited the Idea between preview
and apply. Re-read it, preview again, and re-check your intent before
applying.

**A write failed part way.** The status is `partial` and `applied_operations`
lists what ran. Refresh the Idea to see the real state. Do not retry with a
new Idempotency Key: the earlier steps already happened.

**The same Idempotency Key was reused with a different payload.** That is a
Conflict by design. Use a new key for a genuinely new intent, or replay the
original payload under the original key.

**The context fan-out hit its ceiling.** The status is `conflict` with
`pagination.limit_hit: true`. Narrow the date window or the Idea filter and
start again.

**The Idea was not found.** Its ID may be stale. Search again to get a current
ID.

## Rate limits

Every response may carry `rate_limit` with the remaining budget and the reset
time. A 429 is retried with the delay the upstream asked for. Do not add a
delay of your own on top.

