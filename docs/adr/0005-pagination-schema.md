# ADR-0005: Pagination schema stays cursor-friendly (`per_page`, `total_records`, `next`, `has_more`)

`Envelope.pagination` keeps the plan's existing `{ per_page, total_records, next }` shape and gains one additional derived boolean `has_more`, rather than adopting the mcp-design checklist's literal `{ has_more, total_count, next_offset }` tuple. `next` is an opaque continuation token (cursor or page number, depending on what UserVoice returned in the `Link` header); forcing it into a numeric `next_offset` would silently break the cursor branch of `Continuation` (the part that already won over page-based in the Task 2 contract tests) the first time a tenant switches its default pagination mode.

## Status
Accepted — 2026-07-24

## Considered options

- **Adopt mcp-design literal field names** (`has_more`, `total_count`, `next_offset`) — would close the cosmetic conformance gap with the skill's evaluation checklist. Rejected: `next_offset` implies numeric offset semantics, which UserVoice's cursor tokens cannot honour. Adopting the literal name would force a `u64` field that we lie about.
- **Keep `{ per_page, total_records, next }` as-is** — zero churn. Rejected: agents reading the schema have to compute "is there more?" themselves, which is the single most common question and exactly what `has_more` exists to answer.
- **Keep current names + add `has_more: bool`** *(chosen)* — `has_more` is `next.is_some()` and therefore free to compute, keeps the cursor-friendly `next` token, and gives agents the one boolean they actually need. `per_page` and `total_records` stay because they carry information `has_more` does not (page framing for batch sizing, total for progress UIs).

## Consequences

- `Pagination` struct gains `has_more: bool`; the constructor computes it from `next.is_some()` so it cannot drift.
- The MCP JSON schema documents `next` as "opaque continuation token (cursor or page); pass back verbatim as `continuation`". No claim of numeric offset semantics.
- The mcp-design evaluation checklist item "Paginated list tools include `has_more`, `total_count`, `next_offset`" is recorded as a *partial deviation*: `has_more` ✓, `total_count` ≡ `total_records` ✓, `next_offset` deliberately omitted with this ADR as the justification.
- `OutputMode::Human` printer shows `has_more=true records=N/M` so a human sees at a glance whether the run is complete.
