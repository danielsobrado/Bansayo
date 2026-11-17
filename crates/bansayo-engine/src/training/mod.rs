//! Training runner actor (martial-arts 14 §2.1, §3): hosts the pure `bansayo-training` session,
//! owns the training session, athletes and the analytics writer queue. It shares nothing mutable
//! with tracking: samples arrive over a bounded channel, state leaves over watch and broadcast.
pub mod athletes;

use crate::content_store::ContentStore;
use crate::pipeline::command::{Client, ControlError, TrainingContext, TrainingSample};
use crate::store::{SessionBegin, Write, Writer};
use athletes::{AthleteError, Athletes};
use bansayo_contracts::*;
use bansayo_training::metrics::{GradeContext, Qa};
use bansayo_training::observables::{Cutoffs, ObservableFrame};
use bansayo_training::recognize::RecognizerConfig;
use bansayo_training::segment::SegmenterConfig;
use bansayo_training::session::{Session, SessionConfig, SessionSetup};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio_util::sync::CancellationToken;

/// Athlete, session, content and storage facts merged into `SystemState`.
#[derive(Debug, Clone, PartialEq)]
pub struct TrainingStatus {
    pub athlete: Option<AthleteRef>,
    pub training: Option<TrainingRef>,
    pub content_rev: u64,
    pub storage: StorageStatus,
}

/// Latest observables of the session, for the `observables` topic.
#[derive(Debug, Clone)]
pub struct ObservablesSnapshot {
    pub seq: u64,
    pub t_us: u64,
    pub values: Vec<ObservableValue>,
}

#[derive(Clone)]
pub struct TrainingStreams {
    pub state: watch::Receiver<Option<Arc<TrainingState>>>,
    pub events: broadcast::Sender<Arc<(SessionId, TrainingEvent)>>,
    pub observables: watch::Receiver<Option<Arc<ObservablesSnapshot>>>,
    pub status: watch::Receiver<TrainingStatus>,
}

type Reply<T> = oneshot::Sender<Result<T, ControlError>>;

pub enum Request {
    CreateSession(CreateSessionRequest, Reply<TrainingState>),
    Command(TrainingCommandRequest, Reply<TrainingState>),
    /// Events of the running session (also held when nothing is stored).
    Events(SessionId, u64, u32, Reply<Option<Vec<TrainingEvent>>>),
    ListAthletes(Reply<Vec<AthleteProfile>>),
    GetAthlete(AthleteId, Reply<AthleteProfile>),
    CreateAthlete(CreateAthleteRequest, Reply<AthleteProfile>),
    UpdateAthlete(AthleteId, Box<AthleteProfile>, Reply<AthleteProfile>),
    SetActive(Option<AthleteId>, Reply<Option<AthleteRef>>),
    Consent(AthleteId, ConsentRequest, Reply<AthleteProfile>),
}

/// Capacity of the commands → training channel; full means `unavailable` (14 §3).
pub const COMMAND_CAPACITY: usize = 32;
/// Capacity of the `training_event` broadcast (14 §3).
pub const EVENT_CAPACITY: usize = 1024;

#[derive(Clone)]
pub struct TrainingClient {
    tx: mpsc::Sender<Request>,
}

fn unavailable() -> ControlError {
    ControlError::new(
        ApiErrorCode::Unavailable,
        "Training is busy or stopped; try again.",
    )
}

impl TrainingClient {
    pub async fn ask<T>(&self, make: impl FnOnce(Reply<T>) -> Request) -> Result<T, ControlError> {
        let (tx, rx) = oneshot::channel();
        self.tx.try_send(make(tx)).map_err(|_| unavailable())?;
        rx.await.map_err(|_| unavailable())?
    }
}

fn conflict(m: &str) -> ControlError {
    ControlError::new(ApiErrorCode::Conflict, m)
}

fn invalid(m: &str) -> ControlError {
    ControlError::new(ApiErrorCode::InvalidRequest, m)
}

