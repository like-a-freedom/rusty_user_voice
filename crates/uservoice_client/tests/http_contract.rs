use chrono::{TimeZone, Utc};
use uservoice_client::{
    AccessToken, ApiEndpoint, ForumId, IdeaId, ReqwestUserVoiceClient, SearchSuggestionsRequest,
    UserVoiceApi,
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path, query_param},
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

#[tokio::test]
async fn search_uses_documented_filter_names_and_preserves_unknown_fields() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/admin/suggestions"))
        .and(query_param("q", "roadmap export"))
        .and(query_param("forum", "11"))
        .and(query_param("updated_after", "2025-01-01T00:00:00+00:00"))
        .and(query_param("per_page", "50"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            include_str!("fixtures/suggestions_page.json"),
            "application/json",
        ))
        .mount(&server)
        .await;

    let client = fixture_client(&server);
    let request = SearchSuggestionsRequest {
        query: Some("roadmap export".to_owned()),
        updated_after: Some(Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
        updated_before: None,
        forum_ids: vec![ForumId::new(11)],
        category_ids: Vec::new(),
        status_ids: Vec::new(),
        label_ids: Vec::new(),
        states: Vec::new(),
        sort: Some("-updated_at".to_owned()),
        per_page: 50,
        continuation: None,
    };

    let page = client.search_suggestions(request).await.expect("search");
    assert_eq!(page.data.items[0].id.get(), 101);
    assert_eq!(
        page.data.items[0].extra["fixture_unknown_field"],
        "must survive round-trip"
    );
    assert_eq!(
        page.data.pagination.next,
        Some(uservoice_client::Continuation::Cursor {
            cursor: "cursor-2".to_owned(),
        })
    );
}

#[tokio::test]
async fn single_idea_preserves_sideloaded_associations() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/admin/suggestions/101"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            include_str!("fixtures/suggestions_page.json"),
            "application/json",
        ))
        .expect(1)
        .mount(&server)
        .await;

    let idea = fixture_client(&server)
        .suggestion(IdeaId::new(101))
        .await
        .expect("idea")
        .data;
    assert_eq!(idea.included["attachments"][0]["id"], 901);
    assert_eq!(
        idea.extra["fixture_unknown_field"],
        "must survive round-trip"
    );
}

fn fixture_client(server: &MockServer) -> ReqwestUserVoiceClient {
    ReqwestUserVoiceClient::new(
        ApiEndpoint::parse(&server.uri()).expect("loopback endpoint"),
        AccessToken::parse("fixture-token".to_owned()).expect("fixture token"),
    )
    .expect("client")
}

#[tokio::test]
async fn workspace_catalog_drains_all_taxonomy_collections() {
    let server = MockServer::start().await;
    let fixture = include_str!("fixtures/workspace.json");
    for endpoint in &["forums", "categories", "statuses", "labels"] {
        Mock::given(method("GET"))
            .and(path(format!("/api/v2/admin/{endpoint}")))
            .respond_with(ResponseTemplate::new(200).set_body_raw(fixture, "application/json"))
            .mount(&server)
            .await;
    }

    let catalog = fixture_client(&server)
        .workspace_catalog()
        .await
        .expect("catalog")
        .data;
    assert_eq!(catalog.forums[0].id.get(), 11);
    assert_eq!(catalog.categories[0].id.get(), 12);
    assert_eq!(catalog.statuses[0].id.get(), 13);
    assert_eq!(catalog.labels[0].id.get(), 14);
}
