---
name: mcp-tools
description: The seven MCP tools, their parameters, and which one to call for each task.
---

# MCP tools

The `uservoice-mcp` server speaks stdio and opens no inbound HTTP service. It
holds the bearer token out of assistant messages and out of logs, and sends it
to UserVoice over HTTPS.

Every tool returns the same envelope: `status`, `data`, `next_actions`,
`guidance`, `pagination`, `rate_limit`. See [envelope.md](envelope.md).

The MCP surface returns one page per call. To drain every page in one stream,
use `uservoice-cli ideas search --all` or `ideas collect`.

## uservoice_workspace_context

Load the principal and the taxonomy.

| Parameter | Meaning |
|---|---|
| `include_catalog` | Also load Forums, Categories, Statuses, Labels |

Call it first. Every ID used later comes from the catalog it returns.

## uservoice_search_ideas

Find Ideas by text, time window, taxonomy, state, and sort.

| Parameter | Meaning |
|---|---|
| `query` | Free text |
| `updated_after` | RFC 3339 timestamp, lower bound |
| `updated_before` | RFC 3339 timestamp, upper bound |
| `forum_ids` | Forum IDs |
| `category_ids` | Category IDs |
| `status_ids` | Status IDs |
| `label_ids` | Label IDs |
| `states` | State names |
| `sort` | Sort field |
| `per_page` | 1 to 100, default 100 |
| `continuation` | Continuation from the previous call |

Returns compact summaries. Call `uservoice_get_idea_context` before deciding
anything.

## uservoice_get_idea_context

Load one Idea by its numeric ID.

| Parameter | Meaning |
|---|---|
| `idea_id` | The Idea ID, required |
| `detail_level` | `summary` or `full`, default `full` |
| `include_supporters` | Also load the Supporter list |
| `per_page` | 1 to 100, default 100, per context section |

Full detail adds comments, Notes, Status Updates, Feedback Records, and
activity. A section that fails to load is named in `missing_sections`, and the
status becomes `partial`.

## uservoice_collect_ideas

Collect one resumable page of Ideas updated inside a time window.

| Parameter | Meaning |
|---|---|
| `updated_after` | RFC 3339 timestamp, required |
| `updated_before` | RFC 3339 timestamp, upper bound |
| `query` | Free text |
| `forum_ids` | Forum IDs |
| `category_ids` | Category IDs |
| `status_ids` | Status IDs |
| `label_ids` | Label IDs |
| `states` | State names |
| `sort` | Sort field, defaults to `updated_at` |
| `detail_level` | `summary` or `full`, default `summary` |
| `per_page` | 1 to 100, default 20. Full detail requires 10 or fewer |
| `continuation` | Continuation from the previous call |

The MCP tool does not store checkpoints. The caller must persist the
continuation before passing it back.

## uservoice_update_idea

Change an Idea safely.

| Parameter | Meaning |
|---|---|
| `mode` | `preview` or `apply`, required |
| `idea_id` | The Idea ID, required |
| `title` | New title |
| `body` | New body |
| `forum_id` | Move to another Forum |
| `category_id` | Move to another Category |
| `assigned_owner_id` | Assign an owner |
| `add_label_ids` | Label IDs to attach |
| `remove_label_ids` | Label IDs to detach |
| `internal_note` | Note for admins, invisible to supporters |
| `change_id` | Change ID from the preview. Required for apply |
| `confirmation` | The literal string `apply`. Required for apply |
| `idempotency_key` | Caller-chosen key. Required for apply |

Call `mode=preview` with only the requested fields. Apply repeats the same
fields and adds `change_id`, `confirmation`, and `idempotency_key`.

## uservoice_publish_idea_update

Publish a Status Update to the Supporters.

| Parameter | Meaning |
|---|---|
| `mode` | `preview` or `apply`, required |
| `idea_id` | The Idea ID, required |
| `status_id` | Target Status ID, required |
| `message` | The Status Update body, required |
| `notify_supporters` | Notify the Supporters, default off |
| `change_id` | Change ID from the preview. Required for apply |
| `confirmation` | The literal string `apply`. Required for apply |
| `idempotency_key` | Caller-chosen key. Required for apply |

Read the full Idea context first. The message goes to the Supporters.

## uservoice_record_feedback

Record one inbound Feedback Record with its provenance.

| Parameter | Meaning |
|---|---|
| `body` | The feedback text, required |
| `channel` | `admin`, `api`, `bookmarklet`, `gainsight`, `import`, `portal`, `roadmap`, `slack`, `zendesk` |
| `idea_id` | Idea the feedback belongs to |
| `user_id` | User the feedback belongs to. Conflicts with `user_email` |
| `user_email` | Email the feedback belongs to. Conflicts with `user_id` |
| `source_url` | Where the feedback came from |
| `source_guid` | Stable identifier at the source |
| `created_at` | RFC 3339 timestamp of the original feedback |
| `idempotency_key` | Caller-chosen key, required |

Link at least one of `idea_id`, `user_id`, or `user_email`. This tool adds
records and never edits the Idea.
