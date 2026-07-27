use rusty_user_voice::Settings;

#[test]
fn settings_from_values_works() {
    let s = Settings::from_values("https://acme.uservoice.com", "test-token", None, None)
        .expect("settings");
    assert_eq!(s.max_in_flight, 2);
}

#[test]
fn settings_from_values_clamps_max_in_flight() {
    let s = Settings::from_values("https://acme.uservoice.com", "tok", None, Some(100))
        .expect("settings");
    assert_eq!(s.max_in_flight, 8);

    let s = Settings::from_values("https://acme.uservoice.com", "tok", None, Some(0))
        .expect("settings");
    assert_eq!(s.max_in_flight, 1);
}

#[test]
fn settings_debug_output_redacts_api_key() {
    let s = Settings::from_values(
        "https://acme.uservoice.com",
        "fixture-super-secret",
        None,
        None,
    )
    .expect("settings");
    let rendered = format!("{s:?}");
    assert!(!rendered.contains("fixture-super-secret"));
    assert!(rendered.contains("[REDACTED]"));
}

#[test]
fn settings_default_output_and_log_level_are_none() {
    let s =
        Settings::from_values("https://acme.uservoice.com", "tok", None, None).expect("settings");
    assert!(s.default_output.is_none());
    assert!(s.log_level.is_none());
    assert!(s.ca_cert.is_none());
}
