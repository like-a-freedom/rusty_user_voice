# UserVoice Agent Utility Design

**Status:** design approved, with API-access preflight required before live use. Architectural decisions recorded in `docs/adr/0001` through `docs/adr/0006`; ubiquitous language in `CONTEXT.md`.

**Decision:** build a local, single-user Rust workspace containing a stdio MCP server and a CLI. It serves an AI product-management agent that needs to collect UserVoice feedback, inspect one idea in full context, and safely implement product-feedback decisions.

## Problem and terminology

UserVoice calls product ideas **suggestions** in its Admin API. A product manager, however, needs an *idea context*, not an endpoint response: the suggestion itself, dates, labels, forum/category/status, comments, internal notes, feedback records, supporters, and activity history. The API exposes those data sets separately, so the server must assemble a bounded, decision-ready result.

An endpoint mirror would force the agent to discover relationships, choose pagination, recover from rate limits, and coordinate writes. That is application logic and belongs inside this utility.

## Goals

- Provide one local executable family: `uservoice-mcp` for MCP over stdio and `uservoice-cli` for shell and automation.
- Expose a curated, goal-oriented MCP surface of seven tools; never expose raw UserVoice REST operation IDs.
- Collect feedback incrementally and preserve the exact IDs, dates, URLs, provenance, and pagination cursor that an agent needs to continue work.
- Support safe updates to idea metadata, labels, internal notes, and public status updates through explicit preview and apply phases.
- Let an agent record feedback from interviews, mail, or chat as a UserVoice feedback record and link it to an idea or user.
- Keep UserVoice secrets and customer data local; never automate corporate SSO, browser cookies, or a RuToken.

## Non-goals for v1

- HTTP, remote hosting, multi-user authentication, or an OAuth callback server.
- Browser automation or extraction of a UserVoice web-session cookie after SSO login.
- Deleting, merging, marking as spam, bulk-changing, or reassigning ideas.
- A local full-text index, dashboard, MCP App, or embedded LLM/subagent.
- Automatic prioritization that masquerades as a product decision. The utility may return transparent factual signals, but the calling product agent makes the recommendation.
- A direct dependency on Azure DevOps, Jira, Slack, or another product system.

## External constraints

### UserVoice access preflight

The UserVoice Admin API requires a **trusted API client** and a bearer token. The documented setup is an API key created in the Admin Console under `Settings → Integrations → UserVoice API keys`; a token is created in that UI or obtained with the client-credentials flow. Trusted credentials can perform administrative actions, including deletion.

Corporate SSO authenticated with a RuToken is valid for human access to the UserVoice Admin Console, but it is not an Admin API credential and must not be used by this program. No code path will inspect a browser profile, SAML assertion, USB token, or session cookie.

Before implementation starts, record one of these supported outcomes:

1. An administrator can create a dedicated API client/token named `rusty-user-voice` for the local user. This is the preferred outcome.
2. UserVoice Support or the UserVoice administrator confirms that the organisation's plan or permissions do not permit Admin API access. The project stays fully testable against fixtures, but live UserVoice functionality remains disabled.
3. Support documents a standards-based authorization-code-with-PKCE grant that grants the exact Admin API permissions needed by this utility. It is a future, separately approved enhancement; it does not change v1's local-token design.

The live startup probe is `GET /api/v2/admin/users/current`. It must show the authenticated principal without printing the access token. A `401`, `403`, or unavailable API-key setting is an actionable configuration error, not a retry loop.

### API behaviour

- Use the UserVoice Admin API at `https://<subdomain>.uservoice.com/api/v2/admin/...` over TLS.
- Cursor pagination is preferred when UserVoice provides a cursor; otherwise preserve its page-based continuation. `per_page` is bounded to the API maximum of 100.
- Incremental collection uses UserVoice's `updated_after` and `updated_before` filters. The successful cursor/checkpoint is committed only after the corresponding page has been emitted.
- On `429`, honour `Retry-After` and expose the reset metadata. Do not retry a write unless the local idempotency ledger proves that retrying is safe.
- A full idea context may require separate suggestion, comments, notes, supporters, feedback-record, status-update, and activity queries. Optional section failures yield `partial`, identify the missing sections, and give the agent an exact recovery action.

