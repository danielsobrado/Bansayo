//! Source isolation and capped restart policy. A failed source cannot terminate tracking or HTTP.
use super::{Ingest, Source};
use bansayo_contracts::DeviceId;
use std::time::{Duration, Instant};
pub const MAX_FAILURES: u32 = 5;
pub const INITIAL_BACKOFF: Duration = Duration::from_millis(100);
pub const MAX_BACKOFF: Duration = Duration::from_secs(2);
pub const STABLE_WINDOW: Duration = Duration::from_secs(10);
pub async fn supervise<S: Source + Clone>(source: S, devices: Vec<DeviceId>, context: S::Context) {
    let mut failures = 0;
    loop {
        if context.as_ref().cancel.is_cancelled() {
            break;
        }
        let started = Instant::now();
        let source = source.clone();
        let child = context.clone();
        let mut task = tokio::spawn(async move { source.run(child).await });
        let outcome = tokio::select! {_=context.as_ref().cancel.cancelled()=>{task.abort();let _=task.await;break;},outcome=&mut task=>outcome};
        if context.as_ref().cancel.is_cancelled() {
            break;
        }
        if started.elapsed() >= STABLE_WINDOW {
            failures = 0;
        }
        failures += 1;
        let retry = failures < MAX_FAILURES;
        tracing::error!(
            ?outcome,
            failures,
            retry,
            "Source task stopped unexpectedly"
        );
        let message = if retry {
            "Source stopped unexpectedly. Retrying automatically."
        } else {
            "Source repeatedly failed. Restart the engine after checking its configuration."
        };
        // Control-plane failures wait for bounded capacity, preserving the device failure report.
        let report = Ingest::Failure {
            devices: devices.clone(),
            message: message.into(),
        };
        tokio::select! {
            _ = context.as_ref().cancel.cancelled() => break,
            result = context.as_ref().events.send(report) => {
                if result.is_err() {
                    break;
                }
            }
        }
        if !retry {
            break;
        }
        let delay = (INITIAL_BACKOFF * 2_u32.pow(failures - 1)).min(MAX_BACKOFF);
        tokio::select! {_=context.as_ref().cancel.cancelled()=>break,_=tokio::time::sleep(delay)=>{}}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::SessionClock;
    use crate::sources::SourceContext;
    use std::{
        collections::BTreeMap,
        sync::{
            Arc,
            atomic::{AtomicU32, Ordering},
        },
    };
    #[derive(Clone)]
    struct Failing(Arc<AtomicU32>);
    impl Source for Failing {
        type Context = SourceContext;
        async fn run(self, _: SourceContext) -> Result<(), super::super::SourceError> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Err(super::super::SourceError("test failure".into()))
        }
    }
    #[tokio::test]
    async fn cancellation_interrupts_a_blocked_failure_report() {
        let (events, _rx) = tokio::sync::mpsc::channel(1);
        events
            .try_send(Ingest::Notice {
                code: "full".into(),
                message: "full".into(),
            })
            .unwrap_or_else(|_| panic!("queue should have capacity"));
        let cancel = tokio_util::sync::CancellationToken::new();
        let context = SourceContext {
            events,
            clock: SessionClock::start(),
            cancel: cancel.clone(),
            dropped: BTreeMap::new(),
        };
        let attempts = Arc::new(AtomicU32::new(0));
        let task = tokio::spawn(supervise(Failing(attempts.clone()), vec![], context));
        tokio::time::timeout(Duration::from_secs(2), async {
            while attempts.load(Ordering::Relaxed) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        cancel.cancel();
        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn stops_after_capped_restarts_and_reports_each_failure() {
        let (events, mut rx) = tokio::sync::mpsc::channel(16);
        let context = SourceContext {
            events,
            clock: SessionClock::start(),
            cancel: tokio_util::sync::CancellationToken::new(),
            dropped: BTreeMap::new(),
        };
        let attempts = Arc::new(AtomicU32::new(0));
        supervise(
            Failing(attempts.clone()),
            vec![DeviceId::new("test").unwrap()],
            context,
        )
        .await;
        assert_eq!(attempts.load(Ordering::Relaxed), MAX_FAILURES);
        let mut reports = 0;
        while rx.try_recv().is_ok() {
            reports += 1;
        }
        assert_eq!(reports, MAX_FAILURES);
    }
}
