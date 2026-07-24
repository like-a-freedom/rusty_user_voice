use url::Url;

use crate::ClientError;

/// Validated and normalized UserVoice API root.
///
/// Accepts either a tenant root (`https://<tenant>.uservoice.com`) or an
/// already-`/api/v2` form, and normalizes both to a single internal API root
/// ending in `/api/v2/`. HTTP is permitted only for loopback hosts so contract
/// tests against a local wiremock can run without TLS.
#[derive(Clone, Debug)]
pub struct ApiEndpoint(Url);

impl ApiEndpoint {
    /// Parses and validates a raw `USERVOICE_BASE_URL` value.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::InvalidBaseUrl`] when the value is not a valid
    /// URL, is not on `https://<tenant>.uservoice.com` (or loopback HTTP),
    /// or carries credentials/query/fragment.
    pub fn parse(raw: &str) -> Result<Self, ClientError> {
        let mut url = Url::parse(raw.trim())
            .map_err(|error| ClientError::InvalidBaseUrl(error.to_string()))?;
        let host = url
            .host_str()
            .ok_or_else(|| ClientError::InvalidBaseUrl("host is required".to_owned()))?;
        let loopback = matches!(host, "localhost" | "127.0.0.1" | "::1");
        let trusted_remote = url.scheme() == "https"
            && host.ends_with(".uservoice.com")
            && host.len() > ".uservoice.com".len();
        if !(trusted_remote || (loopback && url.scheme() == "http")) {
            return Err(ClientError::InvalidBaseUrl(
                "expected https://<tenant>.uservoice.com; http is allowed only for loopback tests"
                    .to_owned(),
            ));
        }
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(ClientError::InvalidBaseUrl(
                "credentials, query, and fragment are not allowed".to_owned(),
            ));
        }
        url.set_path("/api/v2/");
        Ok(Self(url))
    }

    pub fn as_url(&self) -> &Url {
        &self.0
    }

    /// Joins an Admin API path under `/api/v2/admin/`. Leading slashes in
    /// `path` are trimmed so both `"users/current"` and `"/users/current"`
    /// produce the same URL.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::InvalidBaseUrl`] when the join fails (which is
    /// limited to malformed path inputs).
    pub(crate) fn admin_url(&self, path: &str) -> Result<Url, ClientError> {
        self.0
            .join(&format!("admin/{}", path.trim_start_matches('/')))
            .map_err(|error| ClientError::InvalidBaseUrl(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_tenant_root_to_api_v2() {
        let endpoint =
            ApiEndpoint::parse("https://acme.uservoice.com").expect("valid UserVoice URL");
        assert_eq!(
            endpoint.as_url().as_str(),
            "https://acme.uservoice.com/api/v2/"
        );
    }

    #[test]
    fn rejects_non_uservoice_remote_host() {
        let error = ApiEndpoint::parse("https://example.com").expect_err("must reject host");
        assert!(matches!(error, ClientError::InvalidBaseUrl(_)));
    }

    #[test]
    fn permits_loopback_http_for_contract_tests() {
        let endpoint = ApiEndpoint::parse("http://127.0.0.1:43123").expect("loopback is test-safe");
        assert_eq!(endpoint.as_url().path(), "/api/v2/");
    }

    #[test]
    fn rejects_apex_uservoice_domain() {
        // `uservoice.com` without a tenant subdomain is rejected: there is no
        // tenant to address.
        let error = ApiEndpoint::parse("https://uservoice.com").expect_err("apex rejected");
        assert!(matches!(error, ClientError::InvalidBaseUrl(_)));
    }

    #[test]
    fn rejects_credentials_in_url() {
        let error = ApiEndpoint::parse("https://user:pass@acme.uservoice.com")
            .expect_err("credentials rejected");
        assert!(matches!(error, ClientError::InvalidBaseUrl(_)));
    }

    #[test]
    fn admin_url_joins_paths() {
        let endpoint =
            ApiEndpoint::parse("https://acme.uservoice.com").expect("valid UserVoice URL");
        let url = endpoint.admin_url("users/current").expect("join succeeds");
        assert_eq!(
            url.as_str(),
            "https://acme.uservoice.com/api/v2/admin/users/current"
        );
    }

    #[test]
    fn admin_url_trims_leading_slash() {
        let endpoint =
            ApiEndpoint::parse("https://acme.uservoice.com").expect("valid UserVoice URL");
        let url = endpoint
            .admin_url("/suggestions/42")
            .expect("join succeeds");
        assert_eq!(
            url.as_str(),
            "https://acme.uservoice.com/api/v2/admin/suggestions/42"
        );
    }
}