## Chosen architecture

```text
AI agent ─┬─ MCP stdio: uservoice-mcp ─┐
          │                             ├─ Agent application modules ─ Typed UserVoice adapters ─ UserVoice Admin API
          └─ CLI: uservoice-cli ────────┘
```

The workspace has two crates:

| Crate | Responsibility |
|---|---|
| `crates/uservoice_client` | Typed HTTP adapter (DDD anti-corruption layer), authentication header, UserVoice request/response models, pagination, rate-limit parsing, retries, and redaction. It has no MCP or Clap dependency. |
| `crates/rusty_user_voice` | Agent-facing application modules, MCP handler, CLI, change-preview/idempotency ledger, output envelopes, documentation, and tests. |

The crate names split by DDD layer (adapter vs. application) rather than by protocol shape; the deviation from the `modern-rust-repo-structure` skill's `core`/`server`/`cli` template is recorded in ADR-0001.

`main.rs` and `src/bin/cli.rs` are thin entry points. Both construct the same `App` and call the same application modules; CLI output formatting and MCP serialization are adapters, not places where product logic lives.

The public seams are small:

| Module | Interface | Implementation hidden behind the interface |
|---|---|---|
| `IdeaReader` | search summaries and load an `IdeaContext` | all UserVoice include/filter choices and bounded fan-out requests |
| `IdeaCollector` | collect one resumable `IdeaPage` | cursor handling, checkpoint semantics, concurrency limit, rate-limit backoff |
| `IdeaUpdater` | create a preview and apply its explicit change record | field mapping, label attach/detach calls, freshness check, partial-write reporting |
| `FeedbackRecorder` | create one linked feedback record | UserVoice channel validation and optional user/idea association |
| `CatalogReader` | load workspace taxonomy | forums, categories, statuses, labels, and current principal lookups |
| `ChangeLedger` | persist preview records and idempotency results | local atomic storage, TTL expiry, and redacted audit metadata |

`ReqwestUserVoiceClient` is the concrete adapter satisfying narrow read/write traits used by those modules. Tests use in-memory adapters or an HTTP fixture server at the same seams. This keeps HTTP shape and UserVoice schema drift local rather than leaking into MCP handlers.

## Configuration and secrets

| Setting | Source | Required | Notes |
|---|---|---|---|
| `USERVOICE_BASE_URL` | environment variable or `--base-url` | yes | tenant root or `/api/v2` form, normalized to one internal API root; rejects non-UserVoice remote hosts. Per `Accepted implementation clarifications` the canonical v1 name. |
| `USERVOICE_API_KEY` | environment variable only | yes for live calls | the bearer access token despite the historical variable name. Never accepted as a CLI argument. Redacted in `Debug`, errors, tracing, CLI, and MCP output. |
| `USERVOICE_STATE_DIR` | environment variable or platform application-state directory | no | holds the local preview/idempotency ledger and explicit CLI checkpoints; contains no access token |
| `USERVOICE_MAX_IN_FLIGHT` | environment variable or CLI flag | no | defaults to `2` for bounded full-context collection |

The initial release does not read client ID/secret from a config file. If Support supplies client credentials, the user exchanges them outside the tool for a token, or a later explicit design adds a secure OS-keychain flow.

## MCP interface

All tools return schema-validated `structuredContent` and a compact text fallback. Every response contains `status`, `data`, `next_actions`, and, when relevant, `guidance`, `pagination`, `rate_limit`, or `applied_operations`. Resource IDs, `portal_url`, and `admin_url` are returned as stable anchors; the agent never has to re-describe an idea in natural language to address it again.

**Outcome vocabulary** (ADR-0002). The application-layer enum is `OutcomeStatus { Ok, Partial, NoResults, Conflict }`; the MCP wire token is the canonical `{ "success", "partial", "no_results", "conflict" }` set, produced by a single mapping at the MCP server seam. `OutcomeStatus::Partial` is a valid degraded result, not a protocol error: it serialises with `isError: false` on the wire so the agent loop continues. Pagination payloads carry `per_page`, `total_records`, `next`, and a derived `has_more: bool` computed from `next.is_some()` (ADR-0005); `next` is an opaque continuation token (cursor or page) passed back verbatim.

