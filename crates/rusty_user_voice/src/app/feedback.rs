use std::collections::BTreeMap;

use uservoice_client::{CreateFeedbackRecordRequest, CreateStatusUpdateRequest, UserVoiceApi};

use crate::{
    App, Envelope, MutationMode, MutationResult, NextAction, OutcomeStatus,
    PublishIdeaUpdateParams, RecordFeedbackParams,
    error::AppError,
    ledger::{PREVIEW_TTL, PreviewKind, fingerprint},
};

impl<G: UserVoiceApi> App<G> {
    pub async fn publish_idea_update(
        &self,
        params: PublishIdeaUpdateParams,
    ) -> Result<Envelope<MutationResult>, AppError> {
        match params.mode {
            MutationMode::Preview => self.preview_status_update(params).await,
            MutationMode::Apply => self.apply_status_update(params).await,
        }
    }

    async fn preview_status_update(
        &self,
        params: PublishIdeaUpdateParams,
    ) -> Result<Envelope<MutationResult>, AppError> {
        validate_publish_preview(&params)?;
        let current = self.api.suggestion(params.idea_id).await?.data;
        let now = self.clock.now();
        let change_id = crate::ChangeId::new();
        let record = crate::PreviewRecord {
            change_id,
            kind: PreviewKind::PublishIdeaUpdate,
            idea_id: params.idea_id,
            expected_updated_at: current.updated_at,
            created_at: now,
            expires_at: now + PREVIEW_TTL,
            fingerprint: fingerprint(&(&params.idea_id, &params.status_id, &params.message))?,
            changed_fields: vec!["status_id".to_owned(), "message".to_owned()],
        };
        self.ledger
            .as_deref()
            .ok_or_else(|| AppError::State("mutation ledger is not configured".to_owned()))?
            .save_preview(record.clone())
            .await?;
        Ok(mutation_envelope(MutationResult {
            mode: MutationMode::Preview,
            change_id: Some(change_id),
            idea_id: params.idea_id,
            expected_updated_at: current.updated_at,
            expires_at: Some(record.expires_at),
            changes: Vec::new(),
            applied_operations: Vec::new(),
            feedback_record_id: None,
        }))
    }

