# rusty-user-voice

Local UserVoice Admin API MCP stdio server and CLI for AI product-manager workflows.

One binary, zero external services. Connect your AI assistant (Claude Code, Cursor, Zed) or terminal directly to your UserVoice tenant — search ideas, collect time-windowed snapshots, preview & apply mutations safely, record provenance-tracked feedback. Built-in rate limiting, retry logic, and an audit ledger.

---

## Why this tool

| Problem | Solution |
|---------|----------|
| AI can't reach UserVoice securely | MCP stdio server — token never leaves your process, no HTTP server exposed |
| Manual triage is slow | `collect` streams NDJSON of ideas in a time window; resumable via named checkpoints |
| Mutations are risky | Two-phase preview→apply with `change_id` TTL, optimistic `updated_at` checks, idempotency keys, on-disk ledger |
| Context is fragmented | `workspace_context` loads taxonomy (forums, categories, statuses, labels) in one call; `get_idea_context` loads full idea + comments + notes + feedback + supporters |
| No audit trail | Local ledger stores preview records, idempotency receipts, collection checkpoints — no secrets, no PII |
| Rate limits bite | Configurable in-flight limit (1–8), automatic backoff with `Retry-After`, structured tracing to stderr |

---

## Architecture

Two-crate Rust workspace (edition 2024, MSRV 1.85):

| Crate | Responsibility |
|-------|----------------|
| [`uservoice_client`](../crates/uservoice_client) | Typed HTTP adapter: auth, models, pagination, rate limits, retries. Zero workflow logic. |
| [`rusty_user_voice`](../crates/rusty_user_voice) | Application layer: CLI (`uservoice-cli`), MCP server (`uservoice-mcp`), config resolution, checkpoint store, change ledger, domain workflows. |

Entry points (both in `crates/rusty_user_voice`):
- `src/main.rs` → `uservoice-mcp` (MCP stdio server)
- `src/bin/cli.rs` → `uservoice-cli` (terminal workflows)

See [`CONTEXT.md`](CONTEXT.md) for ubiquitous language and [`docs/adr/`](docs/adr/) for architectural decisions.

---

## Security

- **Credential boundary**: `USERVOICE_API_KEY` read **only** from process env. Never a CLI flag, never logged, never serialized, `Debug`/`Display` redacted to `[REDACTED]`.
- **Network boundary**: Outbound HTTPS only to `https://<tenant>.uservoice.com/api/v2/admin/...`. Loopback HTTP allowed only in test mode.
- **Mutation safety**: Every write requires preview → apply. Preview returns opaque `change_id` (15 min TTL). Apply demands `confirmation="apply"` + caller-generated `idempotency_key`. Ledger enforces at-most-once apply.
- **Local state**: `USERVOICE_STATE_DIR` holds only preview records, idempotency hashes, and collection cursors. No tokens, no feedback bodies, no emails.
- **File perms**: Local credential files (e.g. `.env`) should be mode `0600`. Config file (`config.yaml`) contains secrets — never commit. See `.env.example` for the variable list.

---

## Configuration

Resolution order (highest wins): **CLI flags → Environment variables → `config.yaml` → Defaults**.

Config file locations (checked in order):
1. `./config.yaml` (next to binary)
2. `~/.config/rusty-user-voice/config.yaml`

```bash
cp config.example.yaml config.yaml
# edit with your values
```

### Environment variables & config keys

| Field | CLI flag | Env var | Config key | Type | Default | Range / Values |
|-------|----------|---------|------------|------|---------|----------------|
| Base URL | `--base-url` | `USERVOICE_BASE_URL` | `uservoice_base_url` | string | **required** | `https://<tenant>.uservoice.com` |
| API token | — | `USERVOICE_API_KEY` | `uservoice_api_key` | string | **required** | Bearer token from Admin Console |
| State dir | `--state-dir` | `USERVOICE_STATE_DIR` | `uservoice_state_dir` | path | platform app data dir¹ | writable directory |
| Max in-flight | `--max-in-flight` | `USERVOICE_MAX_IN_FLIGHT` | `uservoice_max_in_flight` | u32 | `2` | `1–8` |
| Default output | `--output` | `USERVOICE_DEFAULT_OUTPUT` | `default_output` | enum | `human` | `human \| json \| ndjson` |
| CA certificate | — | — | `ca_cert` | path | *(none)* | PEM/DER file for on-prem TLS |
| Log level | — | `USERVOICE_LOG_LEVEL` | `log_level` | enum | `warn` | `error \| warn \| info \| debug \| trace` (RUST_LOG overrides) |