All tool parameters are top-level strings, integers, booleans, enums, or arrays of IDs. Nested arbitrary objects are rejected with `serde(deny_unknown_fields)`.

| Tool | Intent and key inputs | Result and next action | Risk annotation |
|---|---|---|---|
| `uservoice_workspace_context` | Optional `include_catalog` (default `true`) | Authenticated principal plus forums, categories, statuses, and labels. Directs the agent to `search_ideas` or `get_idea_context`. | read-only, idempotent |
| `uservoice_search_ideas` | `query`, date bounds, `forum_ids`, `category_ids`, `status_ids`, `label_ids`, `state`, `sort`, `per_page`, `cursor` | Curated summaries with feedback/engagement counts, dates, status, labels, anchors, and continuation. Directs the agent to `get_idea_context`. | read-only, idempotent |
| `uservoice_get_idea_context` | `idea_id`, `detail_level` (`summary` or `full`), `include_supporters`, `supporter_limit`, `feedback_limit` | One stable idea bundle. `full` includes discussion, notes, status history, feedback, activity, and requested supporters; partial data is explicit. | read-only, idempotent |
| `uservoice_collect_ideas` | `updated_after`, `updated_before`, `detail_level`, `per_page`, `cursor`, `checkpoint_name` | One resumable collection page plus a cursor/checkpoint. `full` turns each item into a bounded idea context. Directs the agent to repeat the tool with `next_cursor`. | read-only, idempotent |
| `uservoice_update_idea` | `mode` (`preview` or `apply`), `idea_id`, optional title/body/forum/category/owner, `add_label_ids`, `remove_label_ids`, `internal_note`; apply additionally requires `change_id`, `confirmation="apply"`, and `idempotency_key` | Preview returns the before/after diff and a TTL-bound `change_id`. Apply verifies freshness, performs only requested changes, and reports every applied or failed upstream operation. | mutating, non-idempotent, open-world |
| `uservoice_publish_idea_update` | `mode`, `idea_id`, `status_id`, `message`, `notify_supporters`; apply requires `change_id`, `confirmation="apply"`, and `idempotency_key` | Previews or creates a public status update. Defaults to no supporter notification. | mutating, non-idempotent, open-world |
| `uservoice_record_feedback` | `body`, `channel`, optional `idea_id`, `user_id`, `user_email`, `source_url`, `source_guid`, `created_at`; requires `idempotency_key` | Creates one feedback record with an explicit provenance link; returns the new record ID and its linked idea/user. | mutating, non-idempotent, open-world |

The tool descriptions themselves are agent instructions: they state when to use the tool, when not to use it, limits, return fields, and recovery. `workspace_context` supplies a small discovery path, while normal responses supply the next likely tool and concrete identifiers.

## CLI interface

The CLI mirrors the seven intents without adding a raw-REST escape hatch:

```text
uservoice-cli workspace context
uservoice-cli ideas search
uservoice-cli ideas get
uservoice-cli ideas collect
uservoice-cli ideas update --mode preview|apply
uservoice-cli ideas publish --mode preview|apply
uservoice-cli feedback record
```

`--output json` is available for every command. `ideas collect --output ndjson` streams one complete result per line for large exports. `--checkpoint <path>` is opt-in and records only continuation metadata; it never stores the token. Human-readable output stays concise and points to the same IDs and next command.

## Mutation protocol

The utility enforces the following state machine independently of any MCP client confirmation UI:

```text
requested fields → preview → local change_id (15-minute TTL) → explicit apply → refresh and report
```

