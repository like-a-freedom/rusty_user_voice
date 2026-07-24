use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use chrono::{DateTime, Duration, Utc};
use serde::de::DeserializeOwned;
use uservoice_client::{
    ActivityEntry, ApiResponse, ClientError, Comment, ContextPage, CreateFeedbackRecordRequest,
    CreateStatusUpdateRequest, FeedbackRecord, IdeaId, LabelId, Note, Pagination,
    SearchSuggestionsRequest, StatusUpdate, Suggestion, SuggestionPage, Supporter,
    UpdateSuggestionRequest, User, UserVoiceApi, WorkspaceCatalog,
};

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct RecordedWrite {
    pub operation: &'static str,
    pub notify_supporters: bool,
}

#[derive(Clone)]
pub struct FakeUserVoiceApi {
    failed_sections: Arc<HashSet<&'static str>>,
    suggestion_updated_at: Arc<Mutex<DateTime<Utc>>>,
    writes: Arc<Mutex<Vec<RecordedWrite>>>,
    fail_write_number: Arc<Mutex<Option<usize>>>,
}

#[allow(dead_code)]
impl FakeUserVoiceApi {
    pub fn complete() -> Self {
        Self {
            failed_sections: Arc::new(HashSet::new()),
            suggestion_updated_at: Arc::new(Mutex::new(
                "2025-02-03T04:05:06Z".parse().expect("fixture date"),
            )),
            writes: Arc::new(Mutex::new(Vec::new())),
            fail_write_number: Arc::new(Mutex::new(None)),
        }
    }

    #[allow(dead_code)]
    pub fn with_failed_section(section: &'static str) -> Self {
        Self {
            failed_sections: Arc::new(HashSet::from([section])),
            ..Self::complete()
        }
    }

    pub fn recorded_writes(&self) -> Vec<RecordedWrite> {
        self.writes.lock().expect("writes lock").clone()
    }

    #[allow(dead_code)]
    pub fn count_writes(&self, operation: &str) -> usize {
        self.recorded_writes()
            .iter()
            .filter(|write| write.operation == operation)
            .count()
    }

    pub fn advance_suggestion_updated_at(&self) {
        let mut value = self.suggestion_updated_at.lock().expect("updated_at lock");
        *value += Duration::minutes(1);
    }

    pub fn fail_write_number(&self, number: usize) {
        *self.fail_write_number.lock().expect("failure lock") = Some(number);
    }

    fn record_write(
        &self,
        operation: &'static str,
        notify_supporters: bool,
    ) -> Result<(), ClientError> {
        let mut writes = self.writes.lock().expect("writes lock");
        let next = writes.len() + 1;
        if *self.fail_write_number.lock().expect("failure lock") == Some(next) {
            return Err(ClientError::HttpStatus { status: 503 });
        }
        writes.push(RecordedWrite {
            operation,
            notify_supporters,
        });
        Ok(())
    }

    fn fail_if_requested(&self, section: &'static str) -> Result<(), ClientError> {
        if self.failed_sections.contains(section) {
            Err(ClientError::HttpStatus { status: 503 })
        } else {
            Ok(())
        }
    }
}

impl UserVoiceApi for FakeUserVoiceApi {
    async fn current_user(&self) -> Result<ApiResponse<User>, ClientError> {
        Ok(response(json(serde_json::json!({
            "id": 7,
            "name": "Fixture PM",
            "email_address": "pm@example.invalid",
            "created_at": "2025-01-01T00:00:00Z",
            "updated_at": "2025-01-02T00:00:00Z"
        }))))
    }

    async fn workspace_catalog(&self) -> Result<ApiResponse<WorkspaceCatalog>, ClientError> {
        Ok(response(json(serde_json::json!({
            "forums": [{"id": 11, "name": "Product Ideas", "is_public": false}],
            "categories": [{"id": 12, "name": "Reporting", "links": {"forum": 11}}],
            "statuses": [{"id": 13, "name": "Planned", "is_open": true}],
            "labels": [{"id": 14, "name": "export"}]
        }))))
    }

    async fn search_suggestions(
        &self,
        _request: SearchSuggestionsRequest,
    ) -> Result<ApiResponse<SuggestionPage>, ClientError> {
        Ok(response(SuggestionPage {
            items: vec![self.suggestion_value()],
            pagination: pagination(),
        }))
    }

    async fn suggestion(&self, _idea_id: IdeaId) -> Result<ApiResponse<Suggestion>, ClientError> {
        let mut value = self.suggestion_value();
        value.updated_at = *self.suggestion_updated_at.lock().expect("updated_at lock");
        Ok(response(value))
    }

    async fn comments(
        &self,
        _idea_id: IdeaId,
        _per_page: u32,
    ) -> Result<ApiResponse<ContextPage<Comment>>, ClientError> {
        self.fail_if_requested("comments")?;
        Ok(response(context_page(vec![json(serde_json::json!({
            "id": 201,
            "created_at": "2025-02-04T00:00:00Z",
            "updated_at": "2025-02-04T00:00:00Z",
            "body": "Fixture comment"
        }))])))
    }

    async fn notes(
        &self,
        _idea_id: IdeaId,
        _per_page: u32,
    ) -> Result<ApiResponse<ContextPage<Note>>, ClientError> {
        self.fail_if_requested("notes")?;
        Ok(response(context_page(vec![json(serde_json::json!({
            "id": 301,
            "created_at": "2025-02-05T00:00:00Z",
            "updated_at": "2025-02-05T00:00:00Z",
            "body": "Fixture note"
        }))])))
    }

