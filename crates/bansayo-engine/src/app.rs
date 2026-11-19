use crate::{
    api::{self, Api},
    clock::SessionClock,
    config::Validated,
    pipeline::Runner,
    registry::Registry,
    skeleton::Skeleton,
    sources::{
        DropCounters, SourceContext,
        camera::{CameraSource, Previews},
        simulated::{Control, SimulatedContext, SimulatedSource},
    },
};
use std::{
    net::SocketAddr,
    sync::{Arc, atomic::AtomicU64},
};
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
pub type EngineError = Box<dyn std::error::Error + Send + Sync>;
pub struct EngineHandle {
    address: SocketAddr,
    cancel: CancellationToken,
    tasks: Vec<JoinHandle<Result<(), EngineError>>>,
}
impl EngineHandle {
    pub fn bound_addr(&self) -> SocketAddr {
        self.address
    }
    pub async fn shutdown(mut self) -> Result<(), EngineError> {
        self.cancel.cancel();
        for task in self.tasks.drain(..) {
            task.await??;
        }
        Ok(())
    }
}
impl Drop for EngineHandle {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
pub async fn start(config: Validated) -> Result<EngineHandle, EngineError> {
    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    let address = listener.local_addr()?;
    let skeleton = Skeleton::builtin(
        &config.config.skeleton.model,
        config.config.skeleton.height_m,
    )?;
    let dir = config.config.engine.data_dir.clone();
    let initial_assignments = config.assignments.clone();
    let sk_clone = skeleton.clone();
    let (mut store, assignments) = tokio::task::spawn_blocking(move || {
        crate::registry::persistence::load_runtime(&dir, &initial_assignments, &sk_clone)
    })
    .await??;
    let clock = SessionClock::start();
    let cancel = CancellationToken::new();
    let (events_tx, events_rx) = mpsc::channel(2048);
    let (faults_tx, faults_rx) = watch::channel(std::collections::BTreeMap::new());
    let mut sources = vec![];
    let mut descriptors = vec![];
    let mut controls = vec![];
    let mut drops = DropCounters::new();
    let (placements, placements_rx) = watch::channel(assignments.clone());
    for spec in &config.simulated {
        let source = SimulatedSource {
            spec: spec.clone(),
            skeleton: skeleton.clone(),
            assignments: placements_rx.clone(),
        };
        for d in source.descriptors() {
            drops.insert(d.id.clone(), Arc::new(AtomicU64::new(0)));
            descriptors.push(d);
        }
        // Declared in memory at every start, never written to disk.
        let created = crate::clock::utc_now_rfc3339();
        for c in &spec.cameras {
            store.profiles.extend(
                crate::sources::simulated::camera::profiles(c, &created)
                    .into_iter()
                    .map(|p| (p.id.clone(), p)),
            );
        }
        let (control, receiver) = watch::channel(Control {
            motion: spec.motion,
            speed: spec.speed,
            paused: false,
            generation: 0,
            suspended: false,
        });
        controls.push(control);
        sources.push(ConfiguredSource::Simulated(Box::new(source), receiver));
    }
    let timing = crate::diagnostics::Timing::new(
        config.config.diagnostics().timing,
        (1e6 / config.config.engine.publish_rate_hz) as u64,
    );
    let previews = Previews::default();
    let fiducials = crate::sources::camera::FiducialPreviews::default();
    let camera_settings = crate::sources::camera::CameraSettings::default();
    let runtime = crate::sources::RuntimeMap::default();
    let feeds: crate::sources::camera::capture::NetworkFeeds = std::sync::Arc::new(
        config
            .cameras
            .iter()
            .flat_map(|s| &s.cameras)
            .filter_map(|c| match &c.backend {
                crate::sources::camera::capture::Backend::Network(id) => {
                    Some((id.clone(), std::sync::Arc::default()))
                }
                _ => None,
            })
            .collect(),
    );
    for spec in &config.cameras {
        let source = CameraSource {
            spec: spec.clone(),
            previews: previews.clone(),
            fiducials: fiducials.clone(),
            settings: camera_settings.clone(),
            timing: timing.clone(),
            feeds: feeds.clone(),
            data_dir: config.config.engine.data_dir.clone(),
            runtime: runtime.clone(),
            management: config.config.camera_management.clone(),
        };
        for d in source.descriptors() {
            drops.insert(d.id.clone(), Arc::new(AtomicU64::new(0)));
            descriptors.push(d);
        }
        sources.push(ConfiguredSource::Camera(Box::new(source)));
    }
    for spec in &config.slimevr {
        let source = crate::sources::slimevr::SlimevrSource {
            spec: spec.clone(),
            runtime: runtime.clone(),
        };
        for d in source.descriptors() {
            drops.insert(d.id.clone(), Arc::new(AtomicU64::new(0)));
            descriptors.push(d);
        }
        sources.push(ConfiguredSource::Slimevr(Box::new(source)));
    }
    let mut registry = Registry::new(descriptors, assignments, drops.clone());
    registry.runtime = runtime.clone();
    let data_dir = config.config.engine.data_dir.clone();
    let content_dirs = config.config.content().dirs;
    let user_dir = data_dir.join("content/user");
    let content = Arc::new(
        tokio::task::spawn_blocking(move || {
            crate::content_store::ContentStore::load(&content_dirs, &user_dir, 1)
        })
        .await?,
    );
    for e in content.index.iter().filter(|e| !e.valid) {
        tracing::warn!(kind=%e.kind, id=%e.id, problems=?e.problems, "content definition is invalid and not offered");
    }
    let db = data_dir.join("db/bansayo.sqlite");
    let writer = match tokio::task::spawn_blocking(move || crate::store::Writer::spawn(db)).await? {
        Ok(w) => Some(w),
        Err(e) => {
            tracing::error!(%e, "analytics store unavailable; training sessions cannot be started");
            None
        }
    };
    let (status_tx, status_rx) = watch::channel(crate::training::Runner::initial_status(
        content.rev,
        writer.is_some(),
    ));
    let (samples_tx, samples_rx) = mpsc::channel(crate::pipeline::command::TRAINING_CAPACITY);
    let runner = Runner {
        training: Some(samples_tx),
        training_status: Some(status_rx),
        config: config.clone(),
        registry,
        store,
        skeleton,
        clock: clock.clone(),
        events: events_rx,
        controls,
        faults: faults_tx,
        placements,
        timing: timing.clone(),
    };
    let (client, streams, runner_task) = runner.spawn(cancel.clone());
    let training = crate::training::Runner {
        config: config.config.clone(),
        content: content.clone(),
        writer: writer.clone(),
        athletes: crate::training::athletes::Athletes::new(&data_dir),
        pipeline: client.clone(),
        clock: clock.clone(),
        samples: samples_rx,
        system: streams.state.clone(),
        timing: timing.clone(),
    };
    let (training_client, training_streams, training_task) =
        training.spawn(status_tx, cancel.clone());
    let mut tasks: Vec<JoinHandle<Result<(), EngineError>>> = vec![
        tokio::spawn(async move {
            runner_task.await?;
            Ok(())
        }),
        tokio::spawn(async move {
            training_task.await?;
            Ok(())
        }),
    ];
    let diagnostics = spawn_diagnostics(
        &mut tasks,
        timing,
        streams.notices.clone(),
        drops.clone(),
        clock.clone(),
        cancel.clone(),
    );
    for source in sources {
        let common = SourceContext {
            events: events_tx.clone(),
            clock: clock.clone(),
            cancel: cancel.clone(),
            dropped: drops.clone(),
        };
        tasks.push(source.spawn(common, faults_rx.clone()));
    }
    let lan_token = if config.config.api.lan.enabled {
        let path = crate::lan::token_path(&config.config);
        let token =
            tokio::task::spawn_blocking(move || crate::lan::load_or_create(&path)).await??;
        tracing::warn!(
            "LAN mode: other machines need the pairing token (desktop, Devices); no TLS"
        );
        Some(Arc::<str>::from(token))
    } else {
        None
    };
    let api = Api {
        client,
        streams,
        allowed_origins: Arc::new(config.config.api.allowed_origins.clone()),
        publish_rate: config.config.engine.publish_rate_hz,
        cancel: cancel.clone(),
        previews,
        fiducials,
        camera_settings,
        runtime: runtime.clone(),
        diagnostics,
        lan_token,
        bind: address,
        feeds,
        training: training_client,
        training_streams,
        content,
        store: writer,
        data_dir: data_dir.clone(),
        progress: config.config.progress(),
    };
    let router = api::router(api);
    let stop = cancel.clone();
    tasks.push(tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(stop.cancelled_owned())
        .await?;
        Ok(())
    }));
    for warning in &config.warnings {
        tracing::warn!(path=%warning.path,message=%warning.message,"configuration warning");
    }
    tracing::info!(%address,"Bansayo engine listening");
    Ok(EngineHandle {
        address,
        cancel,
        tasks,
    })
}
/// Publishes stage timings every 10 s on the `diagnostics` topic, raises a notice when a stage
/// stays over budget for 5 s, and logs the last window at shutdown (15 §4.1).
fn spawn_diagnostics(
    tasks: &mut Vec<JoinHandle<Result<(), EngineError>>>,
    timing: crate::diagnostics::Timings,
    notices: tokio::sync::broadcast::Sender<Arc<crate::pipeline::encoded::Encoded>>,
    drops: DropCounters,
    clock: SessionClock,
    cancel: CancellationToken,
) -> watch::Receiver<Option<Arc<crate::pipeline::encoded::Encoded>>> {
    use crate::pipeline::encoded::Encoded;
    use bansayo_contracts::{DropCount, NoticeLevel, ServerMessage};
    let (tx, rx) = watch::channel(None);
    if !timing.on() {
        return rx;
    }
    tasks.push(tokio::spawn(async move {
        let dropped = || -> Vec<DropCount> {
            drops
                .iter()
                .map(|(id, c)| DropCount {
                    channel: format!("camera_frames:{id}"),
                    count: c.load(std::sync::atomic::Ordering::Relaxed),
                })
                .collect()
        };
        let mut second = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut ticks = 0u64;
        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = second.tick() => {}
            }
            let now = clock.now_us();
            for s in timing.newly_over_budget(now) {
                let message = format!(
                    "Stage {} has run over its budget for 5 s: p95 {:.1} ms, budget {:.1} ms.",
                    s.stage,
                    s.p95_us as f64 / 1000.0,
                    s.budget_us as f64 / 1000.0
                );
                tracing::warn!(stage = %s.stage, p95_us = s.p95_us, budget_us = s.budget_us, "stage over budget");
                let _ = notices.send(Encoded::new(
                    &ServerMessage::Notice {
                        level: NoticeLevel::Warning,
                        code: "stage_over_budget".into(),
                        message,
                    },
                    0,
                ));
            }
            ticks += 1;
            if ticks.is_multiple_of(10) {
                let message = ServerMessage::Diagnostics {
                    window_s: 10,
                    stages: timing.summary(now),
                    dropped: dropped(),
                };
                tx.send_replace(Some(Encoded::new(&message, ticks)));
            }
        }
        for s in timing.summary(clock.now_us()) {
            tracing::info!(stage = %s.stage, p50_us = s.p50_us, p95_us = s.p95_us, p99_us = s.p99_us, max_us = s.max_us, count = s.count, budget_us = s.budget_us, "stage timing over the last 10 s");
        }
        for d in dropped() {
            tracing::info!(channel = %d.channel, count = d.count, "dropped");
        }
        Ok(())
    }));
    rx
}

