mod support;

use std::{collections::BTreeSet, sync::Arc};

use rusty_user_voice::{App, ToolName, UserVoiceMcpServer};

#[test]
fn public_tool_names_are_exactly_the_approved_seven() {
    let names = ToolName::ALL
        .iter()
        .map(|tool| tool.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        names,
        BTreeSet::from([
            "uservoice_collect_ideas",
            "uservoice_get_idea_context",
            "uservoice_publish_idea_update",
            "uservoice_record_feedback",
            "uservoice_search_ideas",
            "uservoice_update_idea",
            "uservoice_workspace_context",
        ])
    );
}

#[test]
fn registered_tools_match_approved_seven() {
    let app = App::new(Arc::new(support::FakeUserVoiceApi::complete()), 2);
    let tools = UserVoiceMcpServer::new(app).tool_definitions();
    assert_eq!(tools.len(), 7);
    let registered: BTreeSet<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    let expected: BTreeSet<&str> = ToolName::ALL.iter().map(|t| t.as_str()).collect();
    assert_eq!(registered, expected);
}

#[test]
fn read_tools_have_closed_input_schemas_and_explicit_risk_hints() {
    let app = App::new(Arc::new(support::FakeUserVoiceApi::complete()), 2);
    let tools = UserVoiceMcpServer::new(app).tool_definitions();
    for name in [
        "uservoice_workspace_context",
        "uservoice_search_ideas",
        "uservoice_get_idea_context",
        "uservoice_collect_ideas",
    ] {
        let tool = tools.iter().find(|tool| tool.name == name).expect("tool");
        assert_eq!(
            tool.input_schema.get("additionalProperties"),
            Some(&serde_json::Value::Bool(false))
        );
        let annotations = tool.annotations.as_ref().expect("annotations");
        assert_eq!(annotations.read_only_hint, Some(true));
        assert_eq!(annotations.idempotent_hint, Some(true));
        assert_eq!(annotations.open_world_hint, Some(true));
        assert!(tool.output_schema.is_some());
    }
}

#[test]
fn mutation_tools_have_correct_risk_annotations() {
    let app = App::new(Arc::new(support::FakeUserVoiceApi::complete()), 2);
    let tools = UserVoiceMcpServer::new(app).tool_definitions();

    let update = tools
        .iter()
        .find(|tool| tool.name == "uservoice_update_idea")
        .expect("update tool");
    let annotations = update.annotations.as_ref().expect("annotations");
    assert_eq!(annotations.read_only_hint, Some(false));
    assert_eq!(annotations.destructive_hint, Some(true));
    assert_eq!(annotations.idempotent_hint, Some(false));
    assert_eq!(annotations.open_world_hint, Some(true));

    let feedback = tools
        .iter()
        .find(|tool| tool.name == "uservoice_record_feedback")
        .expect("feedback tool");
    let annotations = feedback.annotations.as_ref().expect("annotations");
    assert_eq!(annotations.read_only_hint, Some(false));
    assert_eq!(annotations.destructive_hint, Some(false));
    assert_eq!(annotations.idempotent_hint, Some(true));
    assert_eq!(annotations.open_world_hint, Some(true));
}

#[test]
fn cli_intent_names_match_tool_names() {
    let names: BTreeSet<&str> = rusty_user_voice::cli::intent_names().into_iter().collect();
    let expected: BTreeSet<&str> = ToolName::ALL.iter().map(|t| t.as_str()).collect();
    assert_eq!(names, expected);
}

