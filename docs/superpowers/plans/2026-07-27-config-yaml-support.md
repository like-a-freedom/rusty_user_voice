# Config YAML Support for uservoice-cli

**Status:** ready to implement.
**Scope:** single commit — add `config.yaml` loading following the `rusty_azure_devops` reference pattern.
**Pattern:** YAML config with priority chain CLI flags > env vars > config.yaml > defaults.

## Decisions

- **YAML** format (not TOML) — matches reference project, readable for PMs, `serde_yaml` dependency.
- **Config locations**: (1) next to binary, (2) `~/.config/rusty-user-voice/config.yaml`. No `--config` flag, no `USERVOICE_CONFIG` env var.
- **7 fields**: `uservoice_api_key`, `uservoice_base_url`, `uservoice_state_dir`, `uservoice_max_in_flight`, `default_output`, `ca_cert`, `log_level`.
- **`ca_cert`**: path to PEM/DER file, loaded via `reqwest::ClientBuilder::add_root_certificate()`. No `SSL_CERT_FILE` env var pollution.
- **`log_level`**: fallback for `RUST_LOG`. Priority: `RUST_LOG` > `USERVOICE_LOG_LEVEL` > config `log_level` > default (`warn`).
- **`default_output`**: `USERVOICE_DEFAULT_OUTPUT` env var overrides config. CLI `--output` has highest priority.
- **`api_key` security**: NOT in CLI flags. Priority: env var > config.yaml. Config must be in `.gitignore`.
- **`config.example.yaml`**: tracked in git with empty values. Users copy and fill.
- **Single commit**: all changes are one logical feature.

## Files

### Create: `config.example.yaml`
```yaml
# uservoice-cli configuration
# Copy this to config.yaml next to the binary or to ~/.config/rusty-user-voice/config.yaml
# Priority: CLI flags > environment variables > config.yaml > defaults

# Required: UserVoice API bearer token
# Never commit config.yaml with a real token
uservoice_api_key: ""

# Required: UserVoice tenant base URL
# Example: https://your-tenant.uservoice.com
uservoice_base_url: ""

# Optional: Local state directory for checkpoints and ledger
# Default: platform-specific app data directory
# uservoice_state_dir: ""

# Optional: Max concurrent read requests (1-8)
# uservoice_max_in_flight: 2

# Optional: Default output mode when --output is not specified
# Possible values: human, json, ndjson
# default_output: human

# Optional: Path to CA certificate (PEM or DER) for on-prem UserVoice
# ca_cert: "/path/to/corporate-root-ca.pem"

# Optional: Log level (RUST_LOG overrides this)
# Possible values: error, warn, info, debug, trace
# log_level: warn
```

### Modify: `.gitignore`
Add `config.yaml` to the gitignore list (alongside `.env.test`).

### Modify: `crates/rusty_user_voice/Cargo.toml`
Add `serde_yaml` dependency.

### Modify: `crates/rusty_user_voice/src/config.rs`
Rewrite `Settings` to load from config.yaml with the priority chain:

```rust
const CONFIG_FILENAME: &str = "config.yaml";

#[derive(Debug, Clone, serde::Deserialize, Default)]
struct YamlConfig {
    uservoice_api_key: Option<String>,
    uservoice_base_url: Option<String>,
    uservoice_state_dir: Option<String>,
    uservoice_max_in_flight: Option<u32>,
    default_output: Option<String>,
    ca_cert: Option<String>,
    log_level: Option<String>,
}

impl Settings {
    pub fn from_sources(
        base_url_override: Option<&str>,
        state_dir_override: Option<PathBuf>,
        max_in_flight_override: Option<u32>,
    ) -> Result<Self, AppError> {
        let yaml = Self::load_yaml().unwrap_or_default();

        // api_key: env var > config.yaml (NOT CLI flags — security rule)
        let api_key = env::var("USERVOICE_API_KEY")
            .or_else(|_| yaml.uservoice_api_key.clone().ok_or(()))
            .map_err(|_| AppError::Validation(
                "USERVOICE_API_KEY is required: set via env var or config.yaml".to_owned()
            ))?;

        // base_url: CLI flag > env var > config.yaml
        let base_url = base_url_override
            .map(str::to_owned)
            .or_else(|| env::var("USERVOICE_BASE_URL").ok())
            .or_else(|| yaml.uservoice_base_url)
            .ok_or_else(|| AppError::Validation(
                "USERVOICE_BASE_URL is required: set via --base-url, env var, or config.yaml".to_owned()
            ))?;

        // state_dir: CLI flag > env var > config.yaml > platform default
        let state_dir = state_dir_override
            .or_else(|| env::var_os("USERVOICE_STATE_DIR").map(PathBuf::from))
            .or_else(|| yaml.uservoice_state_dir.map(PathBuf::from))
            .or_else(|| dirs::state_dir().map(|path| path.join("rusty-user-voice")))
            .or_else(|| dirs::data_local_dir().map(|path| path.join("rusty-user-voice")));

        // max_in_flight: CLI flag > env var > config.yaml > default(2)
        let max_in_flight = max_in_flight_override
            .or_else(|| env::var("USERVOICE_MAX_IN_FLIGHT").ok().and_then(|v| v.parse().ok()))
            .or(yaml.uservoice_max_in_flight)
            .unwrap_or(2)
            .clamp(1, 8);

        // default_output: env var > config.yaml > default(human)
        let default_output = env::var("USERVOICE_DEFAULT_OUTPUT").ok()
            .or(yaml.default_output);

        // log_level: env var > config.yaml (RUST_LOG handled by tracing separately)
        let log_level = env::var("USERVOICE_LOG_LEVEL").ok()
            .or(yaml.log_level);

        // ca_cert: config.yaml only
        let ca_cert = yaml.ca_cert;

        Ok(Self {
            endpoint: ApiEndpoint::parse(&base_url)?,
            token: AccessToken::parse(api_key)?,
            state_dir: state_dir.ok_or_else(|| ...)?,
            max_in_flight: max_in_flight as usize,
            default_output,
            log_level,
            ca_cert,
        })
    }
}
```

