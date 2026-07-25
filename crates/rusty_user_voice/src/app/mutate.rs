use crate::{
    App, AppliedOperation, Envelope, FieldChange, MutationMode, MutationResult, PreviewKind,
    PreviewRecord, UpdateIdeaParams,
    error::AppError,
    ledger::{Claim, PREVIEW_TTL, fingerprint},
};
use serde::Serialize;
use uservoice_client::{UpdateSuggestionLinks, UpdateSuggestionRequest, UserVoiceApi};

use super::mutation_envelope;

#[derive(Serialize)]
struct UpdateIntent<'a> {
    idea_id: uservoice_client::IdeaId,
    title: &'a Option<String>,
    body: &'a Option<String>,
    forum_id: Option<uservoice_client::ForumId>,
    category_id: Option<uservoice_client::CategoryId>,
    assigned_owner_id: Option<uservoice_client::UserId>,
    add_label_ids: &'a [uservoice_client::LabelId],
    remove_label_ids: &'a [uservoice_client::LabelId],
    internal_note: &'a Option<String>,
}

impl<'a> From<&'a UpdateIdeaParams> for UpdateIntent<'a> {
    fn from(params: &'a UpdateIdeaParams) -> Self {
        Self {
            idea_id: params.idea_id,
            title: &params.title,
            body: &params.body,
            forum_id: params.forum_id,
            category_id: params.category_id,
            assigned_owner_id: params.assigned_owner_id,
            add_label_ids: &params.add_label_ids,
            remove_label_ids: &params.remove_label_ids,
            internal_note: &params.internal_note,
        }
    }
}

impl<G: UserVoiceApi> App<G> {
    pub async fn update_idea(
        &self,
        params: UpdateIdeaParams,
    ) -> Result<Envelope<MutationResult>, AppError> {
        match params.mode {
            MutationMode::Preview => self.preview_update(params).await,
            MutationMode::Apply => self.apply_update(params).await,
        }
    }

    async fn preview_update(
        &self,
        params: UpdateIdeaParams,
    ) -> Result<Envelope<MutationResult>, AppError> {
        validate_preview(&params)?;
        let current = self.api.suggestion(params.idea_id).await?.data;
        let now = self.clock.now();
        let change_id = crate::ChangeId::new();
        let changes = field_changes(&current, &params);
        let intent = UpdateIntent::from(&params);
        let changed_fields = changes.iter().map(|change| change.field.clone()).collect();
        let record = PreviewRecord {
            change_id,
            kind: PreviewKind::UpdateIdea,
            idea_id: params.idea_id,
            expected_updated_at: current.updated_at,
            created_at: now,
            expires_at: now + PREVIEW_TTL,
            fingerprint: fingerprint(&intent)?,
            changed_fields,
        };
        ledger(self)?.save_preview(record.clone()).await?;
        Ok(mutation_envelope(MutationResult {
            mode: MutationMode::Preview,
            change_id: Some(change_id),
            idea_id: params.idea_id,
            expected_updated_at: current.updated_at,
            expires_at: Some(record.expires_at),
            changes,
            applied_operations: Vec::new(),
            feedback_record_id: None,
        }))
    }