#[tokio::test]
async fn mcp_tool_calls_return_structured_output() {
    use rmcp::{ServiceExt, model::CallToolRequestParams};

    let directory = tempfile::tempdir().expect("state directory");
    let api = support::FakeUserVoiceApi::complete();
    let app = App::new(Arc::new(api.clone()), 2)
        .with_ledger(Arc::new(rusty_user_voice::DiskChangeLedger::new(
            directory.path().join("ledger"),
        )))
        .with_clock(Arc::new(rusty_user_voice::FixedClock::new(
            chrono::Utc::now(),
        )));
    let server = UserVoiceMcpServer::new(app);
    let (server_transport, client_transport) = tokio::io::duplex(1 << 20);
    let server_task = tokio::spawn(async move {
        server
            .serve(server_transport)
            .await
            .expect("serve")
            .waiting()
            .await
            .expect("server");
    });
    let client = ().serve(client_transport).await.expect("client");
    let tools = client.list_all_tools().await.expect("tools");

    for (name, arguments) in [
        ("uservoice_workspace_context", serde_json::json!({})),
        (
            "uservoice_search_ideas",
            serde_json::json!({"query": "export", "per_page": 100}),
        ),
        (
            "uservoice_get_idea_context",
            serde_json::json!({
                "idea_id": 101,
                "detail_level": "full",
                "include_supporters": true,
                "per_page": 100
            }),
        ),
        (
            "uservoice_collect_ideas",
            serde_json::json!({
                "updated_after": "2025-01-01T00:00:00Z",
                "detail_level": "summary",
                "per_page": 100
            }),
        ),
        (
            "uservoice_update_idea",
            serde_json::json!({
                "mode": "preview",
                "idea_id": 101,
                "title": "Clearer export title"
            }),
        ),
        (
            "uservoice_publish_idea_update",
            serde_json::json!({
                "mode": "preview",
                "idea_id": 101,
                "status_id": 13,
                "message": "Scheduled",
                "notify_supporters": false
            }),
        ),
        (
            "uservoice_record_feedback",
            serde_json::json!({
                "body": "Customer needs CSV export",
                "channel": "api",
                "idea_id": 101,
                "source_guid": "fixture-case-42",
                "idempotency_key": "fixture-feedback-42"
            }),
        ),
    ] {
        let result = client
            .call_tool(
                CallToolRequestParams::new(name)
                    .with_arguments(arguments.as_object().expect("object").clone()),
            )
            .await
            .expect("tool call");
        assert_ne!(result.is_error, Some(true), "{name} returned a tool error");
        let output = result.structured_content.expect("structuredContent");
        let tool = tools
            .iter()
            .find(|tool| tool.name == name)
            .expect("definition");
        assert_matches_output_schema(tool, output);
    }

    for tool in &tools {
        assert_eq!(
            tool.input_schema.get("additionalProperties"),
            Some(&serde_json::Value::Bool(false)),
            "{} accepts unknown root fields",
            tool.name
        );
    }

    let rejected = client
        .call_tool(
            CallToolRequestParams::new("uservoice_search_ideas").with_arguments(
                serde_json::json!({"query": "export", "unknown": true})
                    .as_object()
                    .expect("object")
                    .clone(),
            ),
        )
        .await
        .expect("RMCP returns argument decoding failures as a tool result");
    assert_eq!(rejected.is_error, Some(true));

    client.cancel().await.expect("cancel");
    server_task.await.expect("join");
}

fn assert_matches_output_schema(tool: &rmcp::model::Tool, output: serde_json::Value) {
    let schema = serde_json::Value::Object(
        tool.output_schema
            .as_ref()
            .expect("output schema")
            .as_ref()
            .clone(),
    );
    let validator = jsonschema::validator_for(&schema).expect("valid output schema");
    assert!(
        validator.is_valid(&output),
        "output must match MCP output schema"
    );
}

/// CLI-only commands with no MCP tool mirror. If a `ToolName` variant ever
/// appears for one of these, this fails loudly: local secret ops must stay out
/// of the MCP surface.
const CLI_ONLY: &[&str] = &["login", "logout"];

#[test]
fn cli_only_commands_have_no_mcp_tool() {
    let tool_names: BTreeSet<&str> = ToolName::ALL.iter().map(|t| t.as_str()).collect();
    for cli_cmd in CLI_ONLY {
        assert!(
            !tool_names.contains(*cli_cmd),
            "CLI-only '{cli_cmd}' must not gain an MCP tool (local secret op)"
        );
    }
}
