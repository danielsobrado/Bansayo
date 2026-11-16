use crate::{
    api::{self, Api},
    calibration::Store,
    clock::SessionClock,
    config::Validated,
    pipeline::Runner,
    registry::Registry,
    skeleton::Skeleton,
    sources::{
        DropCounters, SourceContext,
        simulated::{Control, SimulatedSource},
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
    let dir = config.config.engine.data_dir.clone();
    let store = tokio::task::spawn_blocking(move || Store::load(&dir)).await??;
    let skeleton = Skeleton::builtin(
        &config.config.skeleton.model,
        config.config.skeleton.height_m,
    )?;
    let clock = SessionClock::start();
    let cancel = CancellationToken::new();
    let (events_tx, events_rx) = mpsc::channel(2048);
    let (faults_tx, faults_rx) = watch::channel(std::collections::BTreeMap::new());
    let mut sources = vec![];
    let mut descriptors = vec![];
    let mut controls = vec![];
    let mut drops = DropCounters::new();
    for spec in &config.simulated {
        let source = SimulatedSource {
            spec: spec.clone(),
            skeleton: skeleton.clone(),
            assignments: config.assignments.clone(),
        };
        for d in source.descriptors() {
            drops.insert(d.id.clone(), Arc::new(AtomicU64::new(0)));
            descriptors.push(d);
        }
        let (control, receiver) = watch::channel(Control {
            motion: spec.motion,
            speed: spec.speed,
            paused: false,
            generation: 0,
            suspended: false,
        });
        controls.push(control);
        sources.push((source, receiver));
    }
    let assignments = crate::registry::persistence::load_assignments(
        &config.config.engine.data_dir,
        &config.assignments,
        &skeleton,
    )?;
    let registry = Registry::new(descriptors, assignments, drops.clone());
    let runner = Runner {
        config: config.clone(),
        registry,
        store,
        skeleton,
        clock: clock.clone(),
        events: events_rx,
        controls,
        faults: faults_tx,
    };
    let (client, streams, runner_task) = runner.spawn(cancel.clone());
    let mut tasks: Vec<JoinHandle<Result<(), EngineError>>> = vec![tokio::spawn(async move {
        runner_task.await?;
        Ok(())
    })];
    for (source, control) in sources {
        let context = SourceContext {
            events: events_tx.clone(),
            control,
            clock: clock.clone(),
            cancel: cancel.clone(),
            dropped: drops.clone(),
            faults: faults_rx.clone(),
        };
        let devices = source.descriptors().into_iter().map(|d| d.id).collect();
        tasks.push(tokio::spawn(async move {
            crate::sources::supervision::supervise(source, devices, context).await;
            Ok(())
        }));
    }
    let api = Api {
        client,
        streams,
        allowed_origins: Arc::new(config.config.api.allowed_origins.clone()),
        publish_rate: config.config.engine.publish_rate_hz,
        cancel: cancel.clone(),
    };
    let router = api::router(api);
    let stop = cancel.clone();
    tasks.push(tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(stop.cancelled_owned())
            .await?;
        Ok(())
    }));
    for warning in &config.warnings {
        tracing::warn!(path=%warning.path,message=%warning.message,"configuration warning");
    }
    tracing::info!(%address,"Kinexa engine listening");
    Ok(EngineHandle {
        address,
        cancel,
        tasks,
    })
}
pub async fn run(
    config: Validated,
    shutdown: impl std::future::Future<Output = ()>,
) -> Result<(), EngineError> {
    let engine = start(config).await?;
    shutdown.await;
    engine.shutdown().await
}
