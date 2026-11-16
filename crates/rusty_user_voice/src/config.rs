use std::{env, io::Write, path::PathBuf};

use uservoice_client::{AccessToken, ApiEndpoint};

use crate::error::AppError;

pub(crate) const CONFIG_FILENAME: &str = "config.yaml";

/// The directory name of the user-level config and the default state
/// directory. It matches the crate name, the way `rusty_azure_devops` names
/// its own directories.
const CONFIG_DIR_NAME: &str = "rusty_user_voice";

/// The config directory name used before the rename to [`CONFIG_DIR_NAME`].
/// A config file left there is migrated on first use.
const LEGACY_CONFIG_DIR_NAME: &str = "rusty-user-voice";

// Serialize + crate-visible fields + omit absent optionals: a `null` key
// would be noise, and missing Option<T> keys already load back as None.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default)]
pub(crate) struct YamlConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) uservoice_api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) uservoice_base_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) uservoice_state_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) uservoice_max_in_flight: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) default_output: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) ca_cert: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) log_level: Option<String>,
}

#[derive(Clone)]
pub struct Settings {
    pub endpoint: ApiEndpoint,
    pub token: AccessToken,
    pub state_dir: PathBuf,
    pub max_in_flight: usize,
    pub default_output: Option<String>,
    pub log_level: Option<String>,
    pub ca_cert: Option<String>,
}

impl Settings {
    pub fn from_sources(
        base_url_override: Option<&str>,
        state_dir_override: Option<PathBuf>,
        max_in_flight_override: Option<u32>,
    ) -> Result<Self, AppError> {
        let yaml = Self::load_yaml().unwrap_or_default();

        // api_key: env var > config.yaml (NOT CLI flags — security rule)
        let api_key = env::var("USERVOICE_API_KEY")
            .or_else(|_| yaml.uservoice_api_key.clone().ok_or(()))
            .map_err(|_| {
                AppError::Validation(
                    "USERVOICE_API_KEY is required: set via env var or config.yaml".to_owned(),
                )
            })?;

        // base_url: CLI flag > env var > config.yaml
        let base_url = base_url_override
            .map(str::to_owned)
            .or_else(|| env::var("USERVOICE_BASE_URL").ok())
            .or(yaml.uservoice_base_url)
            .ok_or_else(|| {
                AppError::Validation(
                    "USERVOICE_BASE_URL is required: set via --base-url, env var, or config.yaml"
                        .to_owned(),
                )
            })?;

        // state_dir: CLI flag > env var > config.yaml > platform default
        let state_dir = state_dir_override
            .or_else(|| env::var_os("USERVOICE_STATE_DIR").map(PathBuf::from))
            .or_else(|| yaml.uservoice_state_dir.map(PathBuf::from))
            .or_else(platform_state_dir)
            .ok_or_else(|| {
                AppError::Validation(
                    "USERVOICE_STATE_DIR is required when no platform state directory exists"
                        .to_owned(),
                )
            })?;
        let state_dir = migrate_legacy_state_dir(&state_dir);

        // max_in_flight: CLI flag > env var > config.yaml > default(2)
        let max_in_flight = max_in_flight_override
            .or_else(|| {
                env::var("USERVOICE_MAX_IN_FLIGHT")
                    .ok()
                    .and_then(|v| v.parse().ok())
            })
            .or(yaml.uservoice_max_in_flight)
            .unwrap_or(2)
            .clamp(1, 8);

        // default_output: env var > config.yaml > default(human)
        let default_output = env::var("USERVOICE_DEFAULT_OUTPUT")
            .ok()
            .filter(|v| !v.is_empty())
            .or(yaml.default_output);

        // log_level: env var > config.yaml (RUST_LOG handled by tracing separately)
        let log_level = env::var("USERVOICE_LOG_LEVEL")
            .ok()
            .filter(|v| !v.is_empty())
            .or(yaml.log_level);

        // ca_cert: config.yaml only
        let ca_cert = yaml.ca_cert;

        Ok(Self {
            endpoint: ApiEndpoint::parse(&base_url)?,
            token: AccessToken::parse(api_key)?,
            state_dir,
            max_in_flight: max_in_flight as usize,
            default_output,
            log_level,
            ca_cert,
        })
    }

