use super::encoded::Encoded;
use super::{
    Runner,
    capture::{Capture, profiles},
    command::{Command, ControlError},
    features::features,
    joints::measure,
    solve::solve,
};
use crate::{
    calibration,
    calibration::mounting::CAPTURE_WINDOW_US,
    math::rot_from_wire,
    recording::{Context, Item, Recorder, replay::Replay},
    registry::persistence::{remove_legacy_profile_file, save_runtime_snapshot},
};
use bansayo_contracts::*;
use std::{sync::Arc, time::Duration};
use tokio::sync::{broadcast, mpsc, watch};
use tokio_util::sync::CancellationToken;
pub(super) struct Runtime {
    pub commands: mpsc::Sender<Command>,
    pub state: watch::Sender<SystemState>,
    pub state_wire: watch::Sender<Arc<Encoded>>,
    pub frame: watch::Sender<Arc<Encoded>>,
    pub landmarks: broadcast::Sender<Arc<Encoded>>,
    pub capture: Option<Capture>,
    pub busy: bool,
    pub recorder: Option<Recorder>,
    pub replay: Option<Replay>,
    pub rev: u64,
    pub seq: u64,
    pub notices: broadcast::Sender<Arc<Encoded>>,
    pub skeleton: watch::Sender<SkeletonDefinition>,
    pub storage_unavailable: bool,
    pub reported_damage: std::collections::BTreeSet<String>,
    pub last_landmarks_3d_us: Option<u64>,
}
impl Runtime {
    pub fn new(
        commands: mpsc::Sender<Command>,
        state: watch::Sender<SystemState>,
        frame: watch::Sender<Arc<Encoded>>,
        landmarks: broadcast::Sender<Arc<Encoded>>,
        notices: broadcast::Sender<Arc<Encoded>>,
        skeleton: watch::Sender<SkeletonDefinition>,
        state_wire: watch::Sender<Arc<Encoded>>,
    ) -> Self {
        Self {
            commands,
            state_wire,
            state,
            frame,
            landmarks,
            capture: None,
            busy: false,
            recorder: None,
            replay: None,
            rev: 1,
            seq: 0,
            notices,
            skeleton,
            storage_unavailable: false,
            reported_damage: Default::default(),
            last_landmarks_3d_us: None,
        }
    }
    pub fn context(engine: &Runner) -> Context {
        Context::snapshot(
            &engine.registry,
            &engine.store,
            &engine.skeleton,
            &engine.config.config,
            engine.clock.now_us(),
        )
    }
    pub fn record_context(&mut self, engine: &Runner) {
        if let Some(r) = self.recorder.as_mut() {
            r.push(Item::Context {
                accepted_us: engine.clock.now_us(),
                context: Box::new(Self::context(engine)),
            });
        }
    }
    pub fn publish_state(&mut self, engine: &Runner) {
        let mut active = self.replay.as_ref().map_or_else(
            || engine.skeleton.definition.clone(),
            |r| r.skeleton.definition.clone(),
        );
        let current = self.skeleton.borrow().clone();
        active.rev = current.rev;
        if active != current {
            active.rev += 1;
            self.skeleton.send_replace(active);
        }
        self.rev += 1;
        let state = self.current_state(engine);
        self.state_wire.send_replace(Encoded::new(
            &ServerMessage::State {
                state: state.clone(),
            },
            state.rev,
        ));
        self.state.send_replace(state);
    }
    pub fn current_state(&self, engine: &Runner) -> SystemState {
        let mut state = engine.state(self.rev);
        state.recording = self.recorder.as_ref().map(|r| r.state.clone());
        if let Some(replay) = &self.replay {
            let now = replay.recording.initial.started_t_us + replay.position_us;
            state.engine.mode = EngineMode::Replay;
            state.playback = Some(replay.state());
            state.simulation = None;
            state.devices =
                replay
                    .registry
                    .status(now, replay.recording.initial.quality(), &replay.store);
            state.calibration = replay.store.summaries();
            let solved = solve(
                &replay.skeleton,
                &replay.registry,
                &replay.store,
                now,
                &replay.recording.initial.config,
            );
            state.engine.state = solved.quality.state;
            state.features = features(&state.devices, &solved);
        }
        for feature in &mut state.features {
            if matches!(feature.id, FeatureId::Recording | FeatureId::Playback) {
                feature.state = if self.storage_unavailable {
                    FeatureState::Unavailable
                } else {
                    FeatureState::Available
                };
                feature.reasons = if self.storage_unavailable {
                    vec![crate::registry::reason(
                        "storage_unavailable",
                        None,
                        "Recording storage failed. Check the data directory before trying again.",
                    )]
                } else {
                    vec![]
                };
            }
        }
        state
    }
    pub(super) fn publish_frame(&mut self, mut message: ServerMessage) {
        if let ServerMessage::Frame {
            pose: Some(pose), ..
        } = &mut message
        {
            pose.skeleton_rev = self.skeleton.borrow().rev;
        }

        if let ServerMessage::Frame { seq, .. } = &mut message {
            self.seq += 1;
            *seq = self.seq;
        }
        self.frame.send_replace(Encoded::new(&message, self.seq));
    }
    /// Routes solver and replay output: triangulated landmarks once per camera frame set, replayed
    /// 2D landmarks in order, frames as latest.
    pub(super) fn publish(&mut self, messages: Vec<ServerMessage>) {
        for message in messages {
            if let ServerMessage::Landmarks3d {
                landmarks: PoseLandmarks3D::Triangulated { t_us, .. },
            } = &message
            {
                if self.last_landmarks_3d_us.replace(*t_us) != Some(*t_us) {
                    let _ = self.landmarks.send(Encoded::new(&message, 0));
                }
            } else if matches!(message, ServerMessage::Landmarks2d { .. }) {
                let _ = self.landmarks.send(Encoded::new(&message, 0));
            } else {
                self.publish_frame(message);
            }
        }
    }
    pub async fn run(
        mut self,
        mut engine: Runner,
        mut commands: mpsc::Receiver<Command>,
        cancel: CancellationToken,
    ) {
        let mut timer = crate::ticker::ticker(Duration::from_secs_f64(
            1.0 / engine.config.config.engine.publish_rate_hz,
        ));
        let mut last_tick = engine.clock.now_us();
        let mut last_state = 0;
        let mut events_open = true;
        loop {
            // An inline event avoids allocating every accepted source observation.
            #[allow(clippy::large_enum_variant)]
            enum Event {
                Input(Option<crate::sources::Ingest>),
                Command(Option<Command>),
                Tick,
            }
            let event = tokio::select! { _=cancel.cancelled()=>break,input=engine.events.recv(),if events_open=>Event::Input(input),command=commands.recv()=>Event::Command(command),_=timer.recv()=>Event::Tick };
            match event {
                Event::Input(Some(crate::sources::Ingest::Failure { devices, message })) => {
                    for id in devices {
                        if let Some(entry) = engine.registry.entries.get_mut(&id) {
                            entry.failure = Some(message.clone());
                            entry.connected = false;
                            entry.latest = None;
                            entry.recent.clear();
                            entry.first_us = None;
                        }
                    }
                    self.record_context(&engine);
                    self.publish_state(&engine);
                }
                Event::Input(Some(crate::sources::Ingest::Event(event))) => {
                    if self.replay.is_some() {
                        continue;
                    }
                    self.accept(&mut engine, event);
                }
                Event::Input(None) => events_open = false,
                Event::Command(Some(Command::Simulation(request, reply))) => {
                    let result = if self.replay.is_some() {
                        Err(ControlError::new(
                            ApiErrorCode::Conflict,
                            "Stop playback before changing simulation.",
                        ))
                    } else {
                        engine.update_simulation(request)
                    };
                    let _ = reply.send(result);
                    self.publish_state(&engine);
                }
                Event::Command(Some(Command::Capture(request, reply))) => {
                    if self.replay.is_some() {
                        let _ = reply.send(Err(ControlError::new(
                            ApiErrorCode::Conflict,
                            "Reference-pose capture is unavailable during playback.",
                        )));
                    } else if self.capture.is_some() || self.busy {
                        let _ = reply.send(Err(ControlError::new(
                            ApiErrorCode::Conflict,
                            "Another operation is still running.",
                        )));
                    } else {
                        match engine.targets(request) {
                            Ok(targets) => {
                                self.capture = Some(Capture {
                                    until: engine.clock.now_us() + CAPTURE_WINDOW_US,
                                    targets,
                                    reply,
                                })
                            }
                            Err(e) => {
                                let _ = reply.send(Err(e));
                            }
                        }
                    }
                }
                Event::Command(Some(Command::Saved(profiles, result, reply))) => {
                    self.busy = false;
                    match result {
                        Ok(()) => {
                            let summaries = profiles.iter().map(calibration::summary).collect();
                            engine.store.insert(profiles);
                            self.record_context(&engine);
                            self.publish_state(&engine);
                            let _ = reply.send(Ok(summaries));
                        }
                        Err(_) => {
                            let _ = reply.send(Err(ControlError::new(
                                ApiErrorCode::Unavailable,
                                "Could not save calibration. Check the data directory.",
                            )));
                        }
                    }
                }
                Event::Command(Some(Command::Control(request, reply))) => {
                    self.control(&mut engine, request, reply)
                }
                Event::Command(Some(Command::Complete(result, reply))) => {
                    self.complete(&mut engine, result, reply)
                }
                Event::Command(None) => break,
                Event::Tick => {
                    let now = engine.clock.now_us();
                    let elapsed = now.saturating_sub(last_tick);
                    last_tick = now;
                    let failed = self
                        .recorder
                        .as_mut()
                        .and_then(|r| match r.result.try_recv() {
                            Ok(Err(message)) => Some(message),
                            Ok(Ok(_)) => Some("Recorder stopped unexpectedly".into()),
                            Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                                Some("Recorder thread stopped unexpectedly".into())
                            }
                            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => None,
                        });
                    if let Some(message) = failed {
                        tracing::error!(%message,"Recording failed");
                        self.recorder.take();
                        self.storage_unavailable = true;
                        let _=self.notices.send(Encoded::new(&ServerMessage::Notice{level:NoticeLevel::Error,code:"recording_failed".into(),message:"Recording stopped because the file could not be written. Check the data directory.".into()},0));
                        self.publish_state(&engine);
                    }
                    self.finish_capture(&engine);
                    if let Some(replay) = self.replay.as_mut() {
                        match replay.advance(elapsed) {
                            Ok(messages) => self.publish(messages),
                            Err(message) => {
                                tracing::error!(%message,"Replay failed");
                            }
                        }
                    } else {
                        let solved = solve(
                            &engine.skeleton,
                            &engine.registry,
                            &engine.store,
                            now,
                            &engine.config.config,
                        );
                        let mut joints =
                            measure(&engine.skeleton, &solved.rotations, &solved.observed);
                        super::joints::confidence(
                            &engine.skeleton,
                            &mut joints,
                            solved.pose.as_ref(),
                        );
                        if let Some(landmarks) = solved.landmarks_3d {
                            self.publish(vec![ServerMessage::Landmarks3d { landmarks }]);
                        }
                        self.publish_frame(ServerMessage::Frame {
                            seq: 0,
                            t_us: now,
                            origins: if engine.config.simulated.is_empty() {
                                vec![]
                            } else {
                                vec![Origin::Simulated]
                            },
                            joints,
                            pose: solved.pose,
                            quality: solved.quality,
                        });
                    }
                    let next = self.current_state(&engine);
                    let changed = {
                        let old = self.state.borrow();
                        old.engine.state != next.engine.state
                            || old
                                .devices
                                .iter()
                                .map(|d| d.state)
                                .ne(next.devices.iter().map(|d| d.state))
                    };
                    if changed || now.saturating_sub(last_state) >= 1_000_000 {
                        self.publish_state(&engine);
                        last_state = now;
                    }
                }
            }
        }
        // Finish an in-flight storage operation before releasing a newly opened recorder or exiting.
        while self.busy {
            let Some(command) = commands.recv().await else {
                break;
            };
            match command {
                Command::Complete(result, reply) => self.complete(&mut engine, result, reply),
                Command::Saved(profiles, result, reply) => {
                    self.busy = false;
                    match result {
                        Ok(()) => {
                            let summaries = profiles.iter().map(calibration::summary).collect();
                            engine.store.insert(profiles);
                            self.record_context(&engine);
                            let _ = reply.send(Ok(summaries));
                        }
                        Err(_) => {
                            let _ = reply.send(Err(ControlError::new(
                                ApiErrorCode::Unavailable,
                                "Calibration could not be saved.",
                            )));
                        }
                    }
                }
                Command::Simulation(_, reply) => {
                    let _ = reply.send(Err(ControlError::new(
                        ApiErrorCode::Unavailable,
                        "Engine is shutting down.",
                    )));
                }
                Command::Capture(_, reply) => {
                    let _ = reply.send(Err(ControlError::new(
                        ApiErrorCode::Unavailable,
                        "Engine is shutting down.",
                    )));
                }
                Command::Control(_, reply) => {
                    let _ = reply.send(Err(ControlError::new(
                        ApiErrorCode::Unavailable,
                        "Engine is shutting down.",
                    )));
                }
            }
        }
        if let Some(c) = self.capture.take() {
            let _ = c.reply.send(Err(ControlError::new(
                ApiErrorCode::Unavailable,
                "Engine stopped during reference-pose capture.",
            )));
        }
        if let Some(recorder) = self.recorder.take() {
            let duration = engine
                .clock
                .now_us()
                .saturating_sub(recorder.state.started_t_us);
            if let Err(message) = recorder.finish(duration).await {
                tracing::error!(%message,"Recording finalisation failed");
            }
        }
    }
    pub(super) fn accept(&mut self, engine: &mut Runner, event: SourceEvent) {
        let accepted_us = engine.clock.now_us();
        if let SourceEvent::Observation { observation } = &event {
            let validation = engine
                .registry
                .entries
                .get(&observation.device_id)
                .ok_or_else(|| "Observation device is unknown.".to_owned())
                .and_then(|entry| {
                    crate::sources::validation::observation(
                        observation,
                        &entry.descriptor,
                        accepted_us,
                    )
                });
            if let Err(message) = validation {
                if let Some(entry) = engine.registry.entries.get_mut(&observation.device_id) {
                    entry.dropped += 1;
                }
                tracing::debug!(device=%observation.device_id,%message,"Rejected malformed observation");
                return;
            }
        }
        // Duplicate, reordered and late packets stop here: capture, clients and the recording see
        // exactly what the solver sees, so every recording replays.
        if !engine.registry.event(event.clone()) {
            return;
        }
        if let SourceEvent::Observation { observation } = &event {
            if let Some(c) = self.capture.as_mut()
                && let Some((_, samples)) = c.targets.get_mut(&observation.device_id)
                && let ObservationData::Orientation { q, .. } = observation.data
                && samples.len() < 2000
                && let Some(q) = rot_from_wire(q)
            {
                samples.push(q);
            }
            if let ObservationData::Landmarks2d { landmarks } = &observation.data {
                let _ = self.landmarks.send(Encoded::new(
                    &ServerMessage::Landmarks2d {
                        landmarks: landmarks.clone(),
                    },
                    0,
                ));
            }
        }
        if let Some(recorder) = self.recorder.as_mut() {
            recorder.push(Item::Event { accepted_us, event });
        }
    }
    fn finish_capture(&mut self, engine: &Runner) {
        if self
            .capture
            .as_ref()
            .is_none_or(|c| engine.clock.now_us() < c.until)
        {
            return;
        }
        let Some(c) = self.capture.take() else {
            return;
        };
        match profiles(c.targets) {
            Err(e) => {
                let _ = c.reply.send(Err(e));
            }
            Ok(profiles) => {
                self.busy = true;
                let dir = engine.config.config.engine.data_dir.clone();
                let mut stale = engine.store.stale.clone();
                let mut next_profiles = engine.store.profiles.clone();
                for p in &profiles {
                    stale.remove(&p.id);
                    next_profiles.insert(p.id.clone(), p.clone());
                }
                let snapshot =
                    engine.snapshot_with(&engine.registry.assignments, &stale, &next_profiles);
                let ids: Vec<ProfileId> = profiles.iter().map(|p| p.id.clone()).collect();
                let tx = self.commands.clone();
                tokio::spawn(async move {
                    let result = tokio::task::spawn_blocking(move || {
                        save_runtime_snapshot(&dir, &snapshot)?;
                        for id in &ids {
                            if let Err(error) = remove_legacy_profile_file(&dir, id) {
                                tracing::warn!(
                                    profile=%id,
                                    %error,
                                    "Calibration committed, but legacy file could not be removed"
                                );
                            }
                        }
                        Ok::<(), std::io::Error>(())
                    })
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r.map_err(|e| e.to_string()));
                    let _ = tx.send(Command::Saved(profiles, result, c.reply)).await;
                });
            }
        }
    }
}