pub async fn run(
    config: Validated,
    shutdown: impl std::future::Future<Output = ()>,
) -> Result<(), EngineError> {
    let engine = start(config).await?;
    shutdown.await;
    engine.shutdown().await
}

/// Closed source kinds use static dispatch and one spawning match; no trait objects.
enum ConfiguredSource {
    Simulated(Box<SimulatedSource>, watch::Receiver<Control>),
    Camera(Box<CameraSource>),
    Slimevr(Box<crate::sources::slimevr::SlimevrSource>),
}
impl ConfiguredSource {
    fn spawn(
        self,
        common: SourceContext,
        faults: watch::Receiver<
            std::collections::BTreeMap<
                bansayo_contracts::DeviceId,
                crate::sources::simulated::FaultSetting,
            >,
        >,
    ) -> JoinHandle<Result<(), EngineError>> {
        tokio::spawn(async move {
            match self {
                Self::Simulated(source, control) => {
                    let devices = source.descriptors().into_iter().map(|d| d.id).collect();
                    crate::sources::supervision::supervise(
                        *source,
                        devices,
                        SimulatedContext {
                            common,
                            control,
                            faults,
                        },
                    )
                    .await;
                }
                Self::Camera(source) => {
                    let devices = source.descriptors().into_iter().map(|d| d.id).collect();
                    crate::sources::supervision::supervise(*source, devices, common).await;
                }
                Self::Slimevr(source) => {
                    let devices = source.descriptors().into_iter().map(|d| d.id).collect();
                    crate::sources::supervision::supervise(*source, devices, common).await;
                }
            }
            Ok(())
        })
    }
}