1. `preview` loads the current idea, validates the requested fields against workspace taxonomy, calculates the complete change set, and writes a redacted record to `ChangeLedger`.
2. `apply` accepts only a live `change_id`, literal confirmation, and a caller-generated idempotency key. It reloads the idea and rejects the request if `updated_at` differs from the previewed value.
3. The execution order is: suggestion fields, labels, internal note, then public status update when that separate tool is used. Each completed upstream operation is recorded before the next begins.
4. UserVoice does not offer a transaction across those resource types. If a later operation fails, no compensating write is attempted because it could overwrite a legitimate concurrent change. The result is `partial` and contains the exact completed operations plus `get_idea_context` as recovery.
5. The ledger returns a previously completed result for the same idempotency key and request fingerprint. It never deduplicates a request with a different fingerprint.

## Error, rate-limit, and data-safety rules

- Input/schema failures map to MCP `invalid_params` and name the offending field plus a valid example.
- `401`/`403` report `auth_unavailable` with the Admin Console/API-client path; the error never suggests SSO-cookie workarounds.
- `404` and stale-preview/state-mismatch conditions return MCP `internal_error` carrying a structured `Envelope` payload whose `status` is `not_found` or `conflict` respectively, with a `guidance` field naming the next call and, for stale previews only, the preview's `expires_at` so the agent can diagnose staleness. Application parameter validation is the *only* path to `invalid_params`; everything originating upstream, in the filesystem, or in the ledger goes through `internal_error` plus a structured envelope (ADR-0003).
- `429` returns the retry time and a continuation cursor; reads retry only within a small bounded budget. Section-level 429 inside the bounded idea-context fan-out waits `retry_after` (capped at `MAX_RATE_LIMIT_SLEEP`) and retries once; a second 429 or any retry failure drops that section into `missing_sections` with `status = "partial"` and a guidance entry pointing at `uservoice_get_idea_context` (ADR-0006). Writes never retry.
- `5xx` and network failures on reads have exponential backoff with jitter (`MAX_READ_ATTEMPTS = 3`, `MAX_RATE_LIMIT_SLEEP = 30s`). Mutations stop and return their completion ledger.
- Hitting the `MAX_CONTEXT_PAGES = 100` per-section safety ceiling during collection returns `status = "conflict"` with `pagination.limit_hit: true` and a `guidance` field naming the narrower query — never silent truncation (ADR-0004).
- Logs contain operation name, opaque idea ID, outcome, duration, rate-limit state, and counts. They exclude token values, email addresses, message/comment bodies, and any raw UserVoice response.
- The program stores no feedback payload outside explicit CLI NDJSON output or the UserVoice system; the local ledger stores hashes and redacted operation metadata only.

## Rust implementation constraints

- Rust 2024 edition, MSRV `1.85` declared in `Cargo.toml` and pinned as a matrix entry in CI. Tokio multi-thread runtime, RMCP `2.2`, Clap `4`.
- `reqwest` with rustls, Serde, Schemars, Thiserror, Tracing, URL parsing, and a local atomic-state implementation are permitted supporting crates.
- Public IDs are semantic newtypes (`IdeaId`, `ForumId`, `CategoryId`, `StatusId`, `LabelId`, `UserId`, `FeedbackRecordId`), not interchangeable integers.
- Library errors use `thiserror`; `anyhow` is restricted to binary startup and CLI presentation. Public error enums (`ClientError`, `OutcomeStatus`, `AppError`) are `#[non_exhaustive]`.
- No production `unwrap`, no token/PII logging, no lock held across `await`, and no unsafe code.
- Every parameter struct derives `Deserialize` and `JsonSchema`, rejects unknown fields, and documents validation rules.
- `per_page` is validated through one helper (`uservoice_client::pagination::validate_per_page`) shared by the CLI parser, the adapter request validator, and the collector filter.

## Testing and verification design

| Level | Behaviour verified |
|---|---|
| Unit | URL/config validation, token redaction, pagination conversion, date ranges, rate-limit parsing, request serialization, output mapping, freshness checks, TTL expiry, and idempotency fingerprints |
| Application module | bounded idea-context fan-out, partial result envelopes, cursor commit rules, preview/apply state transitions, and no compensation after partial writes |
| HTTP adapter contract | UserVoice OpenAPI-derived request/response fixtures, auth header presence without token disclosure, `429`/`Retry-After`, `401`, `403`, `422`, and malformed payload handling |
| MCP contract | exact seven-tool discovery surface, JSON schemas, annotations, structured output schema, rejected unknown arguments, and no raw endpoint name dispatch |
| CLI | subcommand parsing, JSON/NDJSON output, checkpoint creation/resume, exit codes, and a guarantee that stderr/stdout never include the token |
| Live smoke test | only after a dedicated token exists: `workspace context` against a real UserVoice instance. It is explicit, read-only, and never runs in default CI. |

