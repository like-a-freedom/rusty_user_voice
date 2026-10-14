# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
