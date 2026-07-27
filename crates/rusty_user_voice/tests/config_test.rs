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

fn setup_home_config(yaml: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let config_dir = dir.path().join(".config").join("rusty-user-voice");
    std::fs::create_dir_all(&config_dir).expect("create config dir");
    std::fs::write(config_dir.join("config.yaml"), yaml).expect("write config");
    dir
}

fn with_home<F: FnOnce()>(dir: &std::path::Path, f: F) {
    let orig_home = std::env::var("HOME").ok();
    let orig_api_key = std::env::var("USERVOICE_API_KEY").ok();
    let orig_base_url = std::env::var("USERVOICE_BASE_URL").ok();
    // SAFETY: single-threaded test, all vars restored before return
    unsafe {
        std::env::set_var("HOME", dir);
        std::env::remove_var("USERVOICE_API_KEY");
        std::env::remove_var("USERVOICE_BASE_URL");
    }
    f();
    unsafe {
        match orig_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
        match orig_api_key {
            Some(v) => std::env::set_var("USERVOICE_API_KEY", v),
            None => std::env::remove_var("USERVOICE_API_KEY"),
        }
        match orig_base_url {
            Some(v) => std::env::set_var("USERVOICE_BASE_URL", v),
            None => std::env::remove_var("USERVOICE_BASE_URL"),
        }
    }
}

#[test]
fn from_sources_loads_config_yaml_from_home() {
    let dir = setup_home_config(
        "uservoice_api_key: yaml-token\nuservoice_base_url: https://yaml.uservoice.com\n",
    );
    let mut result = None;
    with_home(dir.path(), || {
        result = Some(Settings::from_sources(None, None, None).expect("settings"));
    });
    let s = result.unwrap();
    assert_eq!(
        s.endpoint.as_url().as_str(),
        "https://yaml.uservoice.com/api/v2/"
    );
}

#[test]
fn from_sources_env_var_overrides_config_yaml() {
    let dir = setup_home_config(
        "uservoice_api_key: yaml-token\nuservoice_base_url: https://yaml.uservoice.com\n",
    );
    with_home(dir.path(), || {
        unsafe { std::env::set_var("USERVOICE_API_KEY", "env-token") };
        let s = Settings::from_sources(None, None, None).expect("settings");
        assert!(format!("{:?}", s).contains("[REDACTED]"));
    });
}

#[test]
fn from_sources_missing_api_key_returns_error() {
    let dir = setup_home_config("uservoice_base_url: https://yaml.uservoice.com\n");
    with_home(dir.path(), || {
        let err = Settings::from_sources(None, None, None).expect_err("should fail");
        assert!(
            err.to_string().contains("USERVOICE_API_KEY"),
            "error: {err}"
        );
    });
}

#[test]
fn from_sources_missing_base_url_returns_error() {
    let dir = setup_home_config("uservoice_api_key: yaml-token\n");
    with_home(dir.path(), || {
        let err = Settings::from_sources(None, None, None).expect_err("should fail");
        assert!(
            err.to_string().contains("USERVOICE_BASE_URL"),
            "error: {err}"
        );
    });
}

#[test]
fn from_sources_default_output_from_config() {
    let dir = setup_home_config(
        "uservoice_api_key: tok\nuservoice_base_url: https://yaml.uservoice.com\ndefault_output: ndjson\n",
    );
    let mut result = None;
    with_home(dir.path(), || {
        result = Some(Settings::from_sources(None, None, None).expect("settings"));
    });
    assert_eq!(result.unwrap().default_output.as_deref(), Some("ndjson"));
}

#[test]
fn from_sources_log_level_from_config() {
    let dir = setup_home_config(
        "uservoice_api_key: tok\nuservoice_base_url: https://yaml.uservoice.com\nlog_level: debug\n",
    );
    let mut result = None;
    with_home(dir.path(), || {
        result = Some(Settings::from_sources(None, None, None).expect("settings"));
    });
    assert_eq!(result.unwrap().log_level.as_deref(), Some("debug"));
}

#[test]
fn from_sources_max_in_flight_from_config() {
    let dir = setup_home_config(
        "uservoice_api_key: tok\nuservoice_base_url: https://yaml.uservoice.com\nuservoice_max_in_flight: 5\n",
    );
    let mut result = None;
    with_home(dir.path(), || {
        result = Some(Settings::from_sources(None, None, None).expect("settings"));
    });
    assert_eq!(result.unwrap().max_in_flight, 5);
}

#[test]
fn from_sources_cli_flag_overrides_config_yaml() {
    let dir = setup_home_config(
        "uservoice_api_key: tok\nuservoice_base_url: https://yaml.uservoice.com\nuservoice_max_in_flight: 5\n",
    );
    let mut result = None;
    with_home(dir.path(), || {
        result = Some(
            Settings::from_sources(Some("https://cli.uservoice.com"), None, Some(3))
                .expect("settings"),
        );
    });
    let s = result.unwrap();
    assert_eq!(
        s.endpoint.as_url().as_str(),
        "https://cli.uservoice.com/api/v2/"
    );
    assert_eq!(s.max_in_flight, 3);
}
