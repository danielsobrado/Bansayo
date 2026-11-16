use kinexa_contracts::*;
use tokio::sync::{mpsc, oneshot};
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct ControlError {
    pub code: ApiErrorCode,
    pub message: String,
}
impl ControlError {
    pub fn new(code: ApiErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
pub type Reply = oneshot::Sender<Result<serde_json::Value, ControlError>>;
pub enum ControlRequest {
    Assignment(DeviceId, Option<SegmentId>),
    Fault(DeviceId, FaultRequest),
    Delete(ProfileId),
    Sessions,
    RecordStart(RecordingStartRequest),
    RecordStop,
    PlaybackStart(PlaybackStartRequest),
    PlaybackUpdate(PlaybackUpdateRequest),
    PlaybackStop,
}
pub enum Completion {
    Assignment(
        std::collections::BTreeMap<DeviceId, SegmentId>,
        std::collections::BTreeSet<ProfileId>,
        DeviceId,
    ),
    Deleted(ProfileId),
    RecordingStarted(crate::recording::Recorder),
    RecordingStopped(RecordingSession),
    Sessions(Vec<RecordingSession>, Vec<String>),
    PlaybackLoaded(Box<crate::recording::reader::Recording>, f32, bool),
}
pub enum Command {
    Control(ControlRequest, Reply),
    Complete(Result<Completion, ControlError>, Reply),
    Simulation(
        SimulationUpdateRequest,
        oneshot::Sender<Result<SimulationState, ControlError>>,
    ),
    Capture(
        ReferencePoseRequest,
        oneshot::Sender<Result<Vec<CalibrationProfileSummary>, ControlError>>,
    ),
    Saved(
        Vec<CalibrationProfile>,
        Result<(), String>,
        oneshot::Sender<Result<Vec<CalibrationProfileSummary>, ControlError>>,
    ),
}
#[derive(Clone)]
pub struct Client {
    pub commands: mpsc::Sender<Command>,
}
impl Client {
    pub async fn control(
        &self,
        request: ControlRequest,
    ) -> Result<serde_json::Value, ControlError> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::Control(request, tx))
            .await
            .map_err(|_| unavailable())?;
        rx.await.map_err(|_| unavailable())?
    }

    pub async fn simulation(
        &self,
        request: SimulationUpdateRequest,
    ) -> Result<SimulationState, ControlError> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::Simulation(request, tx))
            .await
            .map_err(|_| unavailable())?;
        rx.await.map_err(|_| unavailable())?
    }
    pub async fn capture(
        &self,
        request: ReferencePoseRequest,
    ) -> Result<Vec<CalibrationProfileSummary>, ControlError> {
        let (tx, rx) = oneshot::channel();
        self.commands
            .send(Command::Capture(request, tx))
            .await
            .map_err(|_| unavailable())?;
        rx.await.map_err(|_| unavailable())?
    }
}
fn unavailable() -> ControlError {
    ControlError::new(
        ApiErrorCode::Unavailable,
        "The tracking engine is shutting down.",
    )
}
