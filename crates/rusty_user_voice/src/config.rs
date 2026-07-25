use std::{env, path::PathBuf};

use uservoice_client::{AccessToken, ApiEndpoint};

use crate::error::AppError;

#[derive(Clone)]
pub struct Settings {
    pub endpoint: ApiEndpoint,
    pub token: AccessToken,
    pub state_dir: PathBuf,
    pub max_in_flight: usize,
}

impl Settings {
    pub fn from_sources(
        base_url_override: Option<&str>,
        state_dir_override: Option<PathBuf>,
        max_in_flight_override: Option<u32>,
    ) -> Result<Self, AppError> {
        let base_url = base_url_override
            .map(str::to_owned)
            .or_else(|| env::var("USERVOICE_BASE_URL").ok())
            .ok_or_else(|| AppError::Validation("USERVOICE_BASE_URL is required".to_owned()))?;
        let api_key = env::var("USERVOICE_API_KEY")
            .map_err(|_| AppError::Validation("USERVOICE_API_KEY is required".to_owned()))?;
        Self::from_values(
            &base_url,
            &api_key,
            state_dir_override.or_else(|| env::var_os("USERVOICE_STATE_DIR").map(PathBuf::from)),
            max_in_flight_override.or_else(|| {
                env::var("USERVOICE_MAX_IN_FLIGHT")
                    .ok()
                    .and_then(|value| value.parse().ok())
            }),
        )
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
        })
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
            .finish()
    }
}
