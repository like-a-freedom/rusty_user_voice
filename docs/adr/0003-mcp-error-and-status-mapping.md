# ADR-0003: `AppError → McpError` mapping must stop mixing upstream `NotFound` into `invalid_params`

Application-level validation failures continue to return RMCP `invalid_params`; everything that originates from the upstream API, the filesystem, or the ledger — including the `NotFound` a 404 produces and the `Conflict` raised when a preview is stale — returns RMCP `internal_error` with an accompanying `Envelope` payload that carries `status = "not_found"` or `status = "conflict"` plus an actionable `guidance` field. The plan currently maps both `Conflict` and `ClientError::NotFound` to `invalid_params`, which (a) contradicts the spec's own rule that only application parameter validation uses `invalid_params`, and (b) breaks the agent pattern of using `next_actions` to recover, since `invalid_params` is a hard-stop signal in MCP while `internal_error` carries the structured envelope the agent needs.

## Status
Accepted — 2026-07-24

## Considered options

- **Keep the current mapping (`Validation` + `Conflict` + `NotFound` → `invalid_params`)** — single error code, no envelope on failure. Rejected: it makes `Conflict("change_id has expired")` look like a client-side parameter mistake, which it is not; an agent following the MCP rules will abort retries rather than re-running `preview`, exactly the wrong recovery.
- **Map every error to `internal_error`** — uniform, never wrong. Rejected: genuine client-side mistakes (an unknown idea ID submitted without `uservoice_search_ideas` first, a `per_page` of 0 in CLI) deserve `invalid_params` so agents don't retry-then-give-up on a bug in their own call.
- **Split: `Validation(msg)` only → `invalid_params`; everything else → `internal_error` plus a structured envelope** *(chosen)*. The envelope has `status ∈ {not_found, conflict, error}`, optionally a `guidance` string, and a `next_actions` array when there is a concrete recovery step. This satisfies the mcp-design "every response is guidance" rule and matches the wording the spec already uses.

## Consequences

- `From<AppError> for McpError` becomes a 2-arm split (`Validation` parameter, all other variants). All other variants emit `internal_error(display)` along with a structured payload attached via the `data` field of `McpError` (RMCP 2.2 supports this).
- The `Conflict` and `NotFound` variants each carry a domain-specific `guidance` string: `"idea {id} could not be loaded — call uservoice_search_ideas to refresh the idea_id"` and `"preview change_id {id} has expired — call uservoice_update_idea with mode=preview again"`.
- A single test (`errors_never_emit_token_email_or_feedback_body` in Task 10) already asserts no token or body leaks; the new tests must additionally assert (i) that `NotFound` is wrapped in an `Envelope` with `status = "not_found"` and `guidance`, and (ii) that `Conflict("change_id has expired")` preserves the `expires_at` from the failed preview so the agent can construct a fresh preview without an extra `search`.
- `Conflict` raised by `PaginationLimit` (Task 4 collector safety cap) now also flows through this path: status `"conflict"`, guidance `"section exceeded 100-page safety ceiling; narrow the time window"`. The cap itself is unchanged.
- The MCP server's mcp.rs helper (`assert_matches_output_schema`) needs an extra arm to keep the schema valid for the `not_found` / `conflict` tokens.
