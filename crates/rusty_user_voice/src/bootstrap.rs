use std::sync::Arc;

use uservoice_client::ReqwestUserVoiceClient;

use crate::{App, Settings, error::AppError, ledger::DiskChangeLedger};

pub fn build_app(settings: &Settings) -> Result<App<ReqwestUserVoiceClient>, AppError> {
    let client = ReqwestUserVoiceClient::new(settings.endpoint.clone(), settings.token.clone())?;
    Ok(
        App::new(Arc::new(client), settings.max_in_flight).with_ledger(Arc::new(
            DiskChangeLedger::new(settings.state_dir.join("ledger")),
        )),
    )
}
