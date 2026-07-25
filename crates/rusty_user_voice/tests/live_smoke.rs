use uservoice_client::UserVoiceApi;

#[tokio::test]
#[ignore = "requires live UserVoice API credentials in .env.test"]
async fn live_auth_probe_succeeds() {
    let settings = rusty_user_voice::Settings::from_sources(None, None, None)
        .expect("settings from .env.test");
    let app = rusty_user_voice::bootstrap::build_app(&settings).expect("build app");
    let response = app
        .api_for_smoke()
        .current_user()
        .await
        .expect("current user");
    assert!(response.data.id.get() > 0);
}