fn athlete_error(e: AthleteError) -> ControlError {
    match e {
        AthleteError::Invalid(m) => invalid(&m),
        AthleteError::Exists => conflict("An athlete with this id already exists."),
        AthleteError::NotFound => {
            ControlError::new(ApiErrorCode::NotFound, "No athlete with this id.")
        }
        AthleteError::Io(_) => ControlError::new(
            ApiErrorCode::Unavailable,
            "Athlete profiles could not be written. Check the data directory.",
        ),
    }
}

/// Builds the pure session configuration from the engine configuration (13 §4).
pub fn session_config(c: &crate::config::Config) -> SessionConfig {
    let a = c.analysis();
    let r = c.recognition();
    let t = c.training();
    let s = r.segmenter;
    SessionConfig {
        cutoffs: Cutoffs {
            position_hz: a.filters.position_hz,
            angle_hz: a.filters.angle_hz,
        },
        recognizer: RecognizerConfig {
            segmenter: SegmenterConfig {
                start_above: s.start_above,
                start_hold_us: s.start_hold_ms * 1000,
                end_below: s.end_below,
                end_hold_us: s.end_hold_ms * 1000,
                min_us: s.min_ms * 1000,
                max_us: s.max_ms * 1000,
                merge_gap_us: s.merge_gap_ms * 1000,
            },
            accept_confidence: r.accept_confidence.guided,
            ambiguity_margin: r.ambiguity_margin,
            tempo_scale: 1.0,
        },
        countdown_ms: (t.countdown_s * 1000.0) as u64,
        pause_after_tracking_loss_ms: t.pause_after_tracking_loss_ms,
        strikes_per_session: t.limits.strikes_per_session,
        session_minutes: t.limits.session_minutes,
        cooldown_reps: c.coaching().cooldown_reps,
    }
}

/// Everything about one running (or just finished) session.
struct Current {
    session: Session,
    profile: AthleteProfile,
    stored: bool,
    log: Vec<TrainingEvent>,
    title: String,
    published_rev: u64,
    last_publish_us: u64,
}

pub struct Runner {
    pub config: crate::config::Config,
    pub content: Arc<ContentStore>,
    pub writer: Option<Writer>,
    pub athletes: Athletes,
    pub pipeline: Client,
    pub clock: crate::clock::SessionClock,
    pub samples: mpsc::Receiver<TrainingSample>,
    pub system: watch::Receiver<SystemState>,
}

struct Outputs {
    state: watch::Sender<Option<Arc<TrainingState>>>,
    events: broadcast::Sender<Arc<(SessionId, TrainingEvent)>>,
    observables: watch::Sender<Option<Arc<ObservablesSnapshot>>>,
    status: watch::Sender<TrainingStatus>,
}

struct Actor {
    r: Runner,
    out: Outputs,
    current: Option<Current>,
    active: Option<AthleteProfile>,
    spill: VecDeque<Write>,
    storage_reason: Option<Reason>,
}

impl Runner {
    /// Initial status for the channel the pipeline reads before training starts.
    pub fn initial_status(content_rev: u64, store_ok: bool) -> TrainingStatus {
        TrainingStatus {
            athlete: None,
            training: None,
            content_rev,
            storage: StorageStatus {
                state: if store_ok {
                    StorageStateKind::Ok
                } else {
                    StorageStateKind::Unavailable
                },
                reasons: vec![],
            },
        }
    }