    async fn apply_update(
        &self,
        params: UpdateIdeaParams,
    ) -> Result<Envelope<MutationResult>, AppError> {
        let (change_id, idempotency_key) = validate_apply(&params)?;
        let record = ledger(self)?.load_preview(change_id).await?;
        if !matches!(record.kind, PreviewKind::UpdateIdea) || record.idea_id != params.idea_id {
            return Err(AppError::Validation(
                "change_id belongs to a different mutation or idea_id".to_owned(),
            ));
        }
        if self.clock.now() > record.expires_at {
            return Err(AppError::Conflict("change_id has expired".to_owned()));
        }
        let intent_fingerprint = fingerprint(&UpdateIntent::from(&params))?;
        if intent_fingerprint != record.fingerprint {
            return Err(AppError::Conflict(
                "apply payload does not match the preview".to_owned(),
            ));
        }
        let idempotency_fingerprint = fingerprint(&(change_id, record.fingerprint.as_str()))?;
        if let Some(result) = ledger(self)?
            .replay(&idempotency_key, &idempotency_fingerprint)
            .await?
        {
            return Ok(mutation_envelope(result));
        }
        let fresh = self.api.suggestion(params.idea_id).await?.data;
        if fresh.updated_at != record.expected_updated_at {
            return Err(AppError::Conflict(
                "idea changed since preview; create a new preview".to_owned(),
            ));
        }
        match ledger(self)?
            .claim(&idempotency_key, &idempotency_fingerprint)
            .await?
        {
            Claim::Replay(result) => return Ok(mutation_envelope(result)),
            Claim::Execute => {}
        }

        let mut applied = Vec::new();
        let links = if params.forum_id.is_some()
            || params.category_id.is_some()
            || params.assigned_owner_id.is_some()
        {
            Some(UpdateSuggestionLinks {
                forum: params.forum_id,
                category: params.category_id,
                assigned_owner: params.assigned_owner_id,
            })
        } else {
            None
        };
        let fields = UpdateSuggestionRequest {
            title: params.title.clone(),
            body: params.body.clone(),
            links,
        };
        if fields.title.is_some() || fields.body.is_some() || fields.links.is_some() {
            if let Err(error) = self.api.update_suggestion(params.idea_id, fields).await {
                return self
                    .finish_partial(
                        &idempotency_key,
                        &idempotency_fingerprint,
                        &record,
                        applied,
                        "update_suggestion",
                        error,
                    )
                    .await;
            }
            applied.push(applied_operation(
                "update_suggestion",
                params.idea_id.to_string(),
            ));
        }
        for label_id in &params.add_label_ids {
            if let Err(error) = self.api.attach_label(params.idea_id, *label_id).await {
                return self
                    .finish_partial(
                        &idempotency_key,
                        &idempotency_fingerprint,
                        &record,
                        applied,
                        "attach_label",
                        error,
                    )
                    .await;
            }
            applied.push(applied_operation("attach_label", label_id.to_string()));
        }
        for label_id in &params.remove_label_ids {
            if let Err(error) = self.api.detach_label(params.idea_id, *label_id).await {
                return self
                    .finish_partial(
                        &idempotency_key,
                        &idempotency_fingerprint,
                        &record,
                        applied,
                        "detach_label",
                        error,
                    )
                    .await;
            }
            applied.push(applied_operation("detach_label", label_id.to_string()));
        }
        if let Some(note) = &params.internal_note {
            match self.api.create_note(params.idea_id, note.clone()).await {
                Ok(value) => {
                    applied.push(applied_operation("create_note", value.data.id.to_string()));
                }
                Err(error) => {
                    return self
                        .finish_partial(
                            &idempotency_key,
                            &idempotency_fingerprint,
                            &record,
                            applied,
                            "create_note",
                            error,
                        )
                        .await;
                }
            }
        }
        let result = MutationResult {
            mode: MutationMode::Apply,
            change_id: Some(change_id),
            idea_id: params.idea_id,
            expected_updated_at: record.expected_updated_at,
            expires_at: Some(record.expires_at),
            changes: Vec::new(),
            applied_operations: applied,
            feedback_record_id: None,
        };
        ledger(self)?
            .complete(&idempotency_key, idempotency_fingerprint, result.clone())
            .await?;
        Ok(mutation_envelope(result))
    }

