mod collect;
pub mod feedback;
mod mutate;
mod read;

use std::{collections::BTreeMap, num::NonZeroUsize, sync::Arc};

use uservoice_client::UserVoiceApi;

use crate::{
    contracts::{Envelope, MutationResult, NextAction, OutcomeStatus},
    ledger::{Clock, DiskChangeLedger, SystemClock},
};

#[derive(Clone)]
pub struct App<G: UserVoiceApi> {
    pub(crate) api: Arc<G>,
    pub(crate) max_in_flight: NonZeroUsize,
    pub(crate) ledger: Option<Arc<DiskChangeLedger>>,
    pub(crate) clock: Arc<dyn Clock>,
}

impl<G: UserVoiceApi> App<G> {
    pub fn new(api: Arc<G>, max_in_flight: usize) -> Self {
        let max_in_flight = NonZeroUsize::new(max_in_flight.clamp(1, 8))
            .expect("clamp(1, 8) guarantees non-zero; violated only by a clamp bug");
        Self {
            api,
            max_in_flight,
            ledger: None,
            clock: Arc::new(SystemClock),
        }
    }

    pub fn with_ledger(mut self, ledger: Arc<DiskChangeLedger>) -> Self {
        self.ledger = Some(ledger);
        self
    }

    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Hands the underlying implementation back to live smoke tests so they
    /// can exercise raw adapter calls without going through domain envelopes
    /// (and without exposing the seam in production builds). Only compiled
    /// in tests and when the `live-smoke` Cargo feature is on.
    #[cfg(any(test, feature = "live-smoke"))]
    #[must_use]
    pub fn api_for_smoke(&self) -> &G {
        &self.api
    }
}

/// Shared envelope builder for the apply-step of every mutation. Inspects the
/// applied-operations ledger to flip the envelope to `Partial` when any op
/// failed, and points the agent at `uservoice_get_idea_context` so the next
/// call refreshes the affected idea.
pub(super) fn mutation_envelope(result: MutationResult) -> Envelope<MutationResult> {
    let idea_id = result.idea_id;
    let partial = result
        .applied_operations
        .iter()
        .any(|operation| operation.outcome.starts_with("failed:"));
    let guidance = partial.then(|| {
        "Do not retry with a new key. Refresh the idea context and inspect completed operations."
            .to_owned()
    });
    let next_actions = vec![NextAction {
        tool: "uservoice_get_idea_context".to_owned(),
        reason: "Refresh the idea after apply, or review the preview before apply.".to_owned(),
        idea_id: Some(idea_id),
        arguments: BTreeMap::from([("idea_id".to_owned(), serde_json::json!(idea_id))]),
    }];
    Envelope {
        status: if partial {
            OutcomeStatus::Partial
        } else {
            OutcomeStatus::Ok
        },
        data: result,
        next_actions,
        guidance,
        pagination: None,
        rate_limit: None,
    }
}
