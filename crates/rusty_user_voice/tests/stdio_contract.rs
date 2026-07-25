use std::collections::BTreeSet;

use rmcp::{ServiceExt, model::CallToolRequestParams};

mod support;

#[tokio::test]
#[ignore = "requires built binary"]
async fn stdio_lists_exact_surface_and_calls_workspace() {
    let mock_server = wiremock::MockServer::start().await;
    support::mount_workspace_routes(&mock_server).await;
    let state_dir = tempfile::tempdir().expect("state directory");

    let mut command = tokio::process::Command::new(assert_cmd::cargo::cargo_bin!("uservoice-mcp"));
    command
        .env("USERVOICE_BASE_URL", mock_server.uri())
        .env("USERVOICE_API_KEY", "fixture-token")
        .env("USERVOICE_STATE_DIR", state_dir.path())
        .kill_on_drop(true);

    let transport = rmcp::transport::TokioChildProcess::new(command).expect("child transport");
    let client = ().serve(transport).await.expect("MCP initialize");
    let tools = client.list_all_tools().await.expect("list tools");
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<BTreeSet<_>>(),
        rusty_user_voice::ToolName::ALL
            .iter()
            .copied()
            .map(rusty_user_voice::ToolName::as_str)
            .collect()
    );

    let result = client
        .call_tool(
            CallToolRequestParams::new("uservoice_workspace_context").with_arguments(
                serde_json::json!({"include_catalog": true})
                    .as_object()
                    .expect("object")
                    .clone(),
            ),
        )
        .await
        .expect("workspace call");
    assert_ne!(result.is_error, Some(true));
    let structured = result.structured_content.expect("structuredContent");
    assert_eq!(
        structured.get("status").and_then(serde_json::Value::as_str),
        Some("ok")
    );
    assert!(structured.pointer("/data/current_user/id").is_some());
    assert!(structured.pointer("/data/catalog/forums/0/id").is_some());

    client.cancel().await.expect("cancel");
}
