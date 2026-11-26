use super::{
    Runner,
    command::{Command, Completion, ControlError, ControlRequest, Reply},
    runtime::Runtime,
};
use crate::{
    recording::{Recorder, reader, replay::Replay},
    registry::persistence::{remove_legacy_profile_file, save_runtime_snapshot},
    sources::simulated::camera::DECLARED_BY_SIMULATOR,
};
use bansayo_contracts::*;
use serde_json::{Value, json};
impl Runtime {
    pub(super) fn control(&mut self, engine: &mut Runner, request: ControlRequest, reply: Reply) {
        let replay_control = matches!(
            request,
            ControlRequest::Sessions
                | ControlRequest::PlaybackUpdate(_)
                | ControlRequest::PlaybackStop
        );
        if self.replay.is_some() && !replay_control {
            let _ = reply.send(Err(error(
                ApiErrorCode::Conflict,
                "Stop playback before changing live state or recording.",
            )));
            return;
        }
        if self.busy || self.capture.is_some() {
            let _ = reply.send(Err(error(
                ApiErrorCode::Conflict,
                "Another operation is still running.",
            )));
            return;
        }
        let result = self.dispatch(engine, request, reply);
        if let Err((e, reply)) = result {
            let _ = reply.send(Err(e));
        }
    }
    fn dispatch(
        &mut self,
        engine: &mut Runner,
        request: ControlRequest,
        reply: Reply,
    ) -> Result<(), (ControlError, Reply)> {
        let dir = engine.config.config.engine.data_dir.clone();
        match request {
            ControlRequest::Assignment(id, segment) => {
                if !engine
                    .registry
                    .entries
                    .get(&id)
                    .is_some_and(|e| e.descriptor.kind == DeviceKind::ImuTracker)
                {
                    return Err((
                        error(ApiErrorCode::NotFound, "Orientation tracker was not found."),
                        reply,
                    ));
                }
                if segment
                    .as_ref()
                    .is_some_and(|s| engine.skeleton.index_of(s).is_none())
                {
                    return Err((
                        error(
                            ApiErrorCode::InvalidRequest,
                            "Choose a segment from the active skeleton.",
                        ),
                        reply,
                    ));
                }
                if segment.as_ref().is_some_and(|s| {
                    engine
                        .registry
                        .assignments
                        .iter()
                        .any(|(other, t)| other != &id && t == s)
                }) {
                    return Err((
                        error(
                            ApiErrorCode::Conflict,
                            "That segment already has an orientation tracker.",
                        ),
                        reply,
                    ));
                }
                if engine.registry.assignments.get(&id) == segment.as_ref() {
                    let data = device(engine, &id);
                    let _ = reply.send(Ok(data));
                    return Ok(());
                }
                let mut assignments = engine.registry.assignments.clone();
                match segment {
                    Some(s) => {
                        assignments.insert(id.clone(), s);
                    }
                    None => {
                        assignments.remove(&id);
                    }
                }
                let mut stale = engine.store.stale.clone();
                for p in engine.store.profiles.values().filter(|p| {
                    p.kind == CalibrationKind::Mounting && p.subject.device_id.as_ref() == Some(&id)
                }) {
                    stale.insert(p.id.clone());
                }
                let snapshot = engine.snapshot_with(&assignments, &stale, &engine.store.profiles);
                self.background(reply, move || {
                    save_runtime_snapshot(&dir, &snapshot).map_err(unavailable)?;
                    Ok(Completion::Assignment(assignments, stale, id))
                });
            }
            ControlRequest::Fault(id, request) => {
                if !engine.registry.entries.contains_key(&id) {
                    return Err((
                        error(ApiErrorCode::NotFound, "Simulated device was not found."),
                        reply,
                    ));
                }
                if request
                    .quality
                    .is_some_and(|q| !q.is_finite() || !(0.0..=1.0).contains(&q))
                {
                    return Err((
                        error(
                            ApiErrorCode::InvalidRequest,
                            "Device quality must be between 0 and 1.",
                        ),
                        reply,
                    ));
                }
                let mut setting = engine.faults.borrow().get(&id).copied().unwrap_or_default();
                setting.apply(request.action, request.quality);
                engine.faults.send_modify(|faults| {
                    faults.insert(id.clone(), setting);
                });
                if matches!(
                    request.action,
                    FaultAction::Disconnect | FaultAction::Reconnect
                ) {
                    self.accept(
                        engine,
                        SourceEvent::DeviceLink {
                            device_id: id.clone(),
                            connected: setting.connected,
                            t_us: engine.clock.now_us(),
                        },
                    );
                }
                if let Some(e) = engine.registry.entries.get_mut(&id) {
                    e.reported_quality = setting.quality;
                }
                self.publish_state(engine);
                let _ = reply.send(Ok(device(engine, &id)));
            }
            ControlRequest::Delete(id) => {
                if engine
                    .store
                    .profiles
                    .get(&id)
                    .is_some_and(|p| p.method == DECLARED_BY_SIMULATOR)
                {
                    return Err((
                        error(
                            ApiErrorCode::InvalidRequest,
                            "Simulated camera calibration is declared by the simulator and cannot be deleted.",
                        ),
                        reply,
                    ));
                }
                let in_profiles = engine.store.profiles.contains_key(&id);
                if !in_profiles && !engine.store.invalid.iter().any(|p| p.id == id) {
                    return Err((
                        error(ApiErrorCode::NotFound, "Calibration profile was not found."),
                        reply,
                    ));
                }
                let mut profiles = engine.store.profiles.clone();
                profiles.remove(&id);
                let mut stale = engine.store.stale.clone();
                stale.remove(&id);
                let snapshot =
                    engine.snapshot_with(&engine.registry.assignments, &stale, &profiles);
                self.background(reply, move || {
                    save_runtime_snapshot(&dir, &snapshot).map_err(unavailable)?;
                    if in_profiles {
                        if let Err(error) = remove_legacy_profile_file(&dir, &id) {
                            tracing::warn!(profile=%id, %error, "Runtime snapshot deleted profile, but legacy file could not be removed");
                        }
                    } else {
                        std::fs::remove_file(dir.join("calibration").join(format!("{id}.json")))
                            .map_err(unavailable)?;
                    }
                    Ok(Completion::Deleted(id))
                });
            }
            ControlRequest::Room(room) => {
                if room.as_deref().is_some_and(|r| !valid_room(r)) {
                    return Err((
                        error(
                            ApiErrorCode::InvalidRequest,
                            "A room name is lowercase letters, digits, _ and -, at most 32.",
                        ),
                        reply,
                    ));
                }
                if self.recorder.is_some() {
                    return Err((
                        error(
                            ApiErrorCode::Conflict,
                            "Stop recording before switching rooms: a recording keeps one camera rig.",
                        ),
                        reply,
                    ));
                }
                let mut snapshot = engine.snapshot_with(
                    &engine.registry.assignments,
                    &engine.store.stale,
                    &engine.store.profiles,
                );
                snapshot.active_room.clone_from(&room);
                self.background(reply, move || {
                    save_runtime_snapshot(&dir, &snapshot).map_err(unavailable)?;
                    Ok(Completion::Room(room))
                });
            }
            ControlRequest::Sessions => self.background(reply, move || {
                let (sessions, damaged) = reader::sessions(&dir).map_err(unavailable)?;
                Ok(Completion::Sessions(sessions, damaged))
            }),
            ControlRequest::RecordStart(request) => {
                if self.recorder.is_some() {
                    return Err((
                        error(ApiErrorCode::Conflict, "Recording is already running."),
                        reply,
                    ));
                }
                if request.name.as_ref().is_some_and(|name| name.len() > 160) {
                    return Err((
                        error(
                            ApiErrorCode::InvalidRequest,
                            "Session name must be at most 160 bytes.",
                        ),
                        reply,
                    ));
                }
                let context = Self::context(engine);
                self.background(reply, move || {
                    Recorder::start(dir, request.name, context)
                        .map(Completion::RecordingStarted)
                        .map_err(unavailable)
                });
            }
            ControlRequest::RecordTraining(id) => {
                if self.recorder.is_some() {
                    return Err((
                        error(ApiErrorCode::Conflict, "Recording is already running."),
                        reply,
                    ));
                }
                let context = Self::context(engine);
                let name = Some(format!("Training {id}"));
                self.background(reply, move || {
                    Recorder::start_as(dir, name, context, Some(id))
                        .map(Completion::RecordingStarted)
                        .map_err(unavailable)
                });
            }
            ControlRequest::RecordStopTraining(id) => {
                if self
                    .recorder
                    .as_ref()
                    .is_none_or(|r| r.state.session_id != id)
                {
                    return Err((
                        error(
                            ApiErrorCode::Conflict,
                            "This session's recording is not running.",
                        ),
                        reply,
                    ));
                }
                return self.dispatch(engine, ControlRequest::RecordStop, reply);
            }
            ControlRequest::RecordStop => {
                let Some(recorder) = self.recorder.take() else {
                    return Err((
                        error(ApiErrorCode::Conflict, "No recording is running."),
                        reply,
                    ));
                };
                let duration = engine
                    .clock
                    .now_us()
                    .saturating_sub(recorder.state.started_t_us);
                self.busy = true;
                let tx = self.commands.clone();
                tokio::spawn(async move {
                    let result = recorder
                        .finish(duration)
                        .await
                        .map(Completion::RecordingStopped)
                        .map_err(unavailable);
                    let _ = tx.send(Command::Complete(result, reply)).await;
                });
            }
            ControlRequest::PlaybackStart(request) => {
                if self.recorder.is_some() {
                    return Err((
                        error(
                            ApiErrorCode::Conflict,
                            "Stop recording before starting playback.",
                        ),
                        reply,
                    ));
                }
                let speed = request.speed.unwrap_or(1.0);
                if !valid_speed(speed) {
                    return Err((
                        error(
                            ApiErrorCode::InvalidRequest,
                            "Playback speed must be between 0.1 and 4.",
                        ),
                        reply,
                    ));
                }
                if !dir
                    .join("sessions")
                    .join(request.session_id.as_str())
                    .exists()
                {
                    return Err((
                        error(ApiErrorCode::NotFound, "Session was not found."),
                        reply,
                    ));
                }
                self.background(reply, move || {
                    reader::load(&dir, &request.session_id)
                        .map(|r| {
                            Completion::PlaybackLoaded(
                                Box::new(r),
                                speed,
                                request.looping.unwrap_or(false),
                            )
                        })
                        // Loader messages are user-facing, e.g. "too large for in-memory playback".
                        .map_err(|e| error(ApiErrorCode::Unavailable, e))
                });
            }
            ControlRequest::PlaybackUpdate(request) => {
                let Some(replay) = self.replay.as_mut() else {
                    return Err((
                        error(ApiErrorCode::Conflict, "No playback is running."),
                        reply,
                    ));
                };
                if request.speed.is_some_and(|s| !valid_speed(s))
                    || request
                        .position_us
                        .is_some_and(|p| p > replay.recording.session.duration_us)
                {
                    return Err((
                        error(
                            ApiErrorCode::InvalidRequest,
                            "Playback position or speed is outside the supported range.",
                        ),
                        reply,
                    ));
                }
                if let Some(position) = request.position_us {
                    if let Err(e) = replay.seek(position) {
                        return Err((unavailable(e), reply));
                    }
                    self.last_landmarks_3d_us = None;
                }
                if let Some(speed) = request.speed {
                    replay.speed = speed;
                }
                if let Some(paused) = request.paused {
                    replay.paused = paused;
                }
                let preview = request.position_us.map(|_| replay.preview());
                let data = json!(replay.state());
                self.publish_state(engine);
                if let Some(messages) = preview {
                    self.publish(messages);
                }
                let _ = reply.send(Ok(data));
            }
            ControlRequest::PlaybackStop => {
                if self.replay.take().is_none() {
                    return Err((
                        error(ApiErrorCode::Conflict, "No playback is running."),
                        reply,
                    ));
                }
                self.last_landmarks_3d_us = None;
                for control in &engine.controls {
                    control.send_modify(|c| c.suspended = false);
                }
                self.publish_state(engine);
                let _ = reply.send(Ok(json!({"mode":"live"})));
            }
        }
        Ok(())
    }
    fn background(
        &mut self,
        reply: Reply,
        work: impl FnOnce() -> Result<Completion, ControlError> + Send + 'static,
    ) {
        self.busy = true;
        let tx = self.commands.clone();
        tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(work)
                .await
                .map_err(unavailable)
                .and_then(|r| r);
            let _ = tx.send(Command::Complete(result, reply)).await;
        });
    }
    pub(super) fn complete(
        &mut self,
        engine: &mut Runner,
        result: Result<Completion, ControlError>,
        reply: Reply,
    ) {
        self.busy = false;
        let data = result.and_then(|result| {
            Ok(match result {
                Completion::Assignment(assignments, stale, id) => {
                    engine.placements.send_replace(assignments.clone());
                    engine.registry.assignments = assignments;
                    engine.store.stale = stale;
                    if engine.store.stale.is_empty() {
                        engine.store.warnings.clear();
                    }
                    self.record_context(engine);
                    device(engine, &id)
                }
                Completion::Room(room) => {
                    tracing::info!(room = ?room, "Active room selected");
                    engine.store.active_room.clone_from(&room);
                    json!({"active_room": room})
                }
                Completion::Deleted(id) => {
                    engine.store.profiles.remove(&id);
                    engine.store.stale.remove(&id);
                    if engine.store.stale.is_empty() {
                        engine.store.warnings.clear();
                    }
                    engine.store.invalid.retain(|p| p.id != id);
                    self.record_context(engine);
                    json!({"deleted":true})
                }
                Completion::RecordingStarted(recorder) => {
                    self.storage_unavailable = false;
                    let data = json!(recorder.state);
                    self.recorder = Some(recorder);
                    data
                }
                Completion::RecordingStopped(session) => json!(session),
                Completion::Sessions(sessions, mut damaged) => {
                    if let Some(recorder) = &self.recorder {
                        damaged.retain(|id| id != recorder.state.session_id.as_str());
                    }
                    self.reported_damage.retain(|id| damaged.contains(id));
                    for id in damaged {
                        if !self.reported_damage.insert(id.clone()) {
                            continue;
                        }
                        let _ = self.notices.send(super::encoded::Encoded::new(
                            &ServerMessage::Notice {
                                level: NoticeLevel::Warning,
                                code: "damaged_session".into(),
                                message: format!(
                                    "Session {id} is damaged or incomplete and cannot be replayed."
                                ),
                            },
                            0,
                        ));
                    }
                    json!(sessions)
                }
                Completion::PlaybackLoaded(recording, speed, looping) => {
                    let mut replay =
                        Replay::new(*recording, speed, looping).map_err(unavailable)?;
                    let data = json!(replay.state());
                    let messages = replay.advance(0).map_err(unavailable)?;
                    crate::sources::inspection::clear(&engine.registry.runtime);
                    self.replay = Some(replay);
                    self.last_landmarks_3d_us = None;
                    for control in &engine.controls {
                        control.send_modify(|c| c.suspended = true);
                    }
                    self.publish_state(engine);
                    self.publish(messages);
                    data
                }
            })
        });
        self.publish_state(engine);
        let _ = reply.send(data);
    }
}
fn valid_speed(speed: f32) -> bool {
    speed.is_finite() && (0.1..=4.0).contains(&speed)
}
fn device(engine: &Runner, id: &DeviceId) -> Value {
    json!(
        engine
            .registry
            .status(
                engine.clock.now_us(),
                &engine.config.config.quality,
                &engine.store
            )
            .into_iter()
            .find(|d| &d.descriptor.id == id)
    )
}
fn error(code: ApiErrorCode, message: impl Into<String>) -> ControlError {
    ControlError::new(code, message)
}
fn unavailable(e: impl std::fmt::Display) -> ControlError {
    tracing::warn!(error=%e,"Storage operation failed");
    error(
        ApiErrorCode::Unavailable,
        "The operation could not be completed. Check the data directory or session integrity.",
    )
}
