use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{ChangeId, MutationResult, error::AppError};
use uservoice_client::IdeaId;

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

pub struct FixedClock {
    now: DateTime<Utc>,
}

impl FixedClock {
    pub fn new(now: DateTime<Utc>) -> Self {
        Self { now }
    }
}

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.now
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewKind {
    UpdateIdea,
    PublishIdeaUpdate,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PreviewRecord {
    pub change_id: ChangeId,
    pub kind: PreviewKind,
    pub idea_id: IdeaId,
    pub expected_updated_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub fingerprint: String,
    pub changed_fields: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum IdempotencyRecord {
    Pending {
        fingerprint: String,
    },
    Completed {
        fingerprint: String,
        result: MutationResult,
    },
}

pub enum Claim {
    Execute,
    Replay(MutationResult),
}

pub struct DiskChangeLedger {
    root: PathBuf,
    gate: Arc<Mutex<()>>,
}

impl DiskChangeLedger {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            gate: Arc::new(Mutex::new(())),
        }
    }

    pub async fn save_preview(&self, record: PreviewRecord) -> Result<(), AppError> {
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || {
            let path = root
                .join("changes")
                .join(format!("{}.json", record.change_id.as_uuid()));
            write_json_atomic(&path, &record)
        })
        .await
        .map_err(|error| AppError::State(error.to_string()))?
    }

    pub async fn load_preview(&self, change_id: ChangeId) -> Result<PreviewRecord, AppError> {
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || {
            let path = root
                .join("changes")
                .join(format!("{}.json", change_id.as_uuid()));
            let body = std::fs::read(path)?;
            Ok(serde_json::from_slice(&body)?)
        })
        .await
        .map_err(|error| AppError::State(error.to_string()))?
    }

    pub async fn replay(
        &self,
        key: &str,
        fingerprint: &str,
    ) -> Result<Option<MutationResult>, AppError> {
        validate_key(key)?;
        let path = self
            .root
            .join("idempotency")
            .join(format!("{}.json", hash(key)));
        let fingerprint = fingerprint.to_owned();
        let gate = Arc::clone(&self.gate);
        tokio::task::spawn_blocking(move || {
            let _guard = gate
                .lock()
                .map_err(|_| AppError::State("ledger lock poisoned".to_owned()))?;
            let body = match std::fs::read(path) {
                Ok(body) => body,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                Err(error) => return Err(error.into()),
            };
            match serde_json::from_slice::<IdempotencyRecord>(&body)? {
                IdempotencyRecord::Completed {
                    fingerprint: stored,
                    result,
                } if stored == fingerprint => Ok(Some(result)),
                IdempotencyRecord::Pending {
                    fingerprint: stored,
                } if stored == fingerprint => Err(AppError::Conflict(
                    "the same idempotency key is already in progress".to_owned(),
                )),
                _ => Err(AppError::Conflict(
                    "idempotency key was already used for a different request".to_owned(),
                )),
            }
        })
        .await
        .map_err(|error| AppError::State(error.to_string()))?
    }

    pub async fn claim(&self, key: &str, fingerprint: &str) -> Result<Claim, AppError> {
        validate_key(key)?;
        let path = self
            .root
            .join("idempotency")
            .join(format!("{}.json", hash(key)));
        let fingerprint = fingerprint.to_owned();
        let gate = Arc::clone(&self.gate);
        tokio::task::spawn_blocking(move || {
            let _guard = gate
                .lock()
                .map_err(|_| AppError::State("ledger lock poisoned".to_owned()))?;
            let parent = path
                .parent()
                .ok_or_else(|| AppError::State("idempotency path has no parent".to_owned()))?;
            std::fs::create_dir_all(parent)?;
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    let body = serde_json::to_vec(&IdempotencyRecord::Pending {
                        fingerprint: fingerprint.clone(),
                    })?;
                    file.write_all(&body)?;
                    file.sync_all()?;
                    Ok(Claim::Execute)
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    let body = std::fs::read(path)?;
                    match serde_json::from_slice::<IdempotencyRecord>(&body)? {
                        IdempotencyRecord::Completed {
                            fingerprint: stored,
                            result,
                        } if stored == fingerprint => Ok(Claim::Replay(result)),
                        IdempotencyRecord::Pending {
                            fingerprint: stored,
                        } if stored == fingerprint => Err(AppError::Conflict(
                            "the same idempotency key is already in progress".to_owned(),
                        )),
                        _ => Err(AppError::Conflict(
                            "idempotency key was already used for a different request".to_owned(),
                        )),
                    }
                }
                Err(error) => Err(error.into()),
            }
        })
        .await
        .map_err(|error| AppError::State(error.to_string()))?
    }

    pub async fn complete(
        &self,
        key: &str,
        fingerprint: String,
        result: MutationResult,
    ) -> Result<(), AppError> {
        let path = self
            .root
            .join("idempotency")
            .join(format!("{}.json", hash(key)));
        let gate = Arc::clone(&self.gate);
        tokio::task::spawn_blocking(move || {
            let _guard = gate
                .lock()
                .map_err(|_| AppError::State("ledger lock poisoned".to_owned()))?;
            write_json_atomic(
                &path,
                &IdempotencyRecord::Completed {
                    fingerprint,
                    result,
                },
            )
        })
        .await
        .map_err(|error| AppError::State(error.to_string()))?
    }
}

fn validate_key(key: &str) -> Result<(), AppError> {
    if key.is_empty() || key.len() > 128 || key.chars().any(char::is_whitespace) {
        return Err(AppError::Validation(
            "idempotency_key must contain 1-128 non-whitespace characters".to_owned(),
        ));
    }
    Ok(())
}

fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

pub fn fingerprint<T: Serialize>(value: &T) -> Result<String, AppError> {
    Ok(format!("{:x}", Sha256::digest(serde_json::to_vec(value)?)))
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<(), AppError> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::State("ledger path has no parent".to_owned()))?;
    std::fs::create_dir_all(parent)?;
    let temporary = path.with_extension("json.tmp");
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

pub const PREVIEW_TTL: Duration = Duration::minutes(15);
