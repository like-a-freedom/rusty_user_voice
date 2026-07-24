use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uservoice_client::Continuation;

use crate::error::AppError;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Checkpoint {
    pub name: String,
    pub continuation: Option<Continuation>,
    pub updated_after: chrono::DateTime<chrono::Utc>,
    pub updated_before: Option<chrono::DateTime<chrono::Utc>>,
    pub completed: bool,
}

#[derive(Clone)]
pub struct CheckpointStore {
    directory: PathBuf,
}

impl CheckpointStore {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub async fn commit(&self, checkpoint: &Checkpoint) -> Result<(), AppError> {
        validate_name(&checkpoint.name)?;
        tokio::fs::create_dir_all(&self.directory).await?;
        let final_path = self.directory.join(format!("{}.json", checkpoint.name));
        let temporary_path = self.directory.join(format!("{}.json.tmp", checkpoint.name));
        let body = serde_json::to_vec_pretty(checkpoint)?;
        tokio::fs::write(&temporary_path, body).await?;
        tokio::fs::rename(&temporary_path, &final_path).await?;
        Ok(())
    }

    pub async fn load(&self, name: &str) -> Result<Option<Checkpoint>, AppError> {
        validate_name(name)?;
        let path = self.directory.join(format!("{name}.json"));
        match tokio::fs::read(path).await {
            Ok(body) => Ok(Some(serde_json::from_slice(&body)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

fn validate_name(name: &str) -> Result<(), AppError> {
    if name.is_empty()
        || name.len() > 64
        || !name
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::Validation(
            "checkpoint_name must contain 1-64 ASCII letters, digits, '-' or '_'".to_owned(),
        ));
    }
    Ok(())
}
