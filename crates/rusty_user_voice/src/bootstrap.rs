use std::sync::Arc;

use uservoice_client::ReqwestUserVoiceClient;

use crate::{App, Settings, error::AppError, ledger::DiskChangeLedger};

pub fn build_app(settings: &Settings) -> Result<App<ReqwestUserVoiceClient>, AppError> {
    let mut http_builder = reqwest::Client::builder()
        .user_agent(concat!("rusty-user-voice/", env!("CARGO_PKG_VERSION")));
    if let Some(ref cert_path) = settings.ca_cert {
        let pem = std::fs::read(cert_path)
            .map_err(|e| AppError::Validation(format!("failed to read ca_cert: {e}")))?;
        let cert = reqwest::Certificate::from_pem(&pem)
            .map_err(|e| AppError::Validation(format!("invalid ca_cert PEM: {e}")))?;
        http_builder = http_builder.add_root_certificate(cert);
    }
    let http = http_builder
        .build()
        .map_err(|e| AppError::Validation(e.to_string()))?;
    let client = ReqwestUserVoiceClient::from_http_client(
        http,
        settings.endpoint.clone(),
        settings.token.clone(),
    )?;
    Ok(
        App::new(Arc::new(client), settings.max_in_flight).with_ledger(Arc::new(
            DiskChangeLedger::new(settings.state_dir.join("ledger")),
        )),
    )
}
