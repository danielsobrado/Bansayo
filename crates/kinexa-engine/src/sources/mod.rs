//! Independent source tasks. Ingest drops newest at capacity; counters remain observable.
pub mod simulated;
pub mod supervision;
pub mod validation;
use crate::clock::SessionClock;
use kinexa_contracts::{DeviceId, SourceEvent};
use simulated::Control;
use std::collections::BTreeMap;
use std::sync::{Arc, atomic::AtomicU64};
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;
pub type DropCounters = BTreeMap<DeviceId, Arc<AtomicU64>>;
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct SourceError(pub String);
#[allow(clippy::large_enum_variant)]
pub enum Ingest {
    Event(SourceEvent),
    Failure {
        devices: Vec<DeviceId>,
        message: String,
    },
}
#[derive(Clone)]
pub struct SourceContext {
    pub events: mpsc::Sender<Ingest>,
    pub control: watch::Receiver<Control>,
    pub clock: SessionClock,
    pub cancel: CancellationToken,
    pub dropped: DropCounters,
    pub faults: watch::Receiver<BTreeMap<DeviceId, simulated::FaultSetting>>,
}
pub trait Source: Send + 'static {
    fn run(
        self,
        context: SourceContext,
    ) -> impl std::future::Future<Output = Result<(), SourceError>> + Send;
}
