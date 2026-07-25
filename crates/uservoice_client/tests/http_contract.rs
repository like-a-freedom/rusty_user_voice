use chrono::{TimeZone, Utc};
use uservoice_client::{
    AccessToken, ApiEndpoint, ForumId, IdeaId, ReqwestUserVoiceClient, SearchSuggestionsRequest,
    UserVoiceApi,
};
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path, query_param, query_param_is_missing},
};

#[tokio::test]
async fn get_retries_one_transient_503_but_write_does_not_retry() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    let server = MockServer::start().await;
    let get_attempts = Arc::new(AtomicUsize::new(0));
    let responder_attempts = Arc::clone(&get_attempts);
    Mock::given(method("GET"))
        .and(path("/api/v2/admin/users/current"))
        .respond_with(move |_: &wiremock::Request| {
            if responder_attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                ResponseTemplate::new(503)
            } else {
                ResponseTemplate::new(200).set_body_raw(
                    include_str!("fixtures/current_user.json"),
                    "application/json",
                )
            }
        })
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/v2/admin/notes"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;

    let client = fixture_client(&server);
    client
        .current_user()
        .await
        .expect("eventual 200 after one 503");
    assert_eq!(get_attempts.load(Ordering::SeqCst), 2);
    assert!(
        client
            .create_note(IdeaId::new(101), "one write only".to_owned())
            .await
            .is_err()
    );
}

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

#[tokio::test]
async fn context_collection_follows_next_link_until_every_record_is_loaded() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v2/admin/comments"))
        .and(query_param("suggestion", "101"))
        .and(query_param("per_page", "100"))
        .and(query_param_is_missing("cursor"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header(
                    "Link",
                    format!(
                        "<{}/api/v2/admin/comments?suggestion=101&per_page=100&cursor=next-2>; rel=\"next\"",
                        server.uri()
                    ),
                )
                .set_body_json(serde_json::json!({
                    "comments": [{"id": 201, "created_at": null, "updated_at": null}],
                    "pagination": {"per_page": 100}
                })),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/v2/admin/comments"))
        .and(query_param("cursor", "next-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "comments": [{"id": 202, "created_at": null, "updated_at": null}],
            "pagination": {"per_page": 100}
        })))
        .expect(1)
        .mount(&server)
        .await;

    let page = fixture_client(&server)
        .comments(IdeaId::new(101), 100)
        .await
        .expect("all comments")
        .data;
    assert_eq!(
        page.items
            .iter()
            .map(|item| item.id.get())
            .collect::<Vec<_>>(),
        [201, 202]
    );
    assert!(page.pagination.next.is_none());
}

#[derive(Clone, Default)]
struct SharedBuffer(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

struct BufferWriter(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

impl std::io::Write for BufferWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("buffer lock").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedBuffer {
    type Writer = BufferWriter;

    fn make_writer(&'a self) -> Self::Writer {
        BufferWriter(std::sync::Arc::clone(&self.0))
    }
}

#[tokio::test(flavor = "current_thread")]
async fn tracing_never_contains_upstream_bodies_or_token() {
    const TOKEN: &str = "fixture-secret-do-not-log";
    const EMAIL: &str = "private-person@example.invalid";
    const BODY: &str = "private feedback body";
    let server = wiremock::MockServer::start().await;
    // 503 is chosen deliberately: the GET retry loop in `client.rs::get`
    // emits a `tracing::warn!` for every retry attempt, so this exercise
    // proves instrumentation actually runs through the buffer. The body
    // carries fixture secrets that must never appear in any traced event.
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/api/v2/admin/users/current"))
        .respond_with(
            wiremock::ResponseTemplate::new(503)
                .set_body_json(serde_json::json!({"errors": {"email": EMAIL, "body": BODY}})),
        )
        .expect(1..)
        .mount(&server)
        .await;

    let buffer = SharedBuffer::default();
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(buffer.clone())
        .finish();
    let dispatch = tracing::Dispatch::new(subscriber);
    let _guard = tracing::dispatcher::set_default(&dispatch);
    let client = uservoice_client::ReqwestUserVoiceClient::new(
        uservoice_client::ApiEndpoint::parse(&server.uri()).expect("endpoint"),
        uservoice_client::AccessToken::parse(TOKEN.to_owned()).expect("token"),
    )
    .expect("client");
    let _ = client.current_user().await.expect_err("fixture 503");

    let captured = buffer.0.lock().expect("buffer lock").clone();
    assert!(
        !captured.is_empty(),
        "the GET retry path must emit at least one tracing event, otherwise \
         the no-leak assertion below would be vacuously true"
    );
    let rendered = String::from_utf8(captured).expect("UTF-8 logs");
    for forbidden in [TOKEN, EMAIL, BODY] {
        assert!(!rendered.contains(forbidden), "tracing leaked fixture data");
    }
}
