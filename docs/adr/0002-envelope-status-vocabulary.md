# ADR-0002: Envelope carries `OutcomeStatus`; the MCP wire token stays the mcp-design default

The plan defines an internal `OutcomeStatus` enum (`Ok | Partial | NoResults | Conflict`) on `Envelope<T>` and uses it in the application layer, but the MCP tool surface still serialises the canonical {@"status": "ok" | "partial" | "no_results" | "conflict"} pairs that the mcp-design skill publishes. The internal enum and the wire token are decoupled so that error variants required by the application layer (`NoResults` for empty reads, `Conflict` for state mismatches) survive even if the MCP spec later tightens or loosens its allowed status values. The cost is a single mapping step in the `UserVoiceMcpServer` glue; the benefit is that the application's outcome vocabulary is not hostage to the transport's vocabulary.

## Status
Accepted — 2026-07-24

## Considered options

- **Use `OutcomeStatus` directly on the wire** — drops the `From<OutcomeStatus> for McpWireStatus` step and saves a small amount of code. Rejected: it leaks internal vocabulary into the public MCP schema, which an MCP-rule checker would then flag as a deviation.
- **Drop `OutcomeStatus`; use `McpWireStatus` everywhere** — keeps the wire schema clean. Rejected: `NoResults` (empty search result, not a failure) is a state the application genuinely needs to distinguish from `Ok` (data was present) and from `Error`, and the MCP status token has no slot for it.
- **Both** *(chosen)* — `OutcomeStatus` is the application-layer source of truth; the MCP server translates to the wire-pair at the seam.

## Consequences

- A function that takes an `Envelope<T>` and serialises it for MCP must map `OutcomeStatus::*` to the wire token; this is one helper, not a per-tool mapping.
- `README`, MCP schema docs, and the `uservoice-pm` skill must adopt the wire vocabulary externally; code comments may use either, but tests assert against the wire token when checking the agent-visible shape.
- If the MCP spec later renames `partial` or `not_found`, the application layer is unaffected.