¹ Platform dirs: `$XDG_STATE_HOME/rusty-user-voice` (Linux), `~/Library/Application Support/rusty-user-voice` (macOS), `%LOCALAPPDATA%\rusty-user-voice` (Windows).

### Example `config.yaml`

```yaml
# Required
uservoice_api_key: "uvp_xxxxxxxxxxxxxxxx"
uservoice_base_url: "https://acme.uservoice.com"

# Optional
uservoice_state_dir: "/home/pm/.local/state/rusty-user-voice"
uservoice_max_in_flight: 4
default_output: "json"
ca_cert: "/etc/ssl/certs/corp-root-ca.pem"
log_level: "info"
```

---

## CLI Workflows

Binary: `uservoice-cli` (or `cargo run --bin uservoice-cli --`)

### Global flags
```bash
uservoice-cli --base-url https://acme.uservoice.com \
              --state-dir ~/.local/state/uservoice \
              --max-in-flight 4 \
              --output json \
              <command>
```

### Discover taxonomy & search ideas
```bash
# Full workspace context (principal + forums + categories + statuses + labels + rate limit)
uservoice-cli workspace --include-catalog

# Search ideas (page-based)
uservoice-cli ideas search --query export --per-page 100 --page 1

# Search with cursor pagination (for scripting)
uservoice-cli ideas search --query export --cursor "eyJ..." --per-page 100

# Auto-paginate everything → NDJSON stream (cursor hints to stderr)
uservoice-cli ideas search --query export --all

# Get full idea context (comments, notes, feedback, supporters, status history)
uservoice-cli ideas get 101 --detail-level full --include-supporters
```

### Collect a stable time window (resumable)
```bash
# Named checkpoint auto-resumes on re-run with same --checkpoint-name
uservoice-cli --output ndjson ideas collect \
  --updated-after 2025-01-01T00:00:00Z \
  --updated-before 2026-07-24T00:00:00Z \
  --checkpoint-name weekly-triage

# Filter by forum + query
uservoice-cli --output ndjson ideas collect \
  --updated-after 2025-01-01T00:00:00Z \
  --forum-ids 11 \
  --query osmp
```

> **Pagination & cursors**: See [`skills/uservoice-pm/references/pagination.md`](skills/uservoice-pm/references/pagination.md) for cursor vs page, `--all` streaming, and checkpoint mechanics.

### Safe mutations: preview → apply
```bash
# 1. Preview (dry-run) — returns change_id
uservoice-cli --output json ideas update 101 \
  --mode preview \
  --title "Export roadmap as CSV" \
  --add-label-ids 14

# 2. Apply — must repeat fields, include change_id, confirmation="apply", unique idempotency_key
uservoice-cli --output json ideas update 101 \
  --mode apply \
  --title "Export roadmap as CSV" \
  --add-label-ids 14 \
  --change-id <uuid-from-preview> \
  --confirmation apply \
  --idempotency-key pm-run-20260724-idea-101
```

### Publish public status updates
```bash
uservoice-cli --output json ideas publish 101 \
  --mode preview \
  --status-id 5 \
  --message "Shipping CSV export in v2.4"

uservoice-cli --output json ideas publish 101 \
  --mode apply \
  --status-id 5 \
  --message "Shipping CSV export in v2.4" \
  --change-id <uuid-from-preview> \
  --confirmation apply \
  --idempotency-key pm-publish-20260724-idea-101
```

### Record provenance-tracked feedback
```bash
uservoice-cli --output json feedback record \
  --body "Customer needs a downloadable roadmap." \
  --channel api \
  --idea-id 101 \
  --source-url https://crm.example.invalid/cases/42 \
  --source-guid case-42 \
  --idempotency-key feedback-case-42
```