    async fn apply_status_update(
        &self,
        params: PublishIdeaUpdateParams,
    ) -> Result<Envelope<MutationResult>, AppError> {
        let (change_id, idempotency_key) = validate_publish_apply(&params)?;
        let ledger = self
            .ledger
            .as_deref()
            .ok_or_else(|| AppError::State("mutation ledger is not configured".to_owned()))?;
        let record = ledger.load_preview(change_id).await?;
        if !matches!(record.kind, PreviewKind::PublishIdeaUpdate)
            || record.idea_id != params.idea_id
        {
            return Err(AppError::Validation(
                "change_id belongs to a different mutation or idea_id".to_owned(),
            ));
        }
        if self.clock.now() > record.expires_at {
            return Err(AppError::Conflict("change_id has expired".to_owned()));
        }
        let intent_fingerprint = fingerprint(&(params.idea_id, params.status_id, &params.message))?;
        if intent_fingerprint != record.fingerprint {
            return Err(AppError::Conflict(
                "apply payload does not match the preview".to_owned(),
            ));
        }
        let idempotency_fingerprint = fingerprint(&(change_id, record.fingerprint.as_str()))?;
        if let Some(result) = ledger
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
        match ledger
            .claim(&idempotency_key, &idempotency_fingerprint)
            .await?
        {
            crate::ledger::Claim::Replay(result) => return Ok(mutation_envelope(result)),
            crate::ledger::Claim::Execute => {}
        }

        let request = CreateStatusUpdateRequest {
            body: params.message,
            links: uservoice_client::CreateStatusUpdateLinks {
                suggestion: params.idea_id,
                status: params.status_id,
            },
            notify_supporters: params.notify_supporters,
        };
        let mut applied = Vec::new();
        match self.api.create_status_update(request).await {
            Ok(value) => {
                applied.push(crate::AppliedOperation {
                    operation: "create_status_update".to_owned(),
                    resource_id: Some(value.data.id.to_string()),
                    outcome: "applied".to_owned(),
                });
            }
            Err(error) => {
                applied.push(crate::AppliedOperation {
                    operation: "create_status_update".to_owned(),
                    resource_id: None,
                    outcome: format!("failed: {error}"),
                });
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
                ledger
                    .complete(&idempotency_key, idempotency_fingerprint, result.clone())
                    .await?;
                let mut envelope = mutation_envelope(result);
                envelope.status = OutcomeStatus::Partial;
                envelope.guidance = Some(
                    "Do not retry with a new key. Refresh the idea context and inspect completed operations."
                        .to_owned(),
                );
                return Ok(envelope);
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
        ledger
            .complete(&idempotency_key, idempotency_fingerprint, result.clone())
            .await?;
        Ok(mutation_envelope(result))
    }

    pub async fn record_feedback(
        &self,
        params: RecordFeedbackParams,
    ) -> Result<Envelope<FeedbackRecordResult>, AppError> {
        let ledger = self
            .ledger
            .as_deref()
            .ok_or_else(|| AppError::State("mutation ledger is not configured".to_owned()))?;
        let intent_fingerprint = fingerprint(&(&params.body, &params.channel, &params.idea_id))?;
        let idempotency_fingerprint = fingerprint(&(&params.idempotency_key, &intent_fingerprint))?;

        if let Some(result) = ledger
            .replay(&params.idempotency_key, &idempotency_fingerprint)
            .await?
        {
            return Ok(Envelope {
                status: OutcomeStatus::Ok,
                data: FeedbackRecordResult {
                    feedback_record_id: result.feedback_record_id,
                },
                next_actions: vec![],
                guidance: None,
                pagination: None,
                rate_limit: None,
            });
        }

        match ledger
            .claim(&params.idempotency_key, &idempotency_fingerprint)
            .await?
        {
            crate::ledger::Claim::Replay(crate::MutationResult {
                feedback_record_id, ..
            }) => {
                return Ok(Envelope {
                    status: OutcomeStatus::Ok,
                    data: FeedbackRecordResult { feedback_record_id },
                    next_actions: vec![],
                    guidance: None,
                    pagination: None,
                    rate_limit: None,
                });
            }
            crate::ledger::Claim::Execute => {}
        }

        let request = CreateFeedbackRecordRequest {
            body: params.body,
            body_mime_type: "text/plain",
            channel: params.channel,
            created_at: params.created_at,
            source_url: params.source_url,
            source_guid: params.source_guid,
            links: uservoice_client::CreateFeedbackRecordLinks {
                suggestion: params.idea_id,
                user: params.user_id,
                user_email: params.user_email,
            },
        };
        let response = self.api.create_feedback_record(request).await?;
        let record_id = response.data.id;
        let result = crate::MutationResult {
            mode: MutationMode::Apply,
            change_id: None,
            idea_id: params.idea_id.unwrap_or(uservoice_client::IdeaId::new(0)),
            expected_updated_at: chrono::Utc::now(),
            expires_at: None,
            changes: Vec::new(),
            applied_operations: vec![crate::AppliedOperation {
                operation: "create_feedback_record".to_owned(),
                resource_id: Some(record_id.to_string()),
                outcome: "applied".to_owned(),
            }],
            feedback_record_id: Some(record_id),
        };
        ledger
            .complete(&params.idempotency_key, idempotency_fingerprint, result)
            .await?;
        Ok(Envelope {
            status: OutcomeStatus::Ok,
            data: FeedbackRecordResult {
                feedback_record_id: Some(record_id),
            },
            next_actions: vec![],
            guidance: None,
            pagination: None,
            rate_limit: response.rate_limit,
        })
    }
}

#[derive(Clone, Debug, serde::Deserialize, schemars::JsonSchema, serde::Serialize)]
pub struct FeedbackRecordResult {
    pub feedback_record_id: Option<uservoice_client::FeedbackRecordId>,
}

fn validate_publish_preview(params: &PublishIdeaUpdateParams) -> Result<(), AppError> {
    if params.change_id.is_some()
        || params.confirmation.is_some()
        || params.idempotency_key.is_some()
    {
        return Err(AppError::Validation(
            "preview does not accept change_id, confirmation, or idempotency_key".to_owned(),
        ));
    }
    Ok(())
}

fn validate_publish_apply(
    params: &PublishIdeaUpdateParams,
) -> Result<(crate::ChangeId, String), AppError> {
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

fn mutation_envelope(result: MutationResult) -> Envelope<MutationResult> {
    let idea_id = result.idea_id;
    let partial = result
        .applied_operations
        .iter()
        .any(|op| op.outcome.starts_with("failed:"));
    Envelope {
        status: if partial {
            OutcomeStatus::Partial
        } else {
            OutcomeStatus::Ok
        },
        data: result,
        next_actions: vec![NextAction {
            tool: "uservoice_get_idea_context".to_owned(),
            reason: "Refresh the idea after apply.".to_owned(),
            idea_id: Some(idea_id),
            arguments: BTreeMap::from([("idea_id".to_owned(), serde_json::json!(idea_id))]),
        }],
        guidance: None,
        pagination: None,
        rate_limit: None,
    }
}