    async fn finish_partial(
        &self,
        idempotency_key: &str,
        idempotency_fingerprint: &str,
        record: &PreviewRecord,
        mut applied: Vec<AppliedOperation>,
        failed_operation: &str,
        error: uservoice_client::ClientError,
    ) -> Result<Envelope<MutationResult>, AppError> {
        applied.push(AppliedOperation {
            operation: failed_operation.to_owned(),
            resource_id: None,
            outcome: format!("failed: {error}"),
        });
        let result = MutationResult {
            mode: MutationMode::Apply,
            change_id: Some(record.change_id),
            idea_id: record.idea_id,
            expected_updated_at: record.expected_updated_at,
            expires_at: Some(record.expires_at),
            changes: Vec::new(),
            applied_operations: applied,
            feedback_record_id: None,
        };
        ledger(self)?
            .complete(
                idempotency_key,
                idempotency_fingerprint.to_owned(),
                result.clone(),
            )
            .await?;
        Ok(mutation_envelope(result))
    }
}

fn ledger<G: UserVoiceApi>(app: &App<G>) -> Result<&crate::DiskChangeLedger, AppError> {
    app.ledger
        .as_deref()
        .ok_or_else(|| AppError::State("mutation ledger is not configured".to_owned()))
}

fn validate_preview(params: &UpdateIdeaParams) -> Result<(), AppError> {
    if params.change_id.is_some()
        || params.confirmation.is_some()
        || params.idempotency_key.is_some()
    {
        return Err(AppError::Validation(
            "preview does not accept change_id, confirmation, or idempotency_key".to_owned(),
        ));
    }
    if params.title.is_none()
        && params.body.is_none()
        && params.forum_id.is_none()
        && params.category_id.is_none()
        && params.assigned_owner_id.is_none()
        && params.add_label_ids.is_empty()
        && params.remove_label_ids.is_empty()
        && params.internal_note.is_none()
    {
        return Err(AppError::Validation(
            "preview requires at least one requested change".to_owned(),
        ));
    }
    Ok(())
}

fn validate_apply(params: &UpdateIdeaParams) -> Result<(crate::ChangeId, String), AppError> {
    if params.confirmation.as_deref() != Some("apply") {
        return Err(AppError::Validation(
            "apply requires confirmation=\"apply\"".to_owned(),
        ));
    }
    let change_id = params
        .change_id
        .ok_or_else(|| AppError::Validation("apply requires change_id".to_owned()))?;
    let key = params
        .idempotency_key
        .clone()
        .ok_or_else(|| AppError::Validation("apply requires idempotency_key".to_owned()))?;
    Ok((change_id, key))
}

fn applied_operation(operation: &str, resource_id: String) -> AppliedOperation {
    AppliedOperation {
        operation: operation.to_owned(),
        resource_id: Some(resource_id),
        outcome: "applied".to_owned(),
    }
}

fn field_changes(
    current: &uservoice_client::Suggestion,
    params: &UpdateIdeaParams,
) -> Vec<FieldChange> {
    let mut changes = Vec::new();
    if let Some(value) = &params.title {
        changes.push(FieldChange {
            field: "title".to_owned(),
            before: Some(current.title.clone()),
            after: Some(value.clone()),
        });
    }
    if let Some(value) = &params.body {
        changes.push(FieldChange {
            field: "body".to_owned(),
            before: Some(current.body.clone()),
            after: Some(value.clone()),
        });
    }
    for (field, value) in [
        ("forum_id", params.forum_id.map(|id| id.to_string())),
        ("category_id", params.category_id.map(|id| id.to_string())),
        (
            "assigned_owner_id",
            params.assigned_owner_id.map(|id| id.to_string()),
        ),
    ] {
        if let Some(value) = value {
            changes.push(FieldChange {
                field: field.to_owned(),
                before: None,
                after: Some(value),
            });
        }
    }
    if !params.add_label_ids.is_empty() {
        changes.push(FieldChange {
            field: "add_label_ids".to_owned(),
            before: None,
            after: Some(
                params
                    .add_label_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        });
    }
    if !params.remove_label_ids.is_empty() {
        changes.push(FieldChange {
            field: "remove_label_ids".to_owned(),
            before: None,
            after: Some(
                params
                    .remove_label_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            ),
        });
    }
    if let Some(value) = &params.internal_note {
        changes.push(FieldChange {
            field: "internal_note".to_owned(),
            before: None,
            after: Some(value.clone()),
        });
    }
    changes
}
