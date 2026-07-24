use uservoice_client::{AccessToken, ApiEndpoint, ReqwestUserVoiceClient, UserVoiceApi};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

#[tokio::test]
async fn current_user_sends_bearer_auth_and_parses_rate_limit() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/admin/users/current"))
        .and(header("authorization", "Bearer fixture-token"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("X-Rate-Limit-Limit", "60")
                .insert_header("X-Rate-Limit-Remaining", "59")
                .insert_header("X-Rate-Limit-Reset", "1730000000")
                .set_body_raw(
                    include_str!("fixtures/current_user.json"),
                    "application/json",
                ),
        )
        .mount(&server)
        .await;

    let client = ReqwestUserVoiceClient::new(
        ApiEndpoint::parse(&server.uri()).expect("loopback endpoint"),
        AccessToken::parse("fixture-token".to_owned()).expect("fixture token"),
    )
    .expect("client");

    let response = client.current_user().await.expect("current user");
    assert_eq!(response.data.id.get(), 7);
    assert_eq!(response.rate_limit.expect("rate limit").remaining, 59);
}
