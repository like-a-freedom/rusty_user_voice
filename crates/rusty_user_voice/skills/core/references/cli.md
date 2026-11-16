---
name: cli
description: Every uservoice-cli command and flag, with the MCP tool that does the same job and the defaults that matter.
---

# CLI reference

Global flags come before the subcommand:

```bash
uservoice-cli --output json --base-url https://acme.uservoice.com ideas get 101
```

| Flag | Env | Meaning |
|---|---|---|
| `--base-url` | `USERVOICE_BASE_URL` | Tenant root, for example `https://acme.uservoice.com` |
| `--state-dir` | `USERVOICE_STATE_DIR` | Where the ledger and checkpoints live |
| `--max-in-flight` | `USERVOICE_MAX_IN_FLIGHT` | Concurrent upstream requests, 1 to 8, default 2 |
| `--output` | `USERVOICE_DEFAULT_OUTPUT` | `human`, `json`, or `ndjson`, default `human` |

`--output ndjson` works only for the `ideas` commands. It is meant for the streaming ones, `ideas collect` and `ideas search --all`. `--output json` and `--all` cannot be combined: the stream is NDJSON.

## Commands

| Command | MCP tool | Notes |
|---|---|---|
| `login` | none | Writes `~/.config/rusty_user_voice/config.yaml`, mode 0600 |
| `logout` | none | Deletes that file |
| `workspace` | `uservoice_workspace_context` | Principal plus taxonomy |
| `ideas search` | `uservoice_search_ideas` | One page, or a stream with `--all` |
| `ideas get` | `uservoice_get_idea_context` | One Idea with full detail |
| `ideas collect` | `uservoice_collect_ideas` | One page of a time window, resumable |
| `ideas update` | `uservoice_update_idea` | Preview then apply |
| `ideas publish` | `uservoice_publish_idea_update` | Preview then apply |
| `feedback record` | `uservoice_record_feedback` | One step, idempotent |
| `skills` | none | Serves this corpus. Run `uservoice-cli skills get core` for the guide |

## login

| Flag | Meaning |
|---|---|
| `--base-url` | Tenant root. Falls back to the global flag, then the env var, then a prompt |
| `--api-key-stdin` | Read the key from a pipe. There is no `--api-key` flag |
| `--ca-cert` | PEM or DER file for a corporate TLS root |
| `--force` | Overwrite an existing config, preserving the keys `login` does not own |
| `--no-verify` | Skip the network call. Local validation still runs |

Prompts appear only on a TTY, and they go to stderr. A headless run with a
missing value fails with `missing_value` instead of blocking.

## workspace

| Flag | Meaning |
|---|---|
| `--include-catalog` | Also load Forums, Categories, Statuses, Labels, and the Idea custom fields. On by default |

## ideas search

| Flag | Meaning |
|---|---|
| `--query` | Free text |
| `--updated-after` | RFC 3339 timestamp, lower bound |
| `--updated-before` | RFC 3339 timestamp, upper bound |
| `--forum-ids` | Comma-separated Forum IDs |
| `--category-ids` | Comma-separated Category IDs |
| `--status-ids` | Comma-separated Status IDs |
| `--label-ids` | Comma-separated Label IDs |
| `--states` | Comma-separated state names |
| `--sort` | Sort field |
| `--per-page` | 1 to 100, default 100 |
| `--cursor` | Continuation token from a previous page. Conflicts with `--page` |
| `--page` | Page number. Conflicts with `--cursor` |
| `--all` | Drain every page into an NDJSON stream |

## ideas get

| Flag | Meaning |
|---|---|
| `--detail-level` | `summary` or `full`, default `full` |
| `--include-supporters` | Also load the Supporter list |
| `--per-page` | 1 to 100, default 100, per context section |

## ideas collect

| Flag | Meaning |
|---|---|
| `--updated-after` | RFC 3339 timestamp, required. Start of the window |
| `--updated-before` | RFC 3339 timestamp, upper bound |
| `--query` | Free text |
| `--forum-ids` | Comma-separated Forum IDs |
| `--category-ids` | Comma-separated Category IDs |
| `--status-ids` | Comma-separated Status IDs |
| `--label-ids` | Comma-separated Label IDs |
| `--states` | Comma-separated state names |
| `--sort` | Sort field, defaults to `updated_at` |
| `--detail-level` | `summary` or `full`, default `summary` |
| `--per-page` | 1 to 100, default 20. Full detail requires 10 or fewer |
| `--cursor` | Continuation token. Conflicts with `--page` |
| `--page` | Page number. Conflicts with `--cursor` |
| `--checkpoint-name` | Name of a checkpoint to resume |

## ideas update

| Flag | Meaning |
|---|---|
| `--mode` | `preview` or `apply`, required |
| `--title` | New title |
| `--body` | New body |
| `--forum-id` | Move to another Forum |
| `--category-id` | Move to another Category |
| `--assigned-owner-id` | Assign an owner |
| `--add-label-ids` | Comma-separated Label IDs to attach |
| `--remove-label-ids` | Comma-separated Label IDs to detach |
| `--internal-note` | Note for admins, invisible to supporters |
| `--custom-field` | Set a custom field from a string: `cf_tfs_link=https://...`. Repeatable |
| `--custom-field-json` | Set a custom field from a JSON literal: `cf_reviewed=true`, `cf_points=5`, `null` clears. Repeatable |
| `--change-id` | Change ID from the preview. Required for apply |
| `--confirmation` | The literal string `apply`. Required for apply |
| `--idempotency-key` | Caller-chosen key. Required for apply |

## ideas publish

| Flag | Meaning |
|---|---|
| `--mode` | `preview` or `apply`, required |
| `--status-id` | Target Status ID, required |
| `--message` | The Status Update body, required |
| `--notify-supporters` | Notify the Supporters, default off |
| `--change-id` | Change ID from the preview. Required for apply |
| `--confirmation` | The literal string `apply`. Required for apply |
| `--idempotency-key` | Caller-chosen key. Required for apply |

## feedback record

| Flag | Meaning |
|---|---|
| `--body` | The feedback text, required |
| `--channel` | `admin`, `api`, `bookmarklet`, `gainsight`, `import`, `portal`, `roadmap`, `slack`, `zendesk`. Default `api` |
| `--idea-id` | Idea the feedback belongs to |
| `--user-id` | User the feedback belongs to. Conflicts with `--user-email` |
| `--user-email` | Email the feedback belongs to. Conflicts with `--user-id` |
| `--source-url` | Where the feedback came from |
| `--source-guid` | Stable identifier at the source |
| `--created-at` | RFC 3339 timestamp of the original feedback |
| `--idempotency-key` | Caller-chosen key, required |

## skills

| Command | Meaning |
|---|---|
| `skills list` | Name, description, and reference count of every bundled skill |
| `skills get <name>` | Print one skill, frontmatter included |
| `skills get <name> --ref <path>` | Print one reference, for example `cli.md` |
| `skills get <name> --full` | Print the skill and all of its references |
| `skills get --all` | Print the whole corpus |

The global `--output` decides the shape: `human` prints the document as it is, `json` wraps it. `skills` needs no credentials, so it works before `login`. The text it serves is compiled into the binary, so it always matches the installed version.
