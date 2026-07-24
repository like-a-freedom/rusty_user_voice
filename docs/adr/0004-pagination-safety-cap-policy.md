# ADR-0004: Collect-context safety cap returns `status = "conflict"` with guidable recovery, not silent truncation

When `collect_ideas` (Task 4) hits the `MAX_CONTEXT_PAGES = 100` per-section safety ceiling, the envelope returns `status = "conflict"` carrying the `pagination.limit_hit: true` flag and a `guidance` field that points the caller at the narrower query — narrower date window, smaller `per_page`, or a `state` filter — rather than silently truncating the result set. The cap is a deliberate safety against an upstream that has no formal page bound, so the failure mode must be loud enough that an agent cannot miss it and must point at a concrete next call.

## Status
Accepted — 2026-07-24

## Considered options

- **Continue silently past the cap** — produces an unbounded pull; can hang the process for hours and saturate the rate limit. Rejected.
- **Hard error and abort the run** (`Err(AppError::State(...))`) — surfaces quickly, but loses the sections already collected and forces the caller to restart from scratch. Rejected: the resume semantic for `collect_ideas` already exists, and aborting wastes up to 99 pages of work.
- **Return `Envelope<IdeaPage> { status = Conflict, guidance, missing_sections }`** *(chosen)* — preserves collected sections, marks the run as incomplete, and renders the recovery step as a literal `next_actions` argument so the agent can resume.

## Consequences

- `collect_ideas` never returns fewer pages than the cap allows without flagging it; agents that ignore the `guidance` field will still write a correct but partial context but `Envelope.status = Conflict` forces visibility.
- The CLI `OutputMode::Human` printer renders `status=conflict limit_hit=true` and the same `guidance` text so a human sees the truncation before deploying it.
- The MCP wire schema must include `pagination.limit_hit: bool`; documenting that field is in scope for Task 10's schema-contract work.