    pub fn from_values(
        base_url: &str,
        api_key: &str,
        state_dir: Option<PathBuf>,
        max_in_flight: Option<u32>,
    ) -> Result<Self, AppError> {
        let state_dir = state_dir.or_else(platform_state_dir).ok_or_else(|| {
            AppError::Validation(
                "USERVOICE_STATE_DIR is required when no platform state directory exists"
                    .to_owned(),
            )
        })?;
        let state_dir = migrate_legacy_state_dir(&state_dir);
        Ok(Self {
            endpoint: ApiEndpoint::parse(base_url)?,
            token: AccessToken::parse(api_key.to_owned())?,
            state_dir,
            max_in_flight: max_in_flight.unwrap_or(2).clamp(1, 8) as usize,
            default_output: None,
            log_level: None,
            ca_cert: None,
        })
    }

    fn load_yaml() -> Option<YamlConfig> {
        // Migrate before anything else: a config left at the legacy path moves
        // into the current one even when a deployed config next to the binary
        // shadows the user file — otherwise `logout` would never see it.
        migrate_legacy_user_config();
        if let Some(path) = Self::config_path()
            && let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(yaml) = serde_yaml::from_str(&content)
        {
            return Some(yaml);
        }
        read_user_config_content().and_then(|content| serde_yaml::from_str(&content).ok())
    }

    fn config_path() -> Option<PathBuf> {
        let exe_dir = std::env::current_exe().ok()?;
        let dir = exe_dir.parent()?;
        Some(dir.join(CONFIG_FILENAME))
    }

    fn config_path_home() -> Option<PathBuf> {
        Some(
            home_dir()?
                .join(".config")
                .join(CONFIG_DIR_NAME)
                .join(CONFIG_FILENAME),
        )
    }
}

/// User-level config path: ~/.config/rusty_user_voice/config.yaml
/// (login always writes here — never next to the binary).
pub(crate) fn user_config_path() -> Option<std::path::PathBuf> {
    Settings::config_path_home()
}

/// The user-level config directory: ~/.config/rusty_user_voice
///
/// The directory is named after the crate, with underscores, the same way
/// `rusty_azure_devops` names its own. The earlier `rusty-user-voice`
/// spelling is read as a legacy path and migrated on first use.
fn user_config_dir() -> Option<std::path::PathBuf> {
    Some(home_dir()?.join(".config").join(CONFIG_DIR_NAME))
}

/// The legacy user-level config directory: ~/.config/rusty-user-voice
///
/// Kept for the migration only. A config file that predates the rename is
/// moved to [`user_config_dir`] the first time the tool reads it.
pub(crate) fn legacy_user_config_dir() -> Option<std::path::PathBuf> {
    Some(home_dir()?.join(".config").join(LEGACY_CONFIG_DIR_NAME))
}

/// The platform default state directory: `<state>/rusty_user_voice`
/// (falling back to the local data dir where there is no state dir).
fn platform_state_dir() -> Option<PathBuf> {
    dirs::state_dir()
        .map(|path| path.join(CONFIG_DIR_NAME))
        .or_else(|| dirs::data_local_dir().map(|path| path.join(CONFIG_DIR_NAME)))
}

/// The state directory used before the rename: `<state>/rusty-user-voice`.
fn legacy_platform_state_dir() -> Option<PathBuf> {
    dirs::state_dir()
        .map(|path| path.join(LEGACY_CONFIG_DIR_NAME))
        .or_else(|| dirs::data_local_dir().map(|path| path.join(LEGACY_CONFIG_DIR_NAME)))
}

/// Move the legacy state directory into `current`, once, and return the
/// directory to use.
///
/// Only the platform default is migrated: a `state_dir` the user chose
/// themselves (flag, env, config) is theirs to move, and renaming it would
/// take their ledger away from the path they named.
fn migrate_legacy_state_dir(current: &std::path::Path) -> PathBuf {
    migrate_state_dir(
        current,
        platform_state_dir().as_deref(),
        legacy_platform_state_dir().as_deref(),
    )
}

/// Move `legacy` into `current` when `current` is the platform default and
/// only `legacy` exists. Returns the directory the caller should use.
///
/// The ledger is local history, not a credential: it is moved rather than
/// left behind so previews recorded before the rename are not silently
/// dropped. Nothing is deleted. When the move fails, `legacy` is returned —
/// keeping the old ledger beats starting an empty one in the new path, and a
/// failed rename is logged too late to be seen (tracing starts after the
/// settings load), so the caller must not depend on it.
fn migrate_state_dir(
    current: &std::path::Path,
    platform_default: Option<&std::path::Path>,
    legacy: Option<&std::path::Path>,
) -> PathBuf {
    if platform_default != Some(current) {
        return current.to_owned();
    }
    let Some(legacy) = legacy else {
        return current.to_owned();
    };
    if legacy == current || !legacy.exists() || current.exists() {
        return current.to_owned();
    }
    match std::fs::rename(legacy, current) {
        Ok(()) => current.to_owned(),
        Err(error) => {
            tracing::warn!(
                "cannot migrate state dir {} to {}: {error}",
                legacy.display(),
                current.display()
            );
            legacy.to_owned()
        }
    }
}

