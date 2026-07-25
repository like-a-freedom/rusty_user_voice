#[tokio::test]
#[ignore = "requires live UserVoice API credentials in .env.test"]
async fn live_auth_probe_succeeds() {
    let settings = rusty_user_voice::Settings::from_sources(None, None, None)
        .expect("settings from .env.test");
    let app = rusty_user_voice::bootstrap::build_app(&settings).expect("build app");
    let result = app
        .workspace_context(rusty_user_voice::WorkspaceContextParams {
            include_catalog: true,
        })
        .await
        .expect("workspace context");
    assert_eq!(result.status, rusty_user_voice::OutcomeStatus::Ok);
    assert!(result.data.current_user.id.get() > 0);
}
