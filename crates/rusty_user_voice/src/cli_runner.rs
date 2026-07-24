use std::io::{self, Write};

use serde::Serialize;
use uservoice_client::UserVoiceApi;

use crate::{
    App, Checkpoint, CheckpointStore, CollectIdeasParams, GetIdeaContextParams, SearchIdeasParams,
    WorkspaceContextParams,
    cli::{self, Cli, Command, IdeasCommand, OutputMode},
    error::AppError,
};

pub async fn run_cli<G: UserVoiceApi>(
    app: &App<G>,
    checkpoints: &CheckpointStore,
    cli: Cli,
    output: &mut dyn Write,
) -> Result<(), AppError> {
    let mode = cli.output;
    match cli.command {
        Command::Workspace { include_catalog } => {
            let result = app
                .workspace_context(WorkspaceContextParams { include_catalog })
                .await?;
            write_result(output, mode, &result)?;
        }
        Command::Ideas { command } => match *command {
            IdeasCommand::Search(args) => {
                let result = app
                    .search_ideas(SearchIdeasParams {
                        query: args.query,
                        updated_after: args.updated_after,
                        updated_before: args.updated_before,
                        forum_ids: cli::forum_ids(args.forum_ids),
                        category_ids: cli::category_ids(args.category_ids),
                        status_ids: cli::status_ids(args.status_ids),
                        label_ids: cli::label_ids(args.label_ids),
                        states: args.states,
                        sort: args.sort,
                        per_page: args.per_page,
                        continuation: cli::continuation(args.cursor, args.page),
                    })
                    .await?;
                write_result(output, mode, &result)?;
            }
            IdeasCommand::Get(args) => {
                let result = app
                    .get_idea_context(GetIdeaContextParams {
                        idea_id: cli::idea_id(args.idea_id),
                        detail_level: args.detail_level.into(),
                        include_supporters: args.include_supporters,
                        per_page: args.per_page,
                    })
                    .await?;
                write_result(output, mode, &result)?;
            }
            IdeasCommand::Collect(args) => {
                let explicit_continuation = cli::continuation(args.cursor, args.page);
                let (updated_before, continuation) = match args.checkpoint_name.as_deref() {
                    Some(name) => match checkpoints.load(name).await? {
                        Some(saved) => {
                            if saved.completed {
                                return Err(AppError::Validation(format!(
                                    "checkpoint '{name}' is already complete"
                                )));
                            }
                            if saved.updated_after != args.updated_after
                                || args
                                    .updated_before
                                    .is_some_and(|value| Some(value) != saved.updated_before)
                            {
                                return Err(AppError::Validation(format!(
                                    "checkpoint '{name}' collection window does not match"
                                )));
                            }
                            if explicit_continuation.is_some()
                                && explicit_continuation != saved.continuation
                            {
                                return Err(AppError::Validation(format!(
                                    "checkpoint '{name}' continuation does not match"
                                )));
                            }
                            (
                                saved.updated_before,
                                explicit_continuation.or(saved.continuation),
                            )
                        }
                        None => (
                            args.updated_before.or_else(|| Some(chrono::Utc::now())),
                            explicit_continuation,
                        ),
                    },
                    None => (args.updated_before, explicit_continuation),
                };
                let result = app
                    .collect_ideas(CollectIdeasParams {
                        updated_after: args.updated_after,
                        updated_before,
                        detail_level: args.detail_level.into(),
                        per_page: args.per_page,
                        continuation,
                    })
                    .await?;
                if mode == OutputMode::Ndjson {
                    for idea in &result.data.ideas {
                        serde_json::to_writer(&mut *output, idea)?;
                        writeln!(output)?;
                    }
                } else {
                    write_result(output, mode, &result)?;
                }
                output.flush()?;
                if let Some(name) = args.checkpoint_name {
                    let next = result
                        .pagination
                        .as_ref()
                        .and_then(|pagination| pagination.next.clone());
                    checkpoints
                        .commit(&Checkpoint {
                            name,
                            continuation: next.clone(),
                            updated_after: args.updated_after,
                            updated_before,
                            completed: next.is_none(),
                        })
                        .await?;
                }
            }
            IdeasCommand::Update(args) => {
                let result = app
                    .update_idea(crate::UpdateIdeaParams {
                        mode: args.mode.into(),
                        idea_id: uservoice_client::IdeaId::new(args.idea_id),
                        title: args.title,
                        body: args.body,
                        forum_id: args.forum_id.map(uservoice_client::ForumId::new),
                        category_id: args.category_id.map(uservoice_client::CategoryId::new),
                        assigned_owner_id: args
                            .assigned_owner_id
                            .map(uservoice_client::UserId::new),
                        add_label_ids: args
                            .add_label_ids
                            .into_iter()
                            .map(uservoice_client::LabelId::new)
                            .collect(),
                        remove_label_ids: args
                            .remove_label_ids
                            .into_iter()
                            .map(uservoice_client::LabelId::new)
                            .collect(),
                        internal_note: args.internal_note,
                        change_id: args.change_id.map(crate::ChangeId::from_uuid),
                        confirmation: args.confirmation,
                        idempotency_key: args.idempotency_key,
                    })
                    .await?;
                write_result(output, mode, &result)?;
            }
            IdeasCommand::Publish(args) => {
                let result = app
                    .publish_idea_update(crate::PublishIdeaUpdateParams {
                        mode: args.mode.into(),
                        idea_id: uservoice_client::IdeaId::new(args.idea_id),
                        status_id: uservoice_client::StatusId::new(args.status_id),
                        message: args.message,
                        notify_supporters: args.notify_supporters,
                        change_id: args.change_id.map(crate::ChangeId::from_uuid),
                        confirmation: args.confirmation,
                        idempotency_key: args.idempotency_key,
                    })
                    .await?;
                write_result(output, mode, &result)?;
            }
        },
        Command::Feedback { command } => match *command {
            crate::cli::FeedbackCommand::Record(args) => {
                let result = app
                    .record_feedback(crate::RecordFeedbackParams {
                        body: args.body,
                        channel: args.channel.into(),
                        idea_id: args.idea_id.map(uservoice_client::IdeaId::new),
                        user_id: args.user_id.map(uservoice_client::UserId::new),
                        user_email: args.user_email,
                        source_url: args.source_url,
                        source_guid: args.source_guid,
                        created_at: args.created_at,
                        idempotency_key: args.idempotency_key,
                    })
                    .await?;
                write_result(output, mode, &result)?;
            }
        },
    }
    Ok(())
}

fn write_result<T: Serialize>(
    output: &mut dyn Write,
    mode: OutputMode,
    value: &T,
) -> Result<(), AppError> {
    match mode {
        OutputMode::Json => serde_json::to_writer_pretty(&mut *output, value)?,
        OutputMode::Ndjson => {
            serde_json::to_writer(&mut *output, value)?;
            writeln!(output)?;
        }
        OutputMode::Human => {
            let value = serde_json::to_value(value)?;
            let status = value
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("ok");
            let count = value
                .pointer("/data/ideas")
                .and_then(serde_json::Value::as_array)
                .map_or(1, Vec::len);
            writeln!(output, "status={status} records={count}")?;
        }
    }
    Ok(())
}

pub fn stdout() -> io::Stdout {
    io::stdout()
}
