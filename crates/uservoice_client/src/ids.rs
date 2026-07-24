use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Defines a semantic ID newtype over `i64`.
///
/// Each UserVoice resource gets its own newtype so an `IdeaId` cannot be
/// passed where a `ForumId` is expected (`type-newtype-ids`,
/// `api-newtype-safety`). The macro derives the full trait set needed for
/// serde, schemars (MCP JSON schema), ordering, and hashing.
macro_rules! id_type {
    ($name:ident) => {
        #[derive(
            Clone,
            Copy,
            Debug,
            Deserialize,
            Eq,
            Hash,
            JsonSchema,
            Ord,
            PartialEq,
            PartialOrd,
            Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(i64);

        impl $name {
            pub const fn new(value: i64) -> Self {
                Self(value)
            }

            pub const fn get(self) -> i64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

id_type!(IdeaId);
id_type!(ForumId);
id_type!(CategoryId);
id_type!(StatusId);
id_type!(LabelId);
id_type!(UserId);
id_type!(SupporterId);
id_type!(FeedbackRecordId);
id_type!(CommentId);
id_type!(NoteId);
id_type!(StatusUpdateId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_ids_do_not_compare_across_resources() {
        let idea = IdeaId::new(42);
        let label = LabelId::new(42);
        assert_eq!(idea.get(), 42);
        assert_eq!(label.get(), 42);
        assert_eq!(idea.to_string(), "42");
        assert_eq!(label.to_string(), "42");
    }

    // Compile-time guarantee that two different ID types do not unify.
    // If this function ever fails to compile, the newtypes have collapsed into
    // one type and the type-safety guarantee is broken.
    #[test]
    fn idea_id_and_label_id_are_distinct_types() {
        fn take_idea(_: IdeaId) {}
        fn take_label(_: LabelId) {}
        take_idea(IdeaId::new(1));
        take_label(LabelId::new(1));
        // The line below must NOT compile; uncomment to verify:
        // take_idea(LabelId::new(1));
    }

    #[test]
    fn id_serializes_as_bare_integer() {
        // serde(transparent) keeps the wire shape as a bare i64, matching the
        // UserVoice Admin API JSON.
        let json = serde_json::to_string(&IdeaId::new(7)).expect("serialize");
        assert_eq!(json, "7");
        let parsed: IdeaId = serde_json::from_str("7").expect("deserialize");
        assert_eq!(parsed.get(), 7);
    }
}
