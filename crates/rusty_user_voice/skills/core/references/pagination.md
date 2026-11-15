---
name: pagination
description: Pages, cursors, --all streaming, checkpoints, and the safety ceilings that stop a runaway pull.
---

# Pagination

## Cursor and page

A read returns one page plus a `pagination` block:

```json
{
  "per_page": 100,
  "total_records": 412,
  "next": { "kind": "cursor", "cursor": "abc123" },
  "has_more": true,
  "limit_hit": false
}
```

`has_more` is the boolean to branch on. `next` carries the continuation, and
its `kind` is `cursor` or `page`.

A cursor is an opaque token. Pass it back verbatim. Never read one as a number
and never build one yourself.

Over MCP, one call returns one page. Repeat with the returned continuation
until `next` is absent.

## Streaming everything

`uservoice-cli ideas search --all` drains every page into one NDJSON stream on
stdout, one Idea per line. A cursor hint goes to stderr after each page:

```bash
uservoice-cli ideas search --forum-ids 11 --all
# stdout: NDJSON, one Idea per line
# stderr: next: cursor-abc123
```

`--all` cannot be combined with `--output json`. Use `--output ndjson` or
leave the output mode alone.

When the stream fails, stdout still holds the pages that succeeded and stderr
carries the resume command:

```text
fail: collected 47, resume with --cursor cursor-abc123 --all
```

Resume with the same command plus `--cursor`.

## Checkpoints

A checkpoint is a named, on-disk bundle of the continuation and the time
window. It exists only on the CLI.

```bash
uservoice-cli --output ndjson ideas collect \
  --updated-after 2026-01-01T00:00:00Z \
  --checkpoint-name weekly-triage
```

Re-running the same command resumes where the last run stopped. The window
must match the one the checkpoint was created with, otherwise the command
fails. The checkpoint commits only after the output has been flushed, so a
crash re-reads the last page instead of skipping it.

## Safety ceilings

Two limits stop a runaway pull, and both are loud.

**Context fan-out.** A single Idea's context drains at most 100 pages per
section. Hitting that ceiling returns `status: "conflict"` with
`pagination.limit_hit: true` and a `guidance` string naming the section.
Narrow the query and start again.

**Full-detail collection.** `ideas collect --detail-level full` requires
`--per-page` of 10 or fewer. A larger value is a validation error.

**Search streaming.** `--all` stops after 1000 pages and reports the ceiling
on stderr.

## Choosing a mode

| Need | Mode |
|---|---|
| One page, known cursor | `--cursor` |
| One page, known page number | `--page` |
| Everything, once, as a stream | `--all` |
| A resumable window over days | `ideas collect --checkpoint-name` |
| Everything, through MCP | Repeat the call with the continuation |
