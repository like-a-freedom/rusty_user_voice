use std::{env, path::PathBuf};

use uservoice_client::{AccessToken, ApiEndpoint};

use crate::error::AppError;

const CONFIG_FILENAME: &str = "config.yaml";

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct YamlConfig {
    uservoice_api_key: Option<String>,
    uservoice_base_url: Option<String>,
    uservoice_state_dir: Option<String>,
    uservoice_max_in_flight: Option<u32>,
    default_output: Option<String>,
    ca_cert: Option<String>,
    log_level: Option<String>,
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
