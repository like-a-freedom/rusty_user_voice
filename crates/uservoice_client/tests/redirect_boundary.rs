//! Redirect boundary tests. The validated endpoint host (`ApiEndpoint`) is
//! the declared network boundary (docs/security.md): every outbound call must
//! stay on it. A redirect must therefore surface as an error, never be
//! followed to another host — following it would re-target the request (and
//! for 307/308, the POST body) at an arbitrary host.

use uservoice_client::{
    AccessToken, ApiEndpoint, ClientError, CreateFeedbackRecordLinks, CreateFeedbackRecordRequest,
    FeedbackChannel, IdeaId, ReqwestUserVoiceClient, UserVoiceApi,
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{any, method, path},
};

#[tokio::test]
async fn get_surfaces_redirect_as_error_without_hitting_the_redirect_target() {
    let origin = MockServer::start().await;
    let sink = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/admin/users/current"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("location", format!("{}/stolen", sink.uri())),
        )
        .expect(1)
        .mount(&origin)
        .await;
    // Any request reaching the sink crosses the validated-host boundary.
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            include_str!("fixtures/current_user.json"),
            "application/json",
        ))
        .expect(0)
        .mount(&sink)
        .await;

    let client = ReqwestUserVoiceClient::new(
        ApiEndpoint::parse(&origin.uri()).expect("loopback endpoint"),
        AccessToken::parse("uv-secret-token".to_owned()).expect("fixture token"),
    )
    .expect("client");

    let error = client
        .current_user()
        .await
        .expect_err("redirect must not be followed to a valid response");
    assert!(
        matches!(error, ClientError::HttpStatus { status: 302 }),
        "expected a surfaced 302, got {error:?}"
    );
}

#[tokio::test]
async fn post_body_is_not_resent_when_server_responds_with_307() {
    let origin = MockServer::start().await;
    let sink = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v2/admin/feedback_records"))
        .respond_with(
            ResponseTemplate::new(307).insert_header("location", format!("{}/collect", sink.uri())),
        )
        .expect(1)
        .mount(&origin)
        .await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(0)
        .mount(&sink)
        .await;

    let client = ReqwestUserVoiceClient::new(
        ApiEndpoint::parse(&origin.uri()).expect("loopback endpoint"),
        AccessToken::parse("uv-secret-token".to_owned()).expect("fixture token"),
    )
    .expect("client");

    let error = client
        .create_feedback_record(CreateFeedbackRecordRequest {
            body: "secret customer feedback".to_owned(),
            body_mime_type: "text/plain",
            channel: FeedbackChannel::Api,
            created_at: None,
            source_url: None,
            source_guid: None,
            links: CreateFeedbackRecordLinks {
                suggestion: Some(IdeaId::new(101)),
                user: None,
                user_email: None,
            },
        })
        .await
        .expect_err("307 must not resend the body to the redirect target");
    assert!(
        matches!(error, ClientError::HttpStatus { status: 307 }),
        "expected a surfaced 307, got {error:?}"
    );
}
