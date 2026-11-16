# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `ideas update` and `uservoice_update_idea` can set Idea custom fields (`cf_*`). `--custom-field cf_tfs_link=https://...` sends a string, `--custom-field-json cf_reviewed=true` sends a JSON literal (numbers and booleans, or `null` to clear; a timestamp takes a quoted date or RFC 3339 string and a unix number is rejected; an admin select takes the admin id; a select's options ship with the catalog), and MCP callers pass the `custom_fields` map. The values go into the update body as top-level keys — the only shape the API writes; a nested object is accepted with a 200 and silently ignored. A key that is not `cf_`-shaped is rejected before the request instead of writing nothing, the preview lists each field's before and after, and the values are part of the apply fingerprint.
- `workspace --include-catalog` and `uservoice_workspace_context` now list the Idea custom fields with their keys, types, value types, and select options, so a caller picks a key instead of guessing one. An API key that cannot read the custom-fields endpoint still gets the rest of the catalog: `missing_sections` names the section that was skipped, `partial_reason` says whether the failure is a throttle (retry), a permission problem (do not retry), or another failure, and the envelope status is `partial` with guidance that matches.
- `uservoice-cli skills` serves the agent-facing instructions compiled into the binary. `skills get core` prints the guide, `skills get core --ref <path>` loads one reference on demand, and `skills list` shows what is bundled. The text always matches the installed version, and the command needs no credentials, so it works before `login`.
- The corpus lives in `crates/rusty_user_voice/skills/`: the always-loaded `core` guide, five references (`cli`, `mcp-tools`, `pagination`, `errors`, `envelope`), and the specialized `triage` and `feedback-loop` workflows.
- `docs/agents/skills.md` states the maintenance contract, and `crates/rusty_user_voice/tests/skills_contract.rs` enforces it: a change to a flag, a tool, a reason token, or an envelope status that does not update the corpus fails the build. The test also parses every command in the corpus through the argument parser, so a command the CLI rejects fails the build too.
- The MCP server instructions point at `uservoice-cli skills get core`, so an MCP-only client can find the guide.

### Changed

- The user config and state directories are now named after the crate: `~/.config/rusty_user_voice/` and `rusty_user_voice` under the platform state dir, matching `rusty_azure_devops`. Both are migrated automatically on first use: a config left at the old `~/.config/rusty-user-voice/` path is moved with permissions re-asserted as 0600, and a state directory (the change ledger) is moved when only the old one exists. A `state_dir` you named yourself (flag, env, `uservoice_state_dir`) is never touched. `logout` removes the config from either location.
- `login` and `logout` success output now carries `legacy_warning`: `null` normally, and a `note:` line (printed in human output too) when a config still exists at the pre-rename `~/.config/rusty-user-voice/` path after the command ran. Migration leaves the file when the current config wins the path, and `logout` rechecks it after removing both files, so a surviving secret never stays invisible or reads as a clean logout.
- `skills/uservoice-pm/SKILL.md` is now a stub pointing at `uservoice-cli skills get core`. Its `references/` files moved into the binary, and `skills/uservoice-pm/references/` is gone.
- The CLI help lists every command with a description. `workspace`, `ideas`, and `feedback` had blank descriptions before.
- `--output ndjson` is now rejected for `skills`, with a message naming the supported modes. It was silently printing prose.
- The MCP server instructions mention the skill corpus.

### Internal

- `CommandError` moved from `login.rs` to `cli_error.rs`, with the reason tokens as named constants and a `COMMAND_REASONS` slice. The public shape is unchanged.
- `OutputMode::resolve` is the single owner of the output-mode precedence (flag, then `USERVOICE_DEFAULT_OUTPUT`, then `default_output` in `config.yaml`, then human). `login`, `logout`, and `skills` share it.
- The corpus layout is validated at build time. A file outside `<skill>/SKILL.md` and `<skill>/references/*.md`, or a skill without a document, fails the build instead of crashing the installed binary.
- The build script watches the corpus directory, so a newly added reference triggers a rebuild instead of being silently left out of the binary.

## [0.2.3] - 2026-11-08

### Changed

- Upgraded `rmcp` from 2.2.0 to 3.9.0. Replaced deprecated `ServerInfo` with `ServerConfig` in the MCP server handler. No public API changes.

## [0.2.2] - 2026-11-08

### Changed

- Updated all workspace dependencies to their latest compatible versions (reqwest 0.13.5, tokio 1.54.1, uuid 1.29.1, clap 4.6.9, and transitive crates). No public API changes.

### Fixed

- Bumped crate versions from 0.2.0 to 0.2.2 (the v0.2.1 tag was created without a manifest version bump).

## [0.2.1] - 2026-11-08

### Fixed

- HTTP clients no longer follow redirects: a 3xx from the API now surfaces as an error instead of re-sending the request (path, query, and for 307/308 the POST body) to an arbitrary host, enforcing the network boundary documented in `docs/security.md`.

## [0.2.0] - 2026-10-14

Adds `uservoice-cli login` / `logout` as the primary, agent-drivable authentication path. Prebuilt binaries land on the release; no existing config or MCP client behaviour changes.

### Added

- `uservoice-cli login` and `uservoice-cli logout` — first-run authentication UX writing `~/.config/rusty-user-voice/config.yaml` (mode 0600). ([#1](https://github.com/like-a-freedom/rusty_user_voice))
- `--api-key-stdin` reads the bearer key from a pipe, keeping it out of `argv` / shell history (agent-friendly; no `--api-key` flag by Credential Boundary).
- Interactive prompts for base URL and key when on a TTY; fail-fast `missing_value` (exit 1) when run headless.
- Server validation via `users/current` before saving; skip with `--no-verify` (offline URL/key/CA-cert validation still runs).
- `--force` overwrite guard returning `confirmation_required` (exit 3); on `--force`, non-auth keys (`uservoice_state_dir`, `uservoice_max_in_flight`, `default_output`, `log_level`) are preserved.
- Machine mode via `--output json` / `USERVOICE_DEFAULT_OUTPUT` / config `default_output`, with JSON errors on stderr (`{"ok":false,"reason","detail","hint"}`) and empty stdout on failure.
- ADR-0007 documenting the auth UX.

### Changed

- Toolchain now tracks `stable`; removed the fixed `rust-version` floor.
- Dependency refresh across the workspace.

## [0.1.0] - 2025-07-24

First public release. Initial MCP server, `uservoice-cli` workflows (workspace, search, get, collect, update preview/apply, publish, record feedback), change ledger, checkpoints, and redacted credential handling.
