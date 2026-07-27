# Pagination

How to page through results from UserVoice, both via the MCP surface and the CLI.

## CLI: `--all` (auto-pagination)

Use `--all` on `ideas search` to drain every page in one stream:

```bash
uservoice-cli ideas search --query export --all
```

- Output is NDJSON (one idea per line to stdout).
- After each page, a cursor hint is logged to stderr: `next: <cursor>`.
- On failure, a resume message is logged: `fail: collected N, resume with --cursor <X> --all`.
- Exit code 1 on partial failure; the NDJSON stream on stdout is still valid for the pages that succeeded.

`--all` cannot be combined with `--output json` (use NDJSON or omit `--output`).

## CLI: `--cursor` (manual resumption)

Use `--cursor` when you have a known cursor from a previous failure or partial run:

```bash
uservoice-cli ideas search --query export --cursor <X> --all
```

Or for single-page fetches:

```bash
uservoice-cli --output json ideas search --query export --cursor <X>
```

## MCP: one page per call

The MCP surface returns one page per call. After every read call, inspect:

- `envelope.pagination.has_more` — `true` when more pages exist.
- `envelope.next_actions` — contains the continuation arguments for the next call.

To drain every page in one stream, use the CLI with `--all`; the MCP surface returns one page per call by design.

## Example: CLI `--all`

```bash
uservoice-cli ideas search --query osmp --forum-ids 11 --all
# stdout: NDJSON stream of all matching ideas
# stderr: next: cursor-abc123\n (after each page)
```

## Example: CLI `--cursor` resume

```bash
# First run fails after 47 ideas:
uservoice-cli ideas search --query osmp --all
# stderr: fail: collected 47, resume with --cursor cursor-abc123 --all

# Resume:
uservoice-cli ideas search --query osmp --cursor cursor-abc123 --all
```

## Example: MCP `next_actions`

```json
{
  "pagination": { "has_more": true, "next": { "kind": "cursor", "cursor": "abc123" } },
  "next_actions": [{
    "tool": "uservoice_search_ideas",
    "reason": "...",
    "arguments": { "continuation": { "kind": "cursor", "cursor": "abc123" } }
  }]
}
```
