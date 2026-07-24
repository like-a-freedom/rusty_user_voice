# ADR-0001: Workspace crate layout — `uservoice_client` + `rusty_user_voice` instead of `core`/`server`/`cli`

The plan adopts `crates/uservoice_client` (anti-corruption / HTTP adapter) and `crates/rusty_user_voice` (application shell exposing both MCP and CLI), deliberately departing from the `modern-rust-repo-structure` skill's `crates/core`, `crates/server`, `crates/cli` template. The skill's names imply a *protocol-shape* layout (protocol core vs. transport surface); this project instead splits by *DDD layer* (adapter vs. application), which keeps the MCP and CLI surfaces sharing one workflow code path with a single typed `App<G>` seam. The deviation is deliberate, the cost of a future rename is one afternoon of `cargo rename`, and any reader of `Cargo.toml` would otherwise assume the wrong mental model of where business rules live.

## Status
Accepted — 2026-07-24

## Considered options

- **`crates/core` + `crates/server` + `crates/cli`** — skill default. Splits along protocol surfaces; the same `App` would be imported by both `server` and `cli`, forcing a fourth crate (`app`) or a circular dependency.
- **`crates/adapter` + `crates/app` + `crates/{mcp,cli}`** — strictly layered. Clean, but introduces three small surface crates where one suffices; matches neither the skill nor existing Rusty-* family naming.
- **`crates/uservoice_client` + `crates/rusty_user_voice`** *(chosen)* — `uservoice_client` is the adapter (DDD anti-corruption layer around the upstream HTTP API); `rusty_user_voice` is the application (`App<G>`, MCP server, CLI binary). Aligns with sibling repositories (`rusty_sender`, `rusty_intervals_mcp`) which follow the same `rusty_*` convention.

## Consequences

- A future reader comparing this repo to the skill template will not find the expected crate names; the deviation must be discoverable, which is why this ADR exists next to `CONTEXT.md`.
- All shared types (`SearchIdeasParams`, `OutcomeStatus`, `Envelope`, the `UserVoiceApi` trait) live in `rusty_user_voice`; pulling them out into a `core` crate is a non-breaking later change if the layout ever needs to scale to a second binary.
- The MCP server and the CLI share the same `App<G>` via a single `mount_*` switchboard (Task 9), which is the *payoff* of the chosen layout: one ergonomic seam, not two divergent code paths.
