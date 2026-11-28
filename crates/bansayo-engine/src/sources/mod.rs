//! Independent source tasks. Ingest drops newest at capacity; counters remain observable.
pub mod camera;
mod diagnostic_stream;
pub mod esp32;
pub mod inspection;
pub mod simulated;
pub mod slimevr;
pub mod supervision;
pub mod validation;
pub mod xr;
use crate::clock::SessionClock;
use bansayo_contracts::{DeviceId, SourceEvent};
use std::collections::BTreeMap;
use std::sync::{Arc, atomic::AtomicU64};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
pub type DropCounters = BTreeMap<DeviceId, Arc<AtomicU64>>;
/// Live per-device state a source publishes outside the observation stream (camera lifecycle,
/// clock mapping). Never recorded: replay reconstructs from observations alone.
#[derive(Debug, Clone, Default)]
pub struct DeviceRuntime {
    pub camera: Option<bansayo_contracts::CameraRuntimeStatus>,
    pub time_sync: Option<bansayo_contracts::TimeSyncStatus>,
    pub last_inference_us: Option<u64>,
    pub inspection: Option<inspection::SharedTrace>,
}
pub type RuntimeMap = Arc<std::sync::RwLock<BTreeMap<DeviceId, DeviceRuntime>>>;
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct SourceError(pub String);
#[allow(clippy::large_enum_variant)]
pub enum Ingest {
    Event(SourceEvent),
    Magnetic {
        device_id: DeviceId,
        channel: &'static str,
        t_us: u64,
        seq: u32,
        device_time_us: u32,
        values: [i16; 3],
    },
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
