# UserVoice CLI Papercuts: Bulk Collect Filters, Auto-Pagination, and Skill Discoverability

**Status:** ready to implement.
**Scope:** three papercuts from `.dev/PAPERCUTS.md` (live E2E session, 2026-07-27) closed in three commits.
**Out of scope:** MCP surface changes beyond tool descriptions; ADR-0001..0006 stand.

## Problem

Live end-to-end testing against `kaspersky.uservoice.com` surfaced three rough edges that together make the CLI hard for an AI agent to use correctly:

1. **`ideas collect` has no `--forum-ids` / `--query` / taxonomy filters.** Bulk extraction by date range is the only working pattern, and it requires a local post-filter for any forum-specific use case.
2. **An agent did not realise `ideas search` supports pagination.** `--cursor` and `--page` exist on `SearchArgs`, and `envelope.pagination` is populated, but neither the human output nor the skill pointed the agent at them.
3. **Text search returns noise.** `q=osmp` also matches `KasperskyOS`. This is an upstream UserVoice behaviour; we cannot fix it. We can teach the agent to prefer `forum_ids` for forum-specific work.

## Decisions (no ADR — execution-level, all single-commit reversible)

These are recorded here so the next reader does not have to reconstruct the reasoning. None of them is hard-to-reverse; therefore no new ADR.

- **`--all` is the opt-in auto-paging flag, only on `search`.** `--all` without `--output` implies `--output ndjson`. Reject `--output json --all` with a clear error (Y4) so the envelope-контракт ADR-0002 stays intact.
- **`collect` is unchanged structurally.** It already does "drain a window" via `--checkpoint-name` plus the default `ndjson` output. Adding `--all` to `collect` would duplicate the same idea through two flags (C1). What `collect` lacks is **filters**, not paging.
- **Cursor hint goes to stderr, not on-disk checkpoint.** `search` is read-only and stateless; on-disk checkpoint is the right tool for `collect` (long runs, `updated_after` window) and the wrong tool for `search` resume (K3). A failure message on stderr (`fail: collected 47/200, resume with --cursor <X> --all`) is enough to make `--all` resumable without inventing on-disk state.
- **Exit code 1 on `--all` failure, stderr message, partial stdout preserved.** POSIX-conventional. A NDJSON stream of 47 ideas is still a valid NDJSON stream of 47 ideas; the agent reads exit code to know it was incomplete (X4).
- **MCP discoverability is via tool description, not a new tool.** The seven-tool surface is contractual (ADR-0001). Each of the four read tools gains a one-line description: "after this call, check `envelope.pagination.has_more` and `envelope.next_actions` for the next step." A new tool would be a single-purpose flag exposed as a permanent surface area.
- **Papercut #3 is fixed in the skill, not in the code.** UserVoice's `q=` is full-text and we cannot make it exact-match. The right move is to teach the agent to reach for `forum_ids` first, and to use `search` only inside a known forum. No post-filter on the client (that would silently drop results, which is worse than telling the agent to use a different flag).
- **`CONTEXT.md` is untouched.** `--all`, `cursor`, and pagination are CLI/MCP surface, not domain nouns. The glossary already distinguishes `Checkpoint` (on-disk, `collect`-only) from `Pagination.next` (stateless cursor, opaque).
- **Skill update is iterative, not a rewrite.** Per `writing-great-skills`, audit for sediment and duplication first, then add. New `references/pagination.md` for everything pageable; SKILL.md gets only the short reminder to read it. The `bulk-osmp-2026` style workflow does **not** become a reference file — that is a one-shot scenario, not a principle.

## Commits

### Commit 1: `Add taxonomy and text filters to ideas collect`

**Files:**

- Modify: `crates/rusty_user_voice/src/cli.rs` — extend `CollectArgs` with `query`, `forum_ids`, `category_ids`, `status_ids`, `label_ids`, `states`, `sort`. All optional, all compatible with the existing `updated_after` / `updated_before` window. Reuse `value_delimiter = ','` pattern from `SearchArgs`.
- Modify: `crates/rusty_user_voice/src/cli_runner.rs` — forward new fields to `CollectIdeasParams`.
- Modify: `crates/rusty_user_voice/src/app/read.rs` — already passes through `SearchIdeasParams`; verify the same plumbing exists for `CollectIdeasParams`. (The `SearchSuggestionsRequest` already takes all these fields, so the change is in the CLI → app wiring, not in the adapter.)
- Modify: `crates/rusty_user_voice/tests/cli_contract.rs` — add at least one new test per new flag (a happy-path test that the flag is forwarded, plus a validation test that empty values are rejected the same way `SearchArgs` rejects them).
- Modify: `README.md` — add `ideas collect` example with `--forum-ids`.

**Completion criterion:** `cargo test --workspace` is green; every new flag has a passing contract test; `cargo clippy --workspace --all-targets --all-features -- -D warnings` is clean; `cargo fmt --all -- --check` is clean. Existing `collect` behaviour (date window, checkpoint, ndjson) is unchanged.

### Commit 2: `Add --all to ideas search for streamed auto-pagination`

**Files:**