/// The home directory, on Unix or Windows.
fn home_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())?;
    if home.is_empty() {
        None
    } else {
        Some(PathBuf::from(home))
    }
}

/// Move a legacy user config into the current directory.
///
/// Does nothing when either directory cannot be resolved or there is
/// nothing to move.
pub(crate) fn migrate_legacy_user_config() {
    let (Some(legacy), Some(current)) = (legacy_user_config_dir(), user_config_dir()) else {
        return;
    };
    migrate_at(&legacy, &current);
}

/// Read the user-level config file: the current path first, the legacy path
/// as a fallback.
///
/// The fallback covers a migration that could not complete (permissions, a
/// concurrent writer). A rename failure must not read as a lost login, so
/// the legacy file is read in place until the move succeeds.
fn read_user_config_content() -> Option<String> {
    if let Some(path) = Settings::config_path_home()
        && let Ok(content) = std::fs::read_to_string(&path)
    {
        return Some(content);
    }
    let legacy = legacy_user_config_dir()?.join(CONFIG_FILENAME);
    std::fs::read_to_string(legacy).ok()
}

/// Move the config of `legacy_dir` into `current_dir`.
///
/// The file is moved, not copied, so the secret does not stay behind in two
/// places. The permissions are re-asserted as 0600, because the file carries a
/// bearer token and the legacy copy may have been created with a laxer mode.
///
/// Returns the path the config was moved to, or `None` when there was
/// nothing to migrate.
fn migrate_at(legacy_dir: &std::path::Path, current_dir: &std::path::Path) -> Option<PathBuf> {
    let legacy_path = legacy_dir.join(CONFIG_FILENAME);
    if !legacy_path.exists() {
        return None;
    }

    let current_path = current_dir.join(CONFIG_FILENAME);
    if current_path.exists() {
        // The current config wins. The legacy file stays where it is: removing
        // it would delete a credential the user may still want.
        return None;
    }

    // `rename` needs the target directory itself to exist, not just its parent.
    if let Err(error) = std::fs::create_dir_all(current_dir) {
        tracing::warn!("cannot create {}: {error}", current_dir.display());
        return None;
    }
    if let Err(error) = std::fs::rename(&legacy_path, &current_path) {
        tracing::warn!("cannot migrate {}: {error}", legacy_path.display());
        return None;
    }
    tighten_permissions(&current_path);
    Some(current_path)
}

/// Re-assert owner-only permissions on a file that carries a secret.
fn tighten_permissions(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
            tracing::warn!("cannot chmod 0600 {}: {error}", path.display());
        }
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Binary-dir config path (next to the executable). Exposed read-only so
/// login/logout can report that a deployed config shadows the user file.
pub(crate) fn binary_config_path() -> Option<std::path::PathBuf> {
    Settings::config_path()
}

/// The `default_output` of the effective config file, read without loading
/// credentials: the binary-dir file first, then the user file, exactly like
/// [`Settings::from_sources`]. Used by the commands that run before the
/// settings load (`login`, `logout`, `skills`).
pub(crate) fn config_default_output() -> Option<String> {
    Settings::load_yaml().and_then(|config| config.default_output)
}

/// Raw user-file read for the `--force` merge (decision 13). `None` when the
/// file is absent or unparseable — login then writes a fresh config.
pub(crate) fn load_user_config_raw() -> Option<YamlConfig> {
    // Migrate first: `login --force` merges the keys it does not own out of
    // the existing config, and that config may still sit at the legacy path.
    migrate_legacy_user_config();
    let content = read_user_config_content()?;
    serde_yaml::from_str(&content).ok()
}

/// Write `cfg` as YAML to an explicit `path`, creating parent dirs. The file
/// is created 0600 on Unix (the bearer key is a secret) and perms are
/// re-asserted after the write so a pre-existing 0644 file is tightened too.
pub(crate) fn save_to(path: &std::path::Path, cfg: &YamlConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create config dir {}: {}", parent.display(), e))?;
    }
    let content =
        serde_yaml::to_string(cfg).map_err(|e| format!("cannot serialize config: {}", e))?;
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Mode at creation: never leave a 0644 window on disk.
        opts.mode(0o600);
    }
    let mut f = opts
        .open(path)
        .map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    f.write_all(content.as_bytes())
        .map_err(|e| format!("cannot write {}: {}", path.display(), e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("cannot chmod 0600 {}: {}", path.display(), e))?;
    }
    Ok(())
}