> **Feedback channels**: `admin`, `api`, `bookmarklet`, `gainsight`, `import`, `portal`, `roadmap`, `slack`, `zendesk` (see [`cli.rs:265-292`](../crates/rusty_user_voice/src/cli.rs#L265-L292))

---

## MCP Server

Binary: `uservoice-mcp` (stdio transport only).

### Client config (Claude Code, Cursor, Zed, etc.)
```json
{
  "mcpServers": {
    "uservoice": {
      "command": "/usr/local/bin/uservoice-mcp",
      "env": {
        "USERVOICE_BASE_URL": "https://acme.uservoice.com",
        "USERVOICE_STATE_DIR": "/Users/pm/.local/state/rusty-user-voice"
      }
    }
  }
}
```

> `USERVOICE_API_KEY` must be present in the **process environment** that spawns the MCP server (shell, systemd, launchd, etc.). Never put it in the JSON config.

### Exposed tools (7 intents)

| Tool | Purpose | Read-only? |
|------|---------|------------|
| `uservoice_workspace_context` | Verify access; load principal + full taxonomy (forums, categories, statuses, labels) + rate-limit envelope. **Start here.** | ✅ |
| `uservoice_search_ideas` | Find ideas by text, update window, taxonomy IDs, state, sort. Returns compact summaries + continuation. | ✅ |
| `uservoice_get_idea_context` | Load one idea by `idea_id`. `detail_level=full` adds comments, notes, status history, feedback, supporters. | ✅ |
| `uservoice_collect_ideas` | Resumable page of ideas updated in a bounded time window. Full detail capped at 10/page; summary at 100. Caller must persist continuation. | ✅ |
| `uservoice_update_idea` | Safe mutation: `mode=preview` → returns `change_id`; `mode=apply` consumes `change_id` + `confirmation="apply"` + `idempotency_key`. | ❌ |
| `uservoice_publish_idea_update` | Public status update to supporters. Same preview/apply flow. | ❌ |
| `uservoice_record_feedback` | Import external feedback (support, Slack, email) with provenance (`channel`, `source_url`, `source_guid`). Idempotent via `idempotency_key`. | ❌ |

**Workflow**: `workspace_context` → `search_ideas` → `get_idea_context` → (mutation tools with preview/apply).

---

## Verification

### Offline (no credentials)
```bash
# Formatting
cargo fmt --all -- --check

# Linting (deny warnings)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Unit + integration tests (fixtures only)
cargo test --workspace

# Documentation
cargo doc --workspace --no-deps
```

### Live smoke test (requires credentials in the process env)

Live smoke tests are `#[ignore]`d by default and run only against a real UserVoice
tenant. They expect a valid bearer token in the process environment; the project
does not commit a credential file. On macOS you can source a local env file:

```bash
test "$(stat -f '%Lp' .env)" = "600"   # macOS
set -a; source .env; set +a
cargo test --workspace -- --ignored
```

On Linux use `stat -c '%a' .env` instead of `stat -f '%Lp' .env`.

---

## References in this repo

| Topic | File |
|-------|------|
| Ubiquitous language / domain terms | [`CONTEXT.md`](CONTEXT.md) |
| Architecture Decision Records | [`docs/adr/`](docs/adr/) |
| Security model details | [`docs/security.md`](docs/security.md) |
| UserVoice API surface map | [`docs/uservoice-api-map.md`](docs/uservoice-api-map.md) |
| Pagination, cursors, checkpoints | [`skills/uservoice-pm/references/pagination.md`](skills/uservoice-pm/references/pagination.md) |
| Idea triage workflow | [`skills/uservoice-pm/references/triage-ideas.md`](skills/uservoice-pm/references/triage-ideas.md) |
| Close-the-loop feedback workflow | [`skills/uservoice-pm/references/close-feedback-loop.md`](skills/uservoice-pm/references/close-feedback-loop.md) |
| CLI argument definitions | [`crates/rusty_user_voice/src/cli.rs`](../crates/rusty_user_voice/src/cli.rs) |
| Config loading & precedence | [`crates/rusty_user_voice/src/config.rs`](../crates/rusty_user_voice/src/config.rs) |
| MCP tool definitions | [`crates/rusty_user_voice/src/mcp.rs`](../crates/rusty_user_voice/src/mcp.rs) |
| Example config file | [`config.example.yaml`](config.example.yaml) |

---

## Installation

```bash
# From source (Rust 1.85+)
git clone https://github.com/your-org/rusty-user-voice
cd rusty-user-voice
cargo install --path crates/rusty_user_voice --bins
# → ~/.cargo/bin/uservoice-cli
# → ~/.cargo/bin/uservoice-mcp
```

---

## License

[MIT](LICENSE-MIT)