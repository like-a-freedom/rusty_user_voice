mod collect;
pub mod feedback;
mod mutate;
mod read;

use std::{num::NonZeroUsize, sync::Arc};

use uservoice_client::UserVoiceApi;

use crate::ledger::{Clock, DiskChangeLedger, SystemClock};

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
}
