# ADR-0007: `uservoice-cli login` as primary auth UX — CLI-only, agent-drivable

Status: proposed

Authentication currently requires hand-editing `config.yaml` with a bearer key —
there is no writer for config. `gcx login`, `pangolin login` and `keenable login`
all solve this with a CLI-only login command, none exposed as an MCP tool. This
repo additionally forbids the key as a CLI flag (Credential Boundary:
`docs/security.md`, spec line 92, enforced by `api_key_is_not_a_cli_argument`),
so the automation path cannot be `--api-key`.

We add `uservoice-cli login` / `uservoice-cli logout`. The key resolves as
`--api-key-stdin` (the `gh --with-token` pattern) > `USERVOICE_API_KEY` env >
hidden prompt; the base URL resolves as login `--base-url` > top-level
`--base-url` > `USERVOICE_BASE_URL` env > prompt — with prompts only on a TTY: a
headless run with missing values fails fast with `missing_value` instead of
blocking the caller's turn. Local validation (URL shape, key, CA certificate)
runs on every login; only the `current_user` network preflight is skippable
with `--no-verify`, so `--no-verify` can never persist a config that fails on
the first real command. Existing config is never overwritten without `--force`
(`confirmation_required`, exit 3); `--force` preserves the keys login does not
own (`uservoice_state_dir`, `uservoice_max_in_flight`, `default_output`,
`log_level`).

Agent-UX research (dosu ticket protocol + `agent_next_steps`; dual-mode
paradigm; `gh --with-token`; voxgig agent-drivable CLI rules) shaped the error
contract: machine mode shapes success *and* failure — errors go to stderr as
`{"ok":false,"reason","detail","hint"}` with stable reason tokens, stdout stays
empty on failure, exit codes are 0/1/2/3, and success carries a `next` verify
command. Machine mode is the repo's existing `--output json` (with the same
env/config fallbacks every other command uses), not a new flag. `ToolName::ALL`
is unchanged (7 variants); `tests/mcp_contract.rs` keeps an explicit `CLI_ONLY`
list so login/logout can never acquire an MCP counterpart. Success output
carries only the numeric `user_id` — never a name or email.

## Rejected

OS-keychain storage: the MCP server and CI consume file+env credentials, so
keyring would add platform dependencies and break headless runs; a 0600 file
(enforced at creation, re-asserted on rewrite) is the pragmatic baseline.

Browser/device approval flows: impossible against static bearer keys — there is
no approval authority to bind a session to. The headless equivalent is env or
`--api-key-stdin`.

An `--api-key` flag (the `keenable login --api-key` shape): rejected by the
Credential Boundary — argv leaks via `ps` and shell history. This is the one
deliberate divergence from `2026-10-14-ado-cli-login.md`.

Duplicating the HTTP client inside `login.rs`: rejected — `bootstrap::build_http_client`
is the single owner of the user-agent and corporate CA certificate, so login
validation uses the exact TLS setup the running app will use.

## Consequences

`config.rs` gains save helpers and `YamlConfig` becomes `Serialize`; `login.rs`
becomes the flow's single owner (`run_login`/`run_logout` plus testable
primitives); `bootstrap.rs` gains `build_http_client`; one new dependency
(`rpassword`); README, `CONTEXT.md`, `docs/security.md` and the spec gain the
first-run path, the agent recipe, and the Login/UserConfig vocabulary.
