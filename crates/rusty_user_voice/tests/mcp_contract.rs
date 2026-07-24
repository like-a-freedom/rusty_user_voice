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

#[tokio::test]
async fn mcp_tool_calls_return_structured_output() {
    use rmcp::{ServiceExt, model::CallToolRequestParams};

    let server =
        UserVoiceMcpServer::new(App::new(Arc::new(support::FakeUserVoiceApi::complete()), 2));
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
    ] {
        let result = client
            .call_tool(
                CallToolRequestParams::new(name)
                    .with_arguments(arguments.as_object().expect("object").clone()),
            )
            .await
            .expect("tool");
        assert_ne!(result.is_error, Some(true));
        let output = result.structured_content.expect("structuredContent");
        let tool = tools
            .iter()
            .find(|tool| tool.name == name)
            .expect("definition");
        assert_matches_output_schema(tool, output);
    }

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
