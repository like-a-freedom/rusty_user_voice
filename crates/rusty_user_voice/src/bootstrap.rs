use std::sync::Arc;

use uservoice_client::ReqwestUserVoiceClient;

use crate::{App, Settings, error::AppError, ledger::DiskChangeLedger};

/// Builds the HTTP client shared by `build_app` and by `uservoice-cli login`
/// validation.
///
/// Single owner of the user-agent and the optional corporate root
/// certificate, so login validates against exactly the TLS setup the running
/// app will use (DRY, decision 15).
pub fn build_http_client(ca_cert: Option<&str>) -> Result<reqwest::Client, AppError> {
    let mut builder = reqwest::Client::builder()
        .user_agent(concat!("rusty-user-voice/", env!("CARGO_PKG_VERSION")));
    if let Some(cert_path) = ca_cert {
        let pem = std::fs::read(cert_path)
            .map_err(|e| AppError::Validation(format!("failed to read ca_cert: {e}")))?;
        let cert = reqwest::Certificate::from_pem(&pem)
            .map_err(|e| AppError::Validation(format!("invalid ca_cert PEM: {e}")))?;
        builder = builder.add_root_certificate(cert);
    }
    builder.build().map_err(|e| {
        // A custom root certificate can break the builder (reqwest validates
        // the TLS backend when roots are added), so name it when present.
        AppError::Validation(if ca_cert.is_some() {
            format!("failed to build HTTP client with ca_cert: {e}")
        } else {
            e.to_string()
        })
    })
}

pub fn build_app(settings: &Settings) -> Result<App<ReqwestUserVoiceClient>, AppError> {
    let http = build_http_client(settings.ca_cert.as_deref())?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_client_without_ca_cert() {
        assert!(build_http_client(None).is_ok());
    }

    #[test]
    fn unreadable_ca_cert_is_a_validation_error() {
        let error = build_http_client(Some("/nonexistent/uv-ca.pem")).expect_err("must fail");
        assert!(error.to_string().contains("ca_cert"), "{error}");
    }

    #[test]
    fn invalid_pem_ca_cert_is_a_validation_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("ca.pem");
        std::fs::write(
            &path,
            "-----BEGIN CERTIFICATE-----\nnot-valid-base64!!!\n-----END CERTIFICATE-----\n",
        )
        .expect("write");
        let error =
            build_http_client(Some(path.to_str().expect("utf-8 path"))).expect_err("must fail");
        assert!(error.to_string().contains("ca_cert"), "{error}");
    }
}