    pub fn spawn(
        self,
        status: watch::Sender<TrainingStatus>,
        cancel: CancellationToken,
    ) -> (TrainingClient, TrainingStreams, tokio::task::JoinHandle<()>) {
        let (tx, rx) = mpsc::channel(COMMAND_CAPACITY);
        let (state, state_rx) = watch::channel(None);
        let (events, _) = broadcast::channel(EVENT_CAPACITY);
        let (observables, observables_rx) = watch::channel(None);
        let active = self
            .athletes
            .active()
            .and_then(|id| self.athletes.get(&id).ok());
        let storage_reason = self.writer.is_none().then(|| {
            evidence_reason(
                "store_unavailable",
                "The analytics store could not be opened; sessions cannot be started.",
            )
        });
        let initial = TrainingStatus {
            athlete: active.as_ref().map(|p| AthleteRef {
                id: p.id.clone(),
                display_name: p.display_name.clone(),
            }),
            training: None,
            content_rev: self.content.rev,
            storage: StorageStatus {
                state: if storage_reason.is_some() {
                    StorageStateKind::Unavailable
                } else {
                    StorageStateKind::Ok
                },
                reasons: storage_reason.clone().into_iter().collect(),
            },
        };
        status.send_replace(initial);
        let status_rx = status.subscribe();
        let streams = TrainingStreams {
            state: state_rx,
            events: events.clone(),
            observables: observables_rx,
            status: status_rx,
        };
        let actor = Actor {
            r: self,
            out: Outputs {
                state,
                events,
                observables,
                status,
            },
            current: None,
            active,
            spill: VecDeque::new(),
            storage_reason,
        };
        let task = tokio::spawn(actor.run(rx, cancel));
        (TrainingClient { tx }, streams, task)
    }
}

fn evidence_reason(code: &str, message: &str) -> Reason {
    Reason {
        code: code.into(),
        subject: None,
        message: message.into(),
    }
}

