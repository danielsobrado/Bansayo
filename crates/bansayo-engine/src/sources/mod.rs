//! Independent source tasks. Ingest drops newest at capacity; counters remain observable.
pub mod camera;
pub mod simulated;
pub mod supervision;
pub mod validation;
use crate::clock::SessionClock;
use bansayo_contracts::{DeviceId, SourceEvent};
use std::collections::BTreeMap;
use std::sync::{Arc, atomic::AtomicU64};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
pub type DropCounters = BTreeMap<DeviceId, Arc<AtomicU64>>;
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct SourceError(pub String);
#[allow(clippy::large_enum_variant)]
pub enum Ingest {
    Event(SourceEvent),
    /// A condition the user should see that is not a device state, e.g. `multiple_people`.
    Notice {
        code: String,
        message: String,
    },
    Failure {
        devices: Vec<DeviceId>,
        message: String,
    },
}
#[derive(Clone)]
pub struct SourceContext {
    pub events: mpsc::Sender<Ingest>,
    pub clock: SessionClock,
    pub cancel: CancellationToken,
    pub dropped: DropCounters,
}
impl AsRef<SourceContext> for SourceContext {
    fn as_ref(&self) -> &SourceContext {
        self
    }
}
pub trait Source: Send + 'static {
    type Context: AsRef<SourceContext> + Clone + Send + Sync + 'static;
    fn run(
        self,
        context: Self::Context,
    ) -> impl std::future::Future<Output = Result<(), SourceError>> + Send;
}