### Modify: `crates/rusty_user_voice/src/config.rs` — config path resolution
```rust
fn load_yaml() -> Option<YamlConfig> {
    // Try binary-dir config first, then ~/.config/rusty-user-voice/config.yaml
    if let Some(path) = Self::config_path()
        && let Ok(content) = std::fs::read_to_string(&path)
        && let Ok(yaml) = serde_yaml::from_str(&content)
    {
        return Some(yaml);
    }
    Self::config_path_home().and_then(|path| {
        let content = std::fs::read_to_string(&path).ok()?;
        serde_yaml::from_str(&content).ok()
    })
}

fn config_path() -> Option<PathBuf> {
    let exe_dir = std::env::current_exe().ok()?;
    let dir = exe_dir.parent()?;
    Some(dir.join(CONFIG_FILENAME))
}

fn config_path_home() -> Option<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    Some(PathBuf::from(home)
        .join(".config")
        .join("rusty-user-voice")
        .join(CONFIG_FILENAME))
}
```

### Modify: `crates/rusty_user_voice/src/config.rs` — ca_cert loading
```rust
pub fn load_ca_certificates(&self) -> Result<Option<reqwest::Certificate>, AppError> {
    if let Some(ref cert_path) = self.ca_cert {
        let pem = std::fs::read(cert_path)
            .map_err(|e| AppError::Validation(format!("failed to read ca_cert: {e}")))?;
        let cert = reqwest::Certificate::from_pem(&pem)
            .map_err(|e| AppError::Validation(format!("invalid ca_cert PEM: {e}")))?;
        Ok(Some(cert))
    } else {
        Ok(None)
    }
}
```

### Modify: `crates/rusty_user_voice/src/bootstrap.rs`
After creating `Settings`, load CA cert and pass to reqwest client builder:
```rust
pub fn build_app(settings: &Settings) -> Result<App<ReqwestUserVoiceClient>, AppError> {
    let ca_cert = settings.load_ca_certificates()?;
    let mut builder = reqwest::Client::builder();
    if let Some(cert) = ca_cert {
        builder = builder.add_root_certificate(cert);
    }
    let http = builder.build().map_err(|e| AppError::Validation(e.to_string()))?;
    // ... use http client
}
```

### Modify: `crates/rusty_user_voice/src/lib.rs` — init_tracing with log_level
```rust
pub fn init_tracing(log_level: Option<&str>) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    static INIT: std::sync::Once = std::sync::Once::new();
    let mut result = Ok(());
    INIT.call_once(|| {
        let filter = std::env::var("RUST_LOG")
            .ok()
            .filter(|v| !v.is_empty())
            .or_else(|| log_level.map(String::from))
            .unwrap_or_else(|| "warn".to_owned());
        result = tracing_subscriber::fmt()
            .compact()
            .with_ansi(false)
            .with_target(false)
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .try_init()
            .map_err(|e| e.into());
    });
    result
}
```

### Modify: `crates/rusty_user_voice/src/bin/cli.rs`
Pass `log_level` from settings to `init_tracing`:
```rust
init_tracing(settings.log_level.as_deref()).ok();
```

### Modify: `crates/rusty_user_voice/src/main.rs`
Same for MCP binary.

### Modify: `crates/rusty_user_voice/tests/config_test.rs` (new)
Tests for:
- Config loads from YAML file
- CLI flags override env vars override config.yaml
- Missing api_key returns error with clear message
- ca_cert loading from PEM file
- default_output from config
- log_level from config
- config_path resolution (binary-dir and ~/.config/)

### Modify: `README.md`
Add configuration section documenting:
- Config file locations
- Priority chain
- All 7 fields with examples
- Security note about config.yaml

## Completion criterion

- `cargo test --workspace` green
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean
- `cargo fmt --all -- --check` clean
- `config.example.yaml` tracked, `config.yaml` in `.gitignore`
- New config tests pass
- README updated with config documentation
