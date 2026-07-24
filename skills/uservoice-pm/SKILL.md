# uservoice-pm

A companion skill for AI product managers using the `uservoice-mcp` server and `uservoice-cli` tool.

## Quick start

1. **Discover workspace**: Start with `uservoice_workspace_context` to load forums, categories, statuses, and labels.
2. **Search ideas**: Use `uservoice_search_ideas` with text filters, date ranges, or taxonomy IDs.
3. **Load full context**: Use `uservoice_get_idea_context` with `detail_level=full` before any decision.
4. **Update safely**: Preview changes first, then apply with the returned `change_id`.
5. **Record feedback**: Use `uservoice_record_feedback` with provenance metadata from interviews or support tickets.

## Tool reference

| Tool | When to use | Key inputs |
|---|---|---|
| `uservoice_workspace_context` | First call; taxonomy unknown | `include_catalog` |
| `uservoice_search_ideas` | Find ideas by text, date, or taxonomy | `query`, `forum_ids`, `updated_after` |
| `uservoice_get_idea_context` | Load one idea with full discussion | `idea_id`, `detail_level=full` |
| `uservoice_collect_ideas` | Incremental bulk collection | `updated_after`, `per_page`, `cursor` |
| `uservoice_update_idea` | Change idea fields, labels, or owner | `mode`, `idea_id`, requested fields |
| `uservoice_publish_idea_update` | Post a public status update | `mode`, `idea_id`, `status_id`, `message` |
| `uservoice_record_feedback` | Record inbound feedback with provenance | `body`, `channel`, `idempotency_key` |

## Workflows

### Triage new feedback

1. `uservoice_workspace_context` → discover taxonomy
2. `uservoice_search_ideas` → filter by date range and status
3. `uservoice_get_idea_context` → load each candidate with full context
4. `uservoice_record_feedback` → record the feedback with source metadata
5. `uservoice_update_idea` → add labels or change status based on triage decision

### Prepare a public response

1. `uservoice_get_idea_context` → load full idea context
2. `uservoice_publish_idea_update` → preview the public message
3. `uservoice_publish_idea_update` → apply with change_id and idempotency key

### Bulk collection for analysis

1. `uservoice_collect_ideas` → collect one page with cursor
2. Repeat with `continuation` until absent
3. Analyze the collected ideas offline

## Safety rules

- **Preview before apply**: Always preview mutations before applying. The preview returns a `change_id` with 15-minute TTL.
- **Idempotency keys**: Every mutation requires a unique `idempotency_key`. Reuse the same key for retries.
- **Freshness checks**: Apply verifies `updated_at` matches the preview. Stale previews are rejected with recovery guidance.
- **Partial writes**: If a write fails mid-sequence, the result is `partial` with completed operations listed. Never retry with a new key.
- **No compensation**: Failed writes are never rolled back. Refresh the idea context to inspect the actual state.

## Reference documents

- `references/triage-ideas.md` — Step-by-step triage workflow
- `references/close-feedback-loop.md` — Customer feedback response workflow
