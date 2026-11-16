# ADR-0009: Config and state directories follow the crate name, with migration

Status: proposed

## Context

`rusty_azure_devops` keeps its user-level state under directories named after the
crate: `~/.config/rusty_azure_devops/`, `rusty_azure_devops` under the platform
state dir. This repo used `~/.config/rusty-user-voice/` and `rusty-user-voice`
under the state dir — the same project spelled two ways across the two
repositories, and neither spelling matching the crate `rusty_user_voice`.

The hyphenated spelling was chosen deliberately at login time
(`docs/superpowers/plans/2026-10-14-uservoice-cli-login.md`: "Never copy
`rusty_azure_devops` strings verbatim"). That concern was about copying azure's
*strings* — env var names, config keys, user agent — not about directory
naming. The directories are ours, and the divergence costs a reader a second
guess every time they open either `~/.config`.

Users already have a login at the old path. A rename that ignores it logs every
existing install out.

## Decision

Directories are named after the crate: `CONFIG_DIR_NAME = "rusty_user_voice"`
for both `~/.config/<name>/config.yaml` and the platform state dir.
`LEGACY_CONFIG_DIR_NAME = "rusty-user-voice"` exists only for the migration.

Both migrate automatically, once:

- **Config** — `migrate_legacy_user_config()` runs before every read
  (`load_yaml`, `load_user_config_raw`) and before the `login` overwrite guard.
  The file is *moved*, not copied, so the bearer token is not left in two
  places, and permissions are re-asserted as 0600 because the legacy copy may
  have been created laxer. If a config already exists at the new path, the
  current one wins and the legacy file is left alone — deleting it would destroy
  a credential the user may still want. `logout` removes it from either
  location, or the secret would survive the logout.
- **State dir** — `migrate_legacy_state_dir()` runs after the state dir is
  resolved, and only when the resolved path *is* the platform default. A
  `state_dir` named by the user (flag, env, `uservoice_state_dir`) is never
  touched: renaming it would move the change ledger away from the path they
  asked for. The directory is moved only when the new one does not exist, so a
  partially upgraded install is never overwritten.

`USERVOICE_*` env vars, `uservoice_*` config keys, and the `rusty-user-voice`
user agent are unchanged — they are part of the published contract, and azure
sets no user agent to align with.

## Consequences

- An existing install keeps its login and its ledger across the upgrade; the
  first read moves them and tightens the config to 0600.
- The legacy directory may remain on disk, empty, after a migration. Nothing
  deletes it; only `logout` removes the config file itself.
- The migration is stat-only when there is nothing to move, so the steady-state
  cost is two `exists()` calls per settings load.
- Tests cover the move, the current-config-wins case, the no-op case, and the
  0600 re-assertion, for both the config file and the state directory.