/// Serialize `cfg` to the user config path. Overwrites unconditionally —
/// the caller (login) owns the --force guard.
pub(crate) fn save_user_config(cfg: &YamlConfig) -> Result<std::path::PathBuf, String> {
    let path = Settings::config_path_home()
        .ok_or_else(|| "cannot resolve home directory for config save".to_string())?;
    save_to(&path, cfg)?;
    Ok(path)
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Settings")
            .field("endpoint", &self.endpoint)
            .field("token", &self.token)
            .field("state_dir", &self.state_dir)
            .field("max_in_flight", &self.max_in_flight)
            .field("default_output", &self.default_output)
            .field("log_level", &self.log_level)
            .field("ca_cert", &self.ca_cert.as_deref().unwrap_or("(none)"))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config_dir(tag: &str) -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "uv-cfg-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The legacy config directory inside a fake home.
    fn legacy_dir(home: &std::path::Path) -> std::path::PathBuf {
        home.join(".config").join(LEGACY_CONFIG_DIR_NAME)
    }

    /// The current config directory inside a fake home.
    fn current_dir(home: &std::path::Path) -> std::path::PathBuf {
        home.join(".config").join(CONFIG_DIR_NAME)
    }

    fn sample(key: &str) -> YamlConfig {
        YamlConfig {
            uservoice_api_key: Some(key.to_owned()),
            uservoice_base_url: Some("https://acme.uservoice.com".to_owned()),
            uservoice_state_dir: None,
            uservoice_max_in_flight: Some(4),
            default_output: Some("json".to_owned()),
            ca_cert: None,
            log_level: None,
        }
    }

    #[test]
    fn save_to_roundtrip_with_owner_only_perms() {
        let dir = temp_config_dir("roundtrip");
        let path = dir.join("config.yaml");
        save_to(&path, &sample("uv-pat123")).expect("save works");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("uservoice_base_url"));
        assert!(content.contains("uv-pat123"));
        // Absent optionals are omitted, not written as `null` (skip_serializing_if).
        assert!(!content.contains("null"));
        // Roundtrip: what we wrote parses back through the real loader path.
        let parsed: YamlConfig = serde_yaml::from_str(&content).unwrap();
        assert_eq!(
            parsed.uservoice_base_url.as_deref(),
            Some("https://acme.uservoice.com")
        );
        assert_eq!(parsed.uservoice_max_in_flight, Some(4));
        assert!(parsed.ca_cert.is_none());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "API-key file must be owner-only");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_to_reasserts_perms_on_preexisting_loose_file() {
        let dir = temp_config_dir("loose");
        let path = dir.join("config.yaml");
        std::fs::write(&path, "uservoice_api_key: old\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        }
        save_to(&path, &sample("new-key")).expect("save over existing");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "pre-existing 0644 file must be tightened");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn the_migration_moves_a_legacy_config_into_the_current_directory() {
        let home = temp_config_dir("migrate");
        let legacy = legacy_dir(&home);
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(
            legacy.join(CONFIG_FILENAME),
            "uservoice_api_key: legacy-key\n",
        )
        .unwrap();

        // The test drives the same helper the loader calls, against temp
        // paths rather than the real home directory.
        let moved = migrate_at(&legacy, &current_dir(&home));
        assert_eq!(moved, Some(current_dir(&home).join(CONFIG_FILENAME)));
        assert!(
            !legacy.join(CONFIG_FILENAME).exists(),
            "the legacy file must be moved, not copied"
        );
        let content = std::fs::read_to_string(current_dir(&home).join(CONFIG_FILENAME)).unwrap();
        assert!(content.contains("legacy-key"));
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn the_migration_keeps_the_current_config_when_there_is_one() {
        let home = temp_config_dir("migrate-keep");
        let legacy = legacy_dir(&home);
        let current = current_dir(&home);
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(legacy.join(CONFIG_FILENAME), "uservoice_api_key: legacy\n").unwrap();
        std::fs::write(
            current.join(CONFIG_FILENAME),
            "uservoice_api_key: current\n",
        )
        .unwrap();

        assert_eq!(migrate_at(&legacy, &current), None);
        let content = std::fs::read_to_string(current.join(CONFIG_FILENAME)).unwrap();
        assert!(content.contains("current"), "the current config must win");
        assert!(
            legacy.join(CONFIG_FILENAME).exists(),
            "the legacy file must be left alone, not deleted"
        );
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn the_migration_does_nothing_without_a_legacy_config() {
        let home = temp_config_dir("migrate-none");
        let legacy = legacy_dir(&home);
        std::fs::create_dir_all(&legacy).unwrap();

        assert_eq!(migrate_at(&legacy, &current_dir(&home)), None);
        assert!(!current_dir(&home).join(CONFIG_FILENAME).exists());
        std::fs::remove_dir_all(&home).ok();
    }

    #[cfg(unix)]
    #[test]
    fn the_migration_tightens_the_permissions_of_the_moved_file() {
        use std::os::unix::fs::PermissionsExt;
        let home = temp_config_dir("migrate-perms");
        let legacy = legacy_dir(&home);
        std::fs::create_dir_all(&legacy).unwrap();
        let legacy_file = legacy.join(CONFIG_FILENAME);
        std::fs::write(&legacy_file, "uservoice_api_key: legacy\n").unwrap();
        std::fs::set_permissions(&legacy_file, std::fs::Permissions::from_mode(0o644)).unwrap();

        migrate_at(&legacy, &current_dir(&home));
        let moved = current_dir(&home).join(CONFIG_FILENAME);
        let mode = std::fs::metadata(&moved).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "the moved API-key file must be owner-only");
        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn the_state_dir_migration_moves_the_legacy_ledger_into_place() {
        let root = temp_config_dir("state-migrate");
        let legacy = root.join(LEGACY_CONFIG_DIR_NAME);
        let current = root.join(CONFIG_DIR_NAME);
        std::fs::create_dir_all(legacy.join("ledger")).unwrap();
        std::fs::write(legacy.join("ledger").join("a.json"), "{}").unwrap();

        let used = migrate_state_dir(&current, Some(&current), Some(&legacy));

        assert_eq!(used, current, "the new directory is the one to use");
        assert!(!legacy.exists(), "the legacy state dir must be moved");
        let content = std::fs::read_to_string(current.join("ledger").join("a.json")).unwrap();
        assert_eq!(content, "{}");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_state_dir_migration_keeps_an_existing_state_dir() {
        let root = temp_config_dir("state-keep");
        let legacy = root.join(LEGACY_CONFIG_DIR_NAME);
        let current = root.join(CONFIG_DIR_NAME);
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::create_dir_all(&current).unwrap();
        std::fs::write(legacy.join("a.json"), "old").unwrap();
        std::fs::write(current.join("a.json"), "new").unwrap();

        let used = migrate_state_dir(&current, Some(&current), Some(&legacy));

        assert_eq!(used, current);
        assert!(legacy.exists(), "the current state dir wins");
        let content = std::fs::read_to_string(current.join("a.json")).unwrap();
        assert_eq!(content, "new");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_state_dir_migration_does_nothing_without_a_legacy_dir() {
        let root = temp_config_dir("state-none");
        let legacy = root.join(LEGACY_CONFIG_DIR_NAME);
        let current = root.join(CONFIG_DIR_NAME);
        std::fs::create_dir_all(&current).unwrap();

        assert_eq!(
            migrate_state_dir(&current, Some(&current), Some(&legacy)),
            current
        );
        assert_eq!(migrate_state_dir(&current, Some(&current), None), current);
        assert!(!legacy.exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn the_state_dir_migration_leaves_a_user_named_directory_alone() {
        let root = temp_config_dir("state-named");
        let legacy = root.join(LEGACY_CONFIG_DIR_NAME);
        let named = root.join("my-own-ledger");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("a.json"), "old").unwrap();

        // The resolved dir is not the platform default, so it is the user's
        // and the legacy ledger must stay where they left it.
        let platform_default = root.join(CONFIG_DIR_NAME);
        let used = migrate_state_dir(&named, Some(&platform_default), Some(&legacy));

        assert_eq!(used, named, "the directory the user named is returned");
        assert!(legacy.exists(), "the legacy ledger must not be moved");
        assert!(!named.exists(), "nothing may be created at the named path");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn a_failed_state_dir_move_keeps_the_legacy_ledger() {
        let root = temp_config_dir("state-fail");
        let legacy = root.join(LEGACY_CONFIG_DIR_NAME);
        // A target whose parent does not exist makes `rename` fail, which is
        // the shape of a permissions or mount error in the field.
        let current = root.join("missing-parent").join(CONFIG_DIR_NAME);
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("a.json"), "old").unwrap();

        let used = migrate_state_dir(&current, Some(&current), Some(&legacy));

        assert_eq!(
            used, legacy,
            "a failed move must keep the old ledger in use, not start an empty one"
        );
        assert!(legacy.join("a.json").exists(), "nothing may be lost");
        std::fs::remove_dir_all(&root).ok();
    }
}
