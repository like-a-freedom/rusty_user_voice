---
name: core
description: Start here for any uservoice-cli or uservoice-mcp task. Covers authentication, the read loop, the preview-apply write loop, safety rules, and the error vocabulary.
---

# UserVoice core

`uservoice-cli` and `uservoice-mcp` are one adapter behind two doors. The CLI
serves a terminal and scripts. The MCP server exposes the same seven
operations to an agent over stdio. The behavior is identical; only the
transport differs.

Read this page once per session. Load a reference only when the task needs it.

## Authenticate first

The bearer token never appears in a command line. There is no `--api-key`
flag, on purpose: argv is readable through `ps` and stays in shell history.

```bash
uservoice-cli login
```

`login` prompts for the base URL and the key on a terminal, hides the key
input, and writes `~/.config/rusty-user-voice/config.yaml` with mode 0600. It
verifies the credentials against the tenant before saving them. Headless
callers pass the key on stdin instead:

```bash
echo "$USERVOICE_API_KEY" | uservoice-cli login --base-url https://acme.uservoice.com --api-key-stdin --force
```

An existing config is never overwritten without `--force`. A refused overwrite
exits 3 with the reason `confirmation_required`.

`USERVOICE_API_KEY` in the environment overrides the stored key. `login` never
prints the key.

## The read loop

Three steps, in this order.

**1. Load the workspace.** It returns the principal and the taxonomy: Forums,
Categories, Statuses, Labels. Every ID used later comes from here.

```bash
uservoice-cli workspace --include-catalog
```

**2. Search.** Filter by taxonomy IDs and a time window rather than by text
alone. Text search returns noisy matches.

```bash
uservoice-cli ideas search --forum-ids 11 --updated-after 2026-01-01T00:00:00Z --per-page 100
```

**3. Load one Idea.** Full detail pulls comments, Notes, Status Updates,
Feedback Records, and activity.

```bash
uservoice-cli ideas get 101 --detail-level full --include-supporters
```

Search returns compact summaries. Decide only after `ideas get`.

## The write loop

Every write takes two steps. `preview` returns a Change ID, `apply` spends it.

**Preview.** Send only the fields you want to change.

```bash
uservoice-cli ideas update 101 --mode preview --title "Export roadmap as CSV" --add-label-ids 14
```

The preview lists every field change and returns a Change ID that expires
after 15 minutes.

**Apply.** Repeat the same fields, add the Change ID, confirm, and pass a
unique Idempotency Key.

```bash
uservoice-cli ideas update 101 --mode apply --title "Export roadmap as CSV" --add-label-ids 14 \
  --change-id "$CHANGE_ID" --confirmation apply --idempotency-key pm-run-20261113-idea-101
```

Rules that hold for every write:

- The same Idempotency Key with the same payload replays the recorded result,
  so a retried apply writes once. The same key with a different payload is a
  Conflict.
- A Change ID does not stop you from applying twice under two different keys.
  The second apply is caught by the freshness check instead, because the first
  write moved the Idea. Preview again for a second write.
- Apply checks that the Idea has not changed since the preview. A stale
  preview is a Conflict: re-read the Idea and preview again.
- A write that fails part way leaves the earlier steps in place. The result is
  `partial`. Refresh the Idea before deciding anything.
- Never retry a failed write with a fresh Idempotency Key. That duplicates the
  side effect.

Publishing a Status Update to supporters follows the same two steps, through
`ideas publish`. Recording an inbound Feedback Record takes one step, through
`feedback record`, and needs an Idempotency Key.

## The error vocabulary

Every response carries a status.

| Status | Meaning | What to do |
|---|---|---|
| `ok` | Every requested section is present | Continue |
| `partial` | Some sections failed | Use what arrived, retry the missing ones |
| `no_results` | The read matched nothing | Widen the filter, do not retry unchanged |
| `conflict` | Prior state disagrees with the request | Re-read, then preview again |

A failed CLI command carries a reason token instead: `missing_value`,
`validation_failed`, `not_logged_in`, `io_error`, `confirmation_required`.
Exit codes: 0 success, 1 failure, 2 usage error, 3 a confirmation is required.

## Load more when you need it

Every row is a command. Run it to load that piece.

| Load | When |
|---|---|
| `uservoice-cli skills get core --ref cli.md` | Any command or flag not shown here |
| `uservoice-cli skills get core --ref mcp-tools.md` | Driving the seven tools over MCP |
| `uservoice-cli skills get core --ref pagination.md` | Cursors, `--all`, checkpoints, safety caps |
| `uservoice-cli skills get core --ref errors.md` | Reason tokens, statuses, recovery recipes |
| `uservoice-cli skills get core --ref envelope.md` | The fields of a response envelope |
| `uservoice-cli skills get triage` | Triaging new and updated Ideas |
| `uservoice-cli skills get feedback-loop` | Answering supporters after a fix ships |