- Modify: `crates/rusty_user_voice/src/cli.rs` — add `--all: bool` to `SearchArgs`. When set, force `output = Ndjson` if `output = Json`; emit a clap-level error otherwise (Y4). When `output` is unset and `--all` is set, default to `Ndjson`.
- Modify: `crates/rusty_user_voice/src/cli_runner.rs` — implement the loop: call `search_ideas` repeatedly, write each `Envelope.data.ideas` line-by-line as NDJSON to stdout, log `next: <cursor>` to stderr after each page, stop on `envelope.pagination.has_more == false` or on the first `Err` from the app. On `Err`, write a final stderr line `fail: collected N/M, resume with --cursor <X> --all` and return a non-zero exit.
- Modify: `crates/rusty_user_voice/src/main.rs` and `crates/rusty_user_voice/src/bin/cli.rs` — propagate the new error type if the runner now returns a `Result<(), CliError>`; otherwise the existing `anyhow::Result<()>` already covers it.
- Modify: `crates/rusty_user_voice/tests/cli_contract.rs`:
  - `--output json --all` is rejected with a clear error and exit code 2.
  - `--all` with NDJSON output streams every idea from a two-page fixture, in order.
  - A fixture whose first page has `has_more=true` and whose cursor page returns a 5xx after the retry budget yields NDJSON up to the last good page, exit code 1, and stderr carrying `next: <last-cursor>` plus `fail: ...`.
- Modify: `crates/rusty_user_voice/src/mcp.rs` (or wherever the seven tool descriptions live) — for each of the four read tools, append to its `description`: `After this call, inspect envelope.pagination.has_more and envelope.next_actions for the next step. To drain every page in one stream, use the CLI with --all; the MCP surface returns one page per call by design.`
- Modify: `README.md` — add an `--all` example to the `ideas search` section, and explicitly note the stderr `next:` contract.

**Completion criterion:** same gate as commit 1, plus: `cargo test --all-features` covers the new `--all` paths; `cargo doc --workspace --no-deps` with `RUSTDOCFLAGS=-D warnings` is clean. `--all` and `--checkpoint-name` cannot both be set in `collect`; if `--all` is ever added to `collect` later, it must conflict with `--checkpoint-name` (not in this commit).

### Commit 3: `Refine uservoice-pm skill for pagination and forum-first search`

**Files:**

- Modify: `skills/uservoice-pm/SKILL.md` — audit against `writing-great-skills`:
  - Drop lines that just restate a tool's name and intent (the table of tools is borderline; if a row's "key inputs" is just the obvious first arg, delete the row). Keep only rows that earn their space.
  - Replace any sentence that begins with "Always" or "Never" and is not a guardrail (negation pattern from `writing-great-skills` — name the positive behaviour instead).
  - In Quick Start, add one bullet: "For forum-specific work, pass `forum_ids` (from `workspace_context`) before reaching for text search."
  - In the search-related entries of any table that remains, link to `references/pagination.md`.
- Create: `skills/uservoice-pm/references/pagination.md` — the single source of truth for:
  - When to use `--all` vs `--cursor <X>` (use `--all` for "drain"; use `--cursor` when resuming after a known failure).
  - The stderr contract: `next: <cursor>` is emitted after every page; on failure, `fail: collected N/M, resume with --cursor <X> --all`.
  - Exit code 1 on incomplete drain; NDJSON output is still valid for the pages that succeeded.
  - The MCP counterpart: read `envelope.pagination.has_more` and `envelope.next_actions` after every read call.
  - One example per surface (CLI `--all`, CLI `--cursor`, MCP `next_actions`).
- Modify: `skills/uservoice-pm/references/triage-ideas.md` — replace the "Use `uservoice_search_ideas` for an ad-hoc question" line with: "Use `uservoice_search_ideas` only after you have `forum_ids` from `workspace_context`; text search alone returns noisy matches. For bulk extraction, prefer `uservoice_collect_ideas --forum-ids <id>` over text search." This is the one place that addresses papercut #3 — explicitly, with the workaround that actually works.
- Modify: `skills/uservoice-pm/references/close-feedback-loop.md` — no change (out of scope).
- Modify: `README.md` — add a one-liner cross-reference to `skills/uservoice-pm/references/pagination.md` from the `## CLI workflows` section.

**Completion criterion:** SKILL.md is **shorter** than it was before the change, not longer (sediment audit). The new `references/pagination.md` is reachable from at least one in-skill pointer. `references/triage-ideas.md` is the only existing reference edited, and only in the lines that address papercut #3. No prose change in any file is a no-op (writing-great-skills: every line must change behaviour vs. the default).

## Final verification

After all three commits, run the full offline release gate from the existing plan:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo test --workspace --all-features
cargo doc --workspace --no-deps
RUSTDOCFLAGS=-D warnings
git status --short   # only intended changes; .env.test never listed
```

Then run the live smoke test to confirm the new flags actually talk to UserVoice:

```bash
test "$(stat -f '%Lp' .env.test)" = "600"
set -a; source .env.test; set +a
cargo test -p rusty_user_voice --features live-smoke --test live_smoke -- --ignored --nocapture
```

Expected: all four commands exit 0; the live smoke test passes; `--all` and the new `collect` filters are covered by wiremock contract tests so a UserVoice regression breaks CI before it breaks a user.

## Out of scope, deliberately

- New ADR. None of the decisions above is hard-to-reverse; they are execution details recorded here for the next reader.
- A new MCP tool. ADR-0001 keeps the surface at seven.
- Post-filter on the client for papercut #3. Silent result mutation is worse than teaching the agent to use a better flag.
- A new "OSMP 2026" reference workflow. That is a one-shot scenario; the principle (forum-first) is already in `triage-ideas.md`.
- CONTEXT.md changes. `--all` and `cursor` are surface, not domain.
