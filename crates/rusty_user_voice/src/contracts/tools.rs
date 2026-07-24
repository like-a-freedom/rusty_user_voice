#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ToolName {
    WorkspaceContext,
    SearchIdeas,
    GetIdeaContext,
    CollectIdeas,
    UpdateIdea,
    PublishIdeaUpdate,
    RecordFeedback,
}

impl ToolName {
    pub const ALL: &'static [Self] = &[
        Self::WorkspaceContext,
        Self::SearchIdeas,
        Self::GetIdeaContext,
        Self::CollectIdeas,
        Self::UpdateIdea,
        Self::PublishIdeaUpdate,
        Self::RecordFeedback,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::WorkspaceContext => "uservoice_workspace_context",
            Self::SearchIdeas => "uservoice_search_ideas",
            Self::GetIdeaContext => "uservoice_get_idea_context",
            Self::CollectIdeas => "uservoice_collect_ideas",
            Self::UpdateIdea => "uservoice_update_idea",
            Self::PublishIdeaUpdate => "uservoice_publish_idea_update",
            Self::RecordFeedback => "uservoice_record_feedback",
        }
    }

    pub fn all_names() -> String {
        Self::ALL
            .iter()
            .copied()
            .map(Self::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    }
}