    async fn supporters(
        &self,
        _idea_id: IdeaId,
        _per_page: u32,
    ) -> Result<ApiResponse<ContextPage<Supporter>>, ClientError> {
        self.fail_if_requested("supporters")?;
        Ok(response(context_page(vec![json(serde_json::json!({
            "id": 401,
            "created_at": "2025-02-06T00:00:00Z",
            "updated_at": "2025-02-06T00:00:00Z",
            "email_address": "supporter@example.invalid"
        }))])))
    }

    async fn feedback_records(
        &self,
        _idea_id: IdeaId,
        _per_page: u32,
    ) -> Result<ApiResponse<ContextPage<FeedbackRecord>>, ClientError> {
        self.fail_if_requested("feedback_records")?;
        Ok(response(context_page(vec![json(serde_json::json!({
            "id": 501,
            "created_at": "2025-02-07T00:00:00Z",
            "updated_at": "2025-02-07T00:00:00Z",
            "body": "Fixture feedback"
        }))])))
    }

    async fn status_updates(
        &self,
        _idea_id: IdeaId,
        _per_page: u32,
    ) -> Result<ApiResponse<ContextPage<StatusUpdate>>, ClientError> {
        self.fail_if_requested("status_updates")?;
        Ok(response(context_page(vec![json(serde_json::json!({
            "id": 601,
            "created_at": "2025-02-08T00:00:00Z",
            "updated_at": "2025-02-08T00:00:00Z",
            "body": "Fixture public update"
        }))])))
    }

    async fn activity(
        &self,
        _idea_id: IdeaId,
        _per_page: u32,
    ) -> Result<ApiResponse<ContextPage<ActivityEntry>>, ClientError> {
        self.fail_if_requested("activity")?;
        Ok(response(context_page(vec![json(serde_json::json!({
            "id": 701,
            "created_at": "2025-02-09T00:00:00Z",
            "updated_at": "2025-02-09T00:00:00Z",
            "event": "status_changed"
        }))])))
    }

    async fn update_suggestion(
        &self,
        _idea_id: IdeaId,
        _request: UpdateSuggestionRequest,
    ) -> Result<ApiResponse<Suggestion>, ClientError> {
        self.record_write("update_suggestion", false)?;
        self.suggestion(IdeaId::new(101)).await
    }

    async fn attach_label(
        &self,
        _idea_id: IdeaId,
        _label_id: LabelId,
    ) -> Result<ApiResponse<()>, ClientError> {
        self.record_write("attach_label", false)?;
        Ok(response(()))
    }

    async fn detach_label(
        &self,
        _idea_id: IdeaId,
        _label_id: LabelId,
    ) -> Result<ApiResponse<()>, ClientError> {
        self.record_write("detach_label", false)?;
        Ok(response(()))
    }

    async fn create_note(
        &self,
        _idea_id: IdeaId,
        _body: String,
    ) -> Result<ApiResponse<Note>, ClientError> {
        self.record_write("create_note", false)?;
        Ok(response(json(serde_json::json!({
            "id": 801,
            "created_at": "2025-02-10T00:00:00Z",
            "updated_at": "2025-02-10T00:00:00Z",
            "body": "redacted fixture note"
        }))))
    }

    async fn create_status_update(
        &self,
        request: CreateStatusUpdateRequest,
    ) -> Result<ApiResponse<StatusUpdate>, ClientError> {
        self.record_write("create_status_update", request.notify_supporters)?;
        Ok(response(json(serde_json::json!({
            "id": 901,
            "created_at": "2025-02-11T00:00:00Z",
            "updated_at": "2025-02-11T00:00:00Z",
            "body": "fixture public update"
        }))))
    }

    async fn create_feedback_record(
        &self,
        _request: CreateFeedbackRecordRequest,
    ) -> Result<ApiResponse<FeedbackRecord>, ClientError> {
        self.record_write("create_feedback_record", false)?;
        Ok(response(json(serde_json::json!({
            "id": 1001,
            "created_at": "2025-02-12T00:00:00Z",
            "updated_at": "2025-02-12T00:00:00Z",
            "body": "fixture feedback"
        }))))
    }
}

impl FakeUserVoiceApi {
    fn suggestion_value(&self) -> Suggestion {
        json(serde_json::json!({
            "id": 101,
            "title": "Export roadmap as CSV",
            "body": "We need a portable report.",
            "body_mime_type": "text/plain",
            "channel": "portal",
            "state": "published",
            "created_at": "2025-01-02T03:04:05Z",
            "updated_at": "2025-02-03T04:05:06Z",
            "closed_at": null,
            "comments_count": 1,
            "notes_count": 1,
            "supporters_count": 1,
            "requests_count": 1,
            "portal_url": null,
            "admin_url": null,
            "links": {
                "forum": 11,
                "category": 12,
                "status": 13,
                "labels": [14],
                "created_by": 7,
                "assigned_owner": null
            }
        }))
    }
}

fn response<T>(data: T) -> ApiResponse<T> {
    ApiResponse {
        data,
        rate_limit: None,
        continuation: None,
    }
}

fn context_page<T>(items: Vec<T>) -> ContextPage<T> {
    ContextPage {
        items,
        pagination: pagination(),
    }
}

fn pagination() -> Pagination {
    Pagination {
        per_page: 100,
        total_records: Some(1),
        next: None,
        has_more: false,
    }
}

fn json<T: DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).expect("valid deterministic fixture")
}
