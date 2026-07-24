mod collect;
mod read;

use std::{num::NonZeroUsize, sync::Arc};

use uservoice_client::UserVoiceApi;

#[derive(Clone)]
pub struct App<G: UserVoiceApi> {
    pub(crate) api: Arc<G>,
    pub(crate) max_in_flight: NonZeroUsize,
}

impl<G: UserVoiceApi> App<G> {
    pub fn new(api: Arc<G>, max_in_flight: usize) -> Self {
        // INVARIANT: clamp(1, 8) guarantees the result is in [1, 8]; NonZeroUsize::new
        // therefore cannot return None. If this expect ever fires, the clamp is wrong.
        let max_in_flight = NonZeroUsize::new(max_in_flight.clamp(1, 8))
            .expect("clamp(1, 8) guarantees non-zero; violated only by a clamp bug");
        Self { api, max_in_flight }
    }
}
