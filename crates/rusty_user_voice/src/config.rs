use std::{env, io::Write, path::PathBuf};

use uservoice_client::{AccessToken, ApiEndpoint};

use crate::error::AppError;

const CONFIG_FILENAME: &str = "config.yaml";

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
            .or_else(|| dirs::state_dir().map(|path| path.join("rusty-user-voice")))
            .or_else(|| dirs::data_local_dir().map(|path| path.join("rusty-user-voice")))
            .ok_or_else(|| {
                AppError::Validation(
                    "USERVOICE_STATE_DIR is required when no platform state directory exists"
                        .to_owned(),
                )
            })?;

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
        let state_dir = state_dir
            .or_else(|| dirs::state_dir().map(|path| path.join("rusty-user-voice")))
            .or_else(|| dirs::data_local_dir().map(|path| path.join("rusty-user-voice")))
            .ok_or_else(|| {
                AppError::Validation(
                    "USERVOICE_STATE_DIR is required when no platform state directory exists"
                        .to_owned(),
                )
            })?;
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
        if let Some(path) = Self::config_path()
            && let Ok(content) = std::fs::read_to_string(&path)
            && let Ok(yaml) = serde_yaml::from_str(&content)
        {
            return Some(yaml);
        }
        Self::config_path_home().and_then(|path| {
            let content = std::fs::read_to_string(&path).ok()?;
            serde_yaml::from_str(&content).ok()
        })
    }

    fn config_path() -> Option<PathBuf> {
        let exe_dir = std::env::current_exe().ok()?;
        let dir = exe_dir.parent()?;
        Some(dir.join(CONFIG_FILENAME))
    }

    fn config_path_home() -> Option<PathBuf> {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .ok()?;
        Some(
            PathBuf::from(home)
                .join(".config")
                .join("rusty-user-voice")
                .join(CONFIG_FILENAME),
        )
    }
}

/// User-level config path: ~/.config/rusty-user-voice/config.yaml
/// (login always writes here — never next to the binary).
// Task 1 scaffolding: first used by Task 4; the allow is removed there.
#[allow(dead_code)]
pub(crate) fn user_config_path() -> Option<std::path::PathBuf> {
    Settings::config_path_home()
}

/// Binary-dir config path (next to the executable). Exposed read-only so
/// login/logout can report that a deployed config shadows the user file.
// Task 1 scaffolding: first used by Task 2 (`shadow_note`); the allow is removed in Task 4.
#[allow(dead_code)]
pub(crate) fn binary_config_path() -> Option<std::path::PathBuf> {
    Settings::config_path()
}

/// Raw user-file read for the `--force` merge (decision 13). `None` when the
/// file is absent or unparseable — login then writes a fresh config.
// Task 1 scaffolding: first used by Task 4; the allow is removed there.
#[allow(dead_code)]
pub(crate) fn load_user_config_raw() -> Option<YamlConfig> {
    let path = Settings::config_path_home()?;
    let content = std::fs::read_to_string(&path).ok()?;
    serde_yaml::from_str(&content).ok()
}

/// Write `cfg` as YAML to an explicit `path`, creating parent dirs. The file
/// is created 0600 on Unix (the bearer key is a secret) and perms are
/// re-asserted after the write so a pre-existing 0644 file is tightened too.
// Task 1 scaffolding: used by unit tests now and by `save_user_config`; the
// allow covers the non-test build and is removed in Task 4.
#[allow(dead_code)]
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
// Task 1 scaffolding: first used by Task 4; the allow is removed there.
#[allow(dead_code)]
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
}