## Delivery order

1. Authentication/access preflight and redacted fixture corpus.
2. Workspace scaffold, configuration, errors, typed IDs, and observability.
3. UserVoice client read path: auth probe, pagination, rate limits, taxonomy, search, and single-idea context.
4. Shared MCP/CLI read interface and incremental NDJSON collection.
5. Local change ledger and preview/apply mutations.
6. Feedback recording, contract tests, documentation, and a compact companion PM skill.

The companion skill is an optional progressive-disclosure package. Its short `SKILL.md` maps workflows such as “triage new feedback” and “prepare a public response” to the seven tools, while per-workflow reference files carry parameter recipes. It does not duplicate the Admin API manual or add another runtime dependency.

## Future decisions kept out of v1

- Link an idea to a delivery work item through UserVoice's VSTS/Jira linking endpoints after the existing Azure DevOps workflow and permission model are deliberately integrated.
- A local cache/index and factual portfolio report after export volume proves that paging the API is insufficient.
- An MCP App for a human-led backlog/roadmap dashboard.
- An isolated `research_feedback` subagent after measuring whether it improves agent reliability enough to justify its added latency and reduced tool composability.

## Architectural decisions

The following ADRs in `docs/adr/` record non-obvious, hard-to-reverse choices. The spec text above is the authoritative description; the ADRs explain *why* specific deviations from the skill templates were taken.

- **ADR-0001** — workspace crates are `uservoice_client` + `rusty_user_voice` (DDD layer split), not the skill's `core`/`server`/`cli`.
- **ADR-0002** — application `OutcomeStatus` and the MCP wire status token are decoupled; `Partial` is `isError: false`.
- **ADR-0003** — only `Validation` maps to `invalid_params`; `NotFound`, `Conflict`, and all upstream/ledger errors map to `internal_error` plus a structured envelope with `guidance`. Stale-preview `Conflict` echoes `expires_at`.
- **ADR-0004** — collection safety cap returns `status = "conflict"` with `pagination.limit_hit: true` and a recovery `next_action`.
- **ADR-0005** — pagination schema keeps `per_page`, `total_records`, `next` and adds `has_more: bool`; rejects `next_offset` as incompatible with cursor pagination.
- **ADR-0006** — section-level 429 retries once (capped at `MAX_RATE_LIMIT_SLEEP`), then drops to `partial`.

## Sources

- [UserVoice: authentication and trusted API clients](https://developer.uservoice.com/docs/api/api-key/)
- [UserVoice: Admin API data, pagination, rate limits, and side-loading](https://developer.uservoice.com/docs/api/v2/getting-started/)
- [UserVoice: Admin SSO access](https://help.uservoice.com/hc/en-us/articles/360035554813-How-Admins-Log-in-with-JWT-SSO)
- [UserVoice: SSO protocols](https://help.uservoice.com/hc/en-us/articles/360060499314-Single-Sign-On-SSO)
- [UserVoice: live Admin API OpenAPI reference](https://uservoice.uservoice.com/api/v2/public/doc/api-v2-reference)
- [Model Context Protocol: tools and structured output](https://modelcontextprotocol.io/specification/2025-06-18/server/tools)
- [RMCP Rust SDK](https://docs.rs/rmcp/latest/rmcp/)
- [MCP, Skills, and Agents](https://cra.mr/mcp-skills-and-agents)
- [Rethinking Tools in MCP](https://cra.mr/rethinking-the-definition-of-tools-in-mcp)
- [Subagents with MCP](https://cra.mr/subagents-with-mcp)
- [MCP Apps](https://blog.modelcontextprotocol.io/posts/2026-01-26-mcp-apps/)
- [Lessons on Building MCP Servers](https://taoofmac.com/space/blog/2026/04/29/2341)