impl Actor {
    async fn run(mut self, mut requests: mpsc::Receiver<Request>, cancel: CancellationToken) {
        let mut second = tokio::time::interval(std::time::Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                r = requests.recv() => match r {
                    Some(r) => self.request(r).await,
                    None => break,
                },
                s = self.r.samples.recv() => match s {
                    Some((sample, gap)) => self.sample(&sample, gap),
                    None => break,
                },
                _ = second.tick() => {
                    self.drain();
                    self.publish(true);
                }
            }
        }
        self.finish_storage();
    }

    fn now(&self) -> u64 {
        self.r.clock.now_us()
    }

    fn status(&mut self) {
        let next = TrainingStatus {
            athlete: self.active.as_ref().map(|p| AthleteRef {
                id: p.id.clone(),
                display_name: p.display_name.clone(),
            }),
            training: self.current.as_ref().map(|c| {
                let s = c.session.state();
                TrainingRef {
                    session_id: s.session_id,
                    phase: s.phase,
                    title: c.title.clone(),
                }
            }),
            content_rev: self.r.content.rev,
            storage: StorageStatus {
                state: if self.r.writer.is_none() {
                    StorageStateKind::Unavailable
                } else if self.spill.is_empty() {
                    StorageStateKind::Ok
                } else {
                    StorageStateKind::Degraded
                },
                reasons: self.storage_reason.clone().into_iter().collect(),
            },
        };
        self.out.status.send_if_modified(|s| {
            let changed = *s != next;
            *s = next;
            changed
        });
    }

    /// Publishes `training_state` on change and at 1 Hz while a session exists (14 §4).
    fn publish(&mut self, heartbeat: bool) {
        let now = self.now();
        let storage = !self.spill.is_empty();
        if let Some(c) = self.current.as_mut() {
            let mut s = c.session.state();
            if storage {
                s.storage = StorageHealth::Degraded;
            }
            let due = heartbeat
                || s.rev != c.published_rev
                || now.saturating_sub(c.last_publish_us) >= 1_000_000;
            if due {
                c.published_rev = s.rev;
                c.last_publish_us = now;
                self.out.state.send_replace(Some(Arc::new(s)));
            }
        }
        self.status();
    }

    fn emit(&mut self, events: Vec<TrainingEvent>) {
        let Some(c) = self.current.as_mut() else {
            return;
        };
        let id = c.session.session_id().clone();
        for e in events {
            if e.event_type == "command"
                && e.data["command"] == "ack_safety"
                && let Some(refs) = e.data["args"]["content_refs"].as_array()
            {
                let refs: Vec<String> = refs
                    .iter()
                    .filter_map(|r| r.as_str().map(String::from))
                    .collect();
                match self.r.athletes.acknowledge(
                    &c.profile.id,
                    &refs,
                    &crate::clock::utc_now_rfc3339(),
                ) {
                    Ok(p) => c.profile = p,
                    Err(err) => tracing::warn!(%err, "safety acknowledgement could not be saved"),
                }
            }
            c.log.push(e.clone());
            if c.stored {
                let w = Write::Event(id.clone(), Box::new(e.clone()), c.title.clone());
                if !self.spill.is_empty() {
                    self.spill.push_back(w);
                } else if let Some(writer) = &self.r.writer
                    && let Err(w) = writer.try_write(w)
                {
                    // Never dropped: held until the queue drains (14 §3, 12 §4).
                    self.spill.push_back(w);
                }
            }
            let _ = self.out.events.send(Arc::new((id.clone(), e)));
        }
    }

    fn drain(&mut self) {
        let Some(writer) = &self.r.writer else { return };
        while let Some(w) = self.spill.pop_front() {
            if let Err(w) = writer.try_write(w) {
                self.spill.push_front(w);
                break;
            }
        }
    }

    /// At shutdown, flush what is left or dump it for recovery (12 §9).
    fn finish_storage(&mut self) {
        let Some(writer) = self.r.writer.clone() else {
            return;
        };
        let pending: Vec<Write> = self.spill.drain(..).collect();
        let mut dump = vec![];
        for w in pending {
            if let Write::Event(id, e, _) = &w {
                dump.push((id.clone(), e.clone()));
            }
            if !writer.send(w) {
                break;
            }
        }
        if !dump.is_empty() {
            tracing::warn!(events = dump.len(), "analytics queue flushed at shutdown");
        }
    }

    fn sample(&mut self, sample: &MotionSample, gap: u64) {
        let Some(c) = self.current.as_mut() else {
            return;
        };
        if c.session.finished() {
            return;
        }
        let mut events = vec![];
        if gap > 0 {
            // Dropped samples are a tracking gap of that length (14 §3).
            let hole = MotionSample {
                seq: sample.seq,
                t_us: sample.t_us.saturating_sub(gap),
                origins: vec![],
                body: None,
                planar: vec![],
            };
            events.extend(c.session.sample(&hole).events);
        }
        if let Some(report) = self.r.system.borrow().capability_report.clone() {
            c.session.capability(&report);
        }
        let out = c.session.sample(sample);
        events.extend(out.events);
        events.extend(c.session.tick(sample.t_us));
        if let Some(frame) = out.frame {
            self.publish_observables(sample.seq, frame);
        }
        self.emit(events);
        self.publish(false);
    }

    fn publish_observables(&mut self, seq: u64, frame: ObservableFrame) {
        self.out
            .observables
            .send_replace(Some(Arc::new(ObservablesSnapshot {
                seq,
                t_us: frame.t_us,
                values: frame.values,
            })));
    }

    async fn request(&mut self, r: Request) {
        match r {
            Request::CreateSession(req, reply) => {
                let res = self.create(req).await;
                let _ = reply.send(res);
            }
            Request::Command(req, reply) => {
                let res = self.command(req);
                let _ = reply.send(res);
            }
            Request::Events(id, after, limit, reply) => {
                let found = self
                    .current
                    .as_ref()
                    .filter(|c| c.session.session_id() == &id)
                    .map(|c| {
                        c.log
                            .iter()
                            .filter(|e| e.event_seq > after)
                            .take(limit as usize)
                            .cloned()
                            .collect()
                    });
                let _ = reply.send(Ok(found));
            }
            Request::ListAthletes(reply) => {
                let _ = reply.send(Ok(self.r.athletes.list()));
            }
            Request::GetAthlete(id, reply) => {
                let _ = reply.send(self.r.athletes.get(&id).map_err(athlete_error));
            }
            Request::CreateAthlete(req, reply) => {
                let rec = self.r.config.storage().record_defaults;
                let flags = RecordingFlags {
                    inputs: rec.inputs,
                    motion: rec.motion,
                    video: rec.video,
                };
                let _ = reply.send(
                    self.r
                        .athletes
                        .create(req, &crate::clock::utc_now_rfc3339(), flags)
                        .map_err(athlete_error),
                );
            }
            Request::UpdateAthlete(id, p, reply) => {
                let res = self.r.athletes.update(&id, *p).map_err(athlete_error);
                if let Ok(p) = &res
                    && self.active.as_ref().is_some_and(|a| a.id == p.id)
                {
                    self.active = Some(p.clone());
                }
                self.status();
                let _ = reply.send(res);
            }
            Request::SetActive(id, reply) => {
                let res = if self.running() {
                    Err(conflict(
                        "The active athlete cannot change during a session.",
                    ))
                } else {
                    self.r
                        .athletes
                        .set_active(id.as_ref())
                        .map_err(athlete_error)
                        .map(|()| {
                            self.active = id.as_ref().and_then(|id| self.r.athletes.get(id).ok());
                            self.active.as_ref().map(|p| AthleteRef {
                                id: p.id.clone(),
                                display_name: p.display_name.clone(),
                            })
                        })
                };
                self.status();
                let _ = reply.send(res);
            }
            Request::Consent(id, req, reply) => {
                let res = self
                    .r
                    .athletes
                    .consent(&id, &req, &crate::clock::utc_now_rfc3339())
                    .map_err(athlete_error);
                if let Ok(p) = &res
                    && self.active.as_ref().is_some_and(|a| a.id == p.id)
                {
                    self.active = Some(p.clone());
                }
                let _ = reply.send(res);
            }
        }
    }

    fn running(&self) -> bool {
        self.current.as_ref().is_some_and(|c| !c.session.finished())
    }

    fn command(&mut self, req: TrainingCommandRequest) -> Result<TrainingState, ControlError> {
        let now = self.now();
        let Some(c) = self.current.as_mut() else {
            return Err(conflict("No training session exists."));
        };
        let args = req.args.unwrap_or_default();
        let ev = c
            .session
            .command(now, req.command, &args)
            .map_err(|r| conflict(&r.reason.message))?;
        self.emit(ev);
        self.publish(false);
        self.current
            .as_ref()
            .map(|c| c.session.state())
            .ok_or_else(unavailable)
    }

    async fn create(&mut self, req: CreateSessionRequest) -> Result<TrainingState, ControlError> {
        if self.running() {
            return Err(conflict("A training session is already running."));
        }
        let Some(profile) = self.active.clone() else {
            return Err(conflict("Select an athlete before starting a session."));
        };
        let Some(writer) = self.r.writer.clone() else {
            return Err(ControlError::new(
                ApiErrorCode::Unavailable,
                "The analytics store is unavailable; sessions cannot be started.",
            ));
        };
        let ctx: TrainingContext = self.r.pipeline.training_context().await?;
        if ctx.replay {
            return Err(conflict(
                "Training sessions cannot be created during playback.",
            ));
        }
        let Plan::Technique(plan) = &req.plan;
        let lib = &self.r.content.library;
        let Some(def) = lib.techniques.get(&plan.technique) else {
            return Err(ControlError {
                code: ApiErrorCode::InvalidRequest,
                message: format!("Unknown or invalid technique \"{}\".", plan.technique),
            });
        };
        let consent = profile.consent.motion_data_utc.is_some();
        let record = req.record.unwrap_or(profile.tracking.record_defaults);
        let side = match plan.side {
            PlanSide::Left => "left",
            _ => "right",
        };
        let personal = if consent {
            writer
                .read()
                .ok()
                .and_then(|c| {
                    crate::store::read::personal_targets(&c, profile.id.as_str(), &def.id, side)
                        .ok()
                })
                .unwrap_or_default()
        } else {
            Default::default()
        };
        let session_id = new_session_id();
        let setup = SessionSetup {
            session_id: session_id.clone(),
            athlete_id: profile.id.clone(),
            stance: profile.default_stance,
            coaching: profile.coaching.level,
            tolerance_scale: profile.coaching.tolerance_scale,
            safety_acked: profile.safety_ack.keys().cloned().collect(),
            skeleton: ctx.skeleton.clone(),
            library: lib.clone(),
            plan: plan.clone(),
            grade: GradeContext {
                calibration: ctx.calibration_states.clone(),
                qa: Qa::NotRun,
                origin_live: ctx.origin == Origin::Live,
                detector_id: ctx.detector_id.clone(),
                proportions_valid: ctx.skeleton.proportions == CalibrationState::Valid,
                max_gap_us: u64::from(def.max_gap_ms.unwrap_or(80)) * 1000,
            },
            personal,
            recording: RecordingFlags {
                inputs: consent && record.inputs,
                motion: consent && record.motion,
                video: false,
            },
            capability_profile: ctx.capability.profile,
            tracking_config_id: ctx.tracking.0.clone(),
        };
        let (session, events) = Session::new(session_config(&self.r.config), setup, self.now())
            .map_err(|problems| ControlError {
                code: ApiErrorCode::InvalidRequest,
                message: problems
                    .first()
                    .map_or("The plan is not valid.".into(), |p| {
                        format!("{}: {}", p.path, p.message)
                    }),
            })?;
        if consent {
            // A session does not start unless its provenance is written (ADR-036).
            let begin = SessionBegin {
                session_id: session_id.clone(),
                athlete_id: profile.id.clone(),
                started_utc: crate::clock::utc_now_rfc3339(),
                plan: req.plan.clone(),
                plan_ref: None,
                origin: ctx.origin,
                capability_profile: ctx.capability.profile,
                tracking_config: ctx.tracking.clone(),
                calibration_set: ctx.calibration.clone(),
                snapshots: self
                    .r
                    .content
                    .session_snapshots(&def.id, plan.scoring.as_deref()),
                has_inputs: false,
                has_motion: false,
                reanalysis_of: None,
                analysis_rev: 1,
                source_mask: None,
            };
            let (tx, rx) = oneshot::channel();
            let sent = writer.try_write(Write::Begin(Box::new(begin), tx)).is_ok();
            let written = if sent { rx.await.ok() } else { None };
            if !matches!(written, Some(Ok(()))) {
                return Err(ControlError::new(
                    ApiErrorCode::Unavailable,
                    "The session could not be recorded, so it was not started. Check the data directory.",
                ));
            }
        }
        let title = def.names.display.clone();
        self.current = Some(Current {
            session,
            profile,
            stored: consent,
            log: vec![],
            title,
            published_rev: 0,
            last_publish_us: 0,
        });
        self.emit(events);
        self.publish(true);
        let state = self
            .current
            .as_ref()
            .map(|c| c.session.state())
            .ok_or_else(unavailable)?;
        if !consent {
            tracing::info!(session=%session_id, "session runs without storage: no motion-data consent");
        }
        Ok(state)
    }
}

/// `YYYYMMDDTHHMMSSZ-xxxx`, as the recorder makes them.
pub fn new_session_id() -> SessionId {
    static IDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let stamp = crate::clock::utc_now_rfc3339().replace(['-', ':'], "");
    let mut n = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.subsec_nanos()) as u64
        + IDS.fetch_add(7919, std::sync::atomic::Ordering::Relaxed))
        % 36_u64.pow(4);
    let mut suffix = String::new();
    for _ in 0..4 {
        suffix.push(char::from_digit((n % 36) as u32, 36).unwrap_or('0'));
        n /= 36;
    }
    SessionId::new(format!("{stamp}-{suffix}"))
        .unwrap_or_else(|_| SessionId::new("19700101T000000Z-0000").expect("literal session id"))
}
