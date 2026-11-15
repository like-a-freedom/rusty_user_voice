---
name: envelope
description: The fields of a response envelope, what each one carries, and how to read the next_actions pointer.
---

# The response envelope

Every tool returns the same six fields, whatever the call.

```json
{
  "status": "ok",
  "data": { },
  "next_actions": [
    {
      "tool": "uservoice_get_idea_context",
      "reason": "Load full context for a selected idea ID.",
      "idea_id": 101,
      "arguments": { "idea_id": 101 }
    }
  ],
  "guidance": null,
  "pagination": null,
  "rate_limit": null
}
```

| Field | Meaning |
|---|---|
| `status` | `ok`, `partial`, `no_results`, or `conflict`. See [errors.md](errors.md) |
| `data` | The payload. Its shape depends on the tool |
| `next_actions` | What to call next, with the arguments to use |
| `guidance` | A sentence of recovery advice, present when the status needs it |
| `pagination` | The page block, present on paged reads |
| `rate_limit` | The remaining upstream budget, when the upstream reports it |

## next_actions

`next_actions` is the pointer to the next call. Read it instead of guessing
the sequence. Each entry names the `tool`, explains why in `reason`, and
carries the `arguments` to send.

The entry after `uservoice_workspace_context` names `uservoice_search_ideas`.
The entry after `uservoice_search_ideas` names `uservoice_get_idea_context`. The
entry after a mutation names `uservoice_get_idea_context`, because the state
after a write must be re-read rather than assumed.

An empty `next_actions` array means the call is complete.

## guidance

`guidance` appears when the outcome needs an explanation, for example a
conflict or a partial load. It is a sentence addressed to the caller. It
describes what to do next, not what went wrong in detail: the detail belongs
in `status` and in the error payload.

## Data shapes

| Tool | `data` |
|---|---|
| `uservoice_workspace_context` | `current_user` and, with `include_catalog`, the catalog |
| `uservoice_search_ideas` | `ideas`, a list of compact summaries |
| `uservoice_get_idea_context` | `suggestion`, `comments`, `notes`, `supporters`, `feedback_records`, `status_updates`, `activity`, `missing_sections` |
| `uservoice_collect_ideas` | `ideas`, a list of Idea contexts |
| `uservoice_update_idea` | `mode`, `change_id`, `idea_id`, `expected_updated_at`, `expires_at`, `changes`, `applied_operations` |
| `uservoice_publish_idea_update` | The same shape as an update |
| `uservoice_record_feedback` | The recorded Feedback Record |

`missing_sections` names every section that failed to load. It is empty on a
complete result.

## applied_operations

A mutation lists every operation it performed, in order, with the outcome of
each. An operation whose outcome starts with `failed:` is the reason the
status became `partial`.

Read this list before deciding what to do after a failed write. It says what
already happened.
