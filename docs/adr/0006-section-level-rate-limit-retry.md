# ADR-0006: Section-level rate-limit retry is bounded (1 retry, then `partial`)

When a sub-request inside `get_idea_context`'s bounded fan-out (comments, notes, supporters, feedback records, status updates, activity) receives HTTP 429, the section waits `retry_after` (capped at the existing `MAX_RATE_LIMIT_SLEEP`) and retries exactly once. A second 429 — or any retry attempt that itself fails — drops the section into `Envelope.data.missing_sections` with `status = "partial"`, a `guidance` string pointing at the same `get_idea_context` call, and the `rate_limit` envelope populated so the agent can pace its next attempt.

## Status
Accepted — 2026-07-24

## Considered options

- **No retry; fall through to `missing_sections` on first 429** — simplest, preserves the existing fan-out latency. Rejected: UserVoice 429s are frequently transient (a brief burst from another admin session), and dropping a section on first contact forces the agent into an immediate second `get_idea_context` call that re-hits the same throttle.
- **Unbounded exponential backoff per section** — maximises recovery. Rejected: a single section stuck in backoff blocks the whole fan-out and can hold the process for minutes, which violates the agent-facing latency contract and the rate-limit hygiene rule (don't amplify load).
- **Bounded retry (1 attempt, then partial)** *(chosen)* — gives the upstream one chance to recover, caps the worst-case added latency at `MAX_RATE_LIMIT_SLEEP`, and surfaces the unrecoverable case as `partial` with a literal recovery `next_action`. Matches the existing top-level `get` retry policy in Task 10 (`MAX_READ_ATTEMPTS = 2`); the section fan-out simply inherits that bound.

## Consequences

- The retry helper introduced for the top-level `get` in Task 10 (`retry_delay`, `MAX_RATE_LIMIT_SLEEP`) is reused inside `collect_context`; no new constant is introduced.
- Each wait logs a `tracing::warn!` with `section`, `retry_after`, and `attempt = 1/1` so the rate-limit pattern is observable without leaking request bodies (Task 10's redact tests already cover this path).
- Writes (`update_idea`, `publish_idea_update`, `record_feedback`) are explicitly *not* covered by this policy — they remain non-retryable per the spec's mutation-safety rule. Only the read fan-out is affected.
- The `missing_sections` entry for a throttled section carries a distinct guidance string ("section throttled by upstream; retry `uservoice_get_idea_context` after the rate-limit window resets") so an agent can distinguish "upstream 500" from "upstream 429" in its recovery logic.
