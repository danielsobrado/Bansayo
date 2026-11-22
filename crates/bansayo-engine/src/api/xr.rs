//! `GET /api/v1/ingest/xr`: the VR client's OpenXR head and grip poses (Phase 01).
//!
//! One writer at a time, this machine only, typed JSON text frames of bounded size. Each frame is
//! validated and stamped by `sources::xr`, then queued without waiting on the existing ingest
//! channel, so the runtime validates, records and replays it like any other source.
use super::{Api, failure};
use crate::{
    clock::SessionClock,
    config::XrConfig,
    sources::{
        DropCounters, Ingest,
        xr::{XrLink, XrValidationError},
    },
};
use axum::{
    extract::{
        ConnectInfo, Extension, State, WebSocketUpgrade,
        ws::{CloseFrame, Message, WebSocket, close_code, rejection::WebSocketUpgradeRejection},
    },
    response::{IntoResponse, Response},
};
use bansayo_contracts::*;
use std::{
    collections::BTreeSet,
    net::SocketAddr,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch},
    time::Instant,
};
use tokio_util::sync::CancellationToken;

/// A connection that has not said hello by then is closed.
const HELLO_TIMEOUT: Duration = Duration::from_secs(5);
/// Bound on sending a control message to a client that does not read.
const SEND_TIMEOUT: Duration = Duration::from_secs(1);

pub struct XrIngress {
    pub config: XrConfig,
    pub clock: SessionClock,
    pub events: mpsc::Sender<Ingest>,
    pub cancel: CancellationToken,
    pub single_writer: Arc<Semaphore>,
    pub dropped: DropCounters,
}
impl XrIngress {
    pub fn new(
        config: XrConfig,
        clock: SessionClock,
        events: mpsc::Sender<Ingest>,
        cancel: CancellationToken,
        dropped: DropCounters,
    ) -> Self {
        Self {
            config,
            clock,
            events,
            cancel,
            single_writer: Arc::new(Semaphore::new(1)),
            dropped,
        }
    }
    /// Queues without waiting; a full queue counts a drop for the event's device.
    fn enqueue(&self, event: SourceEvent) -> bool {
        let device = match &event {
            SourceEvent::Observation { observation } => observation.device_id.clone(),
            SourceEvent::DeviceLink { device_id, .. } => device_id.clone(),
            SourceEvent::DeviceAnnounced { descriptor } => descriptor.id.clone(),
        };
        let queued = self.events.try_send(Ingest::Event(event)).is_ok();
        if !queued && let Some(c) = self.dropped.get(&device) {
            c.fetch_add(1, Ordering::Relaxed);
        }
        queued
    }
}

/// Only a peer on this machine may stream poses; missing connection info is not trusted.
pub(super) fn peer_allowed(peer: Option<SocketAddr>) -> bool {
    peer.is_some_and(|p| p.ip().is_loopback())
}

pub async fn upgrade(
    State(api): State<Api>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    let Some(ingress) = api.xr.clone() else {
        return failure(
            ApiErrorCode::NotFound,
            "XR ingest is off: set xr.enabled: true in the engine configuration.",
        );
    };
    if !peer_allowed(peer.map(|Extension(c)| c.0)) {
        return failure(
            ApiErrorCode::ForbiddenOrigin,
            "XR ingest accepts connections from this machine only.",
        );
    }
    if api.streams.state.borrow().playback.is_some() {
        return failure(
            ApiErrorCode::Conflict,
            "A recording is playing back: stop playback before streaming live XR poses.",
        );
    }
    let Ok(ws) = ws else {
        return failure(
            ApiErrorCode::InvalidRequest,
            "A WebSocket upgrade is required.",
        );
    };
    let Ok(permit) = ingress.single_writer.clone().try_acquire_owned() else {
        return failure(
            ApiErrorCode::Conflict,
            "Another XR client is already streaming.",
        );
    };
    let state = api.streams.state.clone();
    ws.max_message_size(ingress.config.max_message_bytes)
        .max_frame_size(ingress.config.max_message_bytes)
        .on_upgrade(move |socket| session(socket, ingress, state, permit))
        .into_response()
}

async fn send(socket: &mut WebSocket, message: &XrServerMessage) {
    if let Ok(text) = serde_json::to_string(message) {
        let _ = tokio::time::timeout(SEND_TIMEOUT, socket.send(Message::Text(text.into()))).await;
    }
}

async fn close(socket: &mut WebSocket, code: u16, reason: &'static str) {
    let frame = CloseFrame {
        code,
        reason: reason.into(),
    };
    let _ = tokio::time::timeout(SEND_TIMEOUT, socket.send(Message::Close(Some(frame)))).await;
}

fn oversized(e: axum::Error) -> bool {
    use tokio_tungstenite::tungstenite::Error;
    e.into_inner()
        .downcast::<Error>()
        .is_ok_and(|e| matches!(*e, Error::Capacity(_)))
}

#[derive(Debug, Default)]
struct Stats {
    frames: u64,
    observations: u64,
    rejected: u64,
    over_rate: u64,
    idle: u64,
}

/// Waits for `hello`; answers `ready`, or closes with 1002 and returns `None`.
async fn handshake(socket: &mut WebSocket, cancel: &CancellationToken) -> Option<()> {
    let first = tokio::select! {
        _ = cancel.cancelled() => return None,
        m = tokio::time::timeout(HELLO_TIMEOUT, socket.recv()) => m,
    };
    let text = match first {
        Ok(Some(Ok(Message::Text(text)))) => text,
        Ok(Some(Ok(Message::Binary(_)))) => {
            close(socket, close_code::UNSUPPORTED, "text frames only").await;
            return None;
        }
        Ok(Some(Err(e))) => {
            if oversized(e) {
                close(socket, close_code::SIZE, "message too large").await;
            }
            return None;
        }
        Err(_) => {
            close(socket, close_code::PROTOCOL, "hello expected").await;
            return None;
        }
        _ => return None,
    };
    match serde_json::from_str::<XrClientMessage>(&text) {
        Ok(XrClientMessage::Hello(XrHello { protocol_version }))
            if protocol_version == XR_PROTOCOL_VERSION =>
        {
            let ready = XrServerMessage::Ready {
                protocol_version: XR_PROTOCOL_VERSION,
                time_quality: TimeQuality::ReceiveTime,
            };
            send(socket, &ready).await;
            Some(())
        }
        Ok(XrClientMessage::Hello(XrHello { protocol_version })) => {
            let error = XrServerMessage::Error {
                code: "xr_protocol_version".into(),
                message: format!(
                    "protocol_version {protocol_version} is not supported; this engine speaks {XR_PROTOCOL_VERSION}"
                ),
            };
            send(socket, &error).await;
            close(socket, close_code::PROTOCOL, "unsupported protocol_version").await;
            None
        }
        _ => {
            close(socket, close_code::PROTOCOL, "hello expected").await;
            None
        }
    }
}

async fn session(
    mut socket: WebSocket,
    ingress: Arc<XrIngress>,
    state: watch::Receiver<SystemState>,
    _permit: OwnedSemaphorePermit,
) {
    let cancel = ingress.cancel.clone();
    if handshake(&mut socket, &cancel).await.is_none() {
        tracing::info!("XR uplink refused at handshake");
        return;
    }
    tracing::info!("XR uplink connected");
    let config = &ingress.config;
    let idle = Duration::from_millis(config.tracking_stale_after_ms);
    let mut link = XrLink::default();
    let mut stats = Stats::default();
    // Each kind of rejection is reported to the client once per connection, not per frame.
    let mut reported = BTreeSet::new();
    let mut idle_at = Some(Instant::now() + idle);
    let ended = loop {
        let message = tokio::select! {
            _ = cancel.cancelled() => break "engine shutdown",
            _ = async { tokio::time::sleep_until(idle_at.unwrap_or_else(Instant::now)).await }, if idle_at.is_some() => {
                // Silent sender: stop presenting its last poses as current; the socket stays.
                link.unlink_all(ingress.clock.now_us(), |e| ingress.enqueue(e));
                stats.idle += 1;
                idle_at = None;
                continue;
            }
            m = socket.recv() => m,
        };
        let text = match message {
            Some(Ok(Message::Text(text))) => text,
            Some(Ok(Message::Binary(_))) => {
                close(&mut socket, close_code::UNSUPPORTED, "text frames only").await;
                break "binary frame";
            }
            Some(Ok(Message::Close(_))) | None => break "closed by client",
            Some(Ok(_)) => continue,
            Some(Err(e)) => {
                if oversized(e) {
                    close(&mut socket, close_code::SIZE, "message too large").await;
                    break "message too large";
                }
                break "transport error";
            }
        };
        // One receive instant for the whole frame: all three devices share it.
        let received_us = ingress.clock.now_us();
        let result = match serde_json::from_str::<XrClientMessage>(&text) {
            Ok(XrClientMessage::Frame(frame)) if state.borrow().playback.is_some() => {
                let _ = frame;
                Err((
                    "xr_replay_active",
                    "A recording is playing back; live poses are ignored.".to_owned(),
                ))
            }
            Ok(XrClientMessage::Frame(frame)) => {
                match link.frame(&frame, config, received_us, |e| ingress.enqueue(e)) {
                    Ok(n) => {
                        stats.frames += 1;
                        stats.observations += n as u64;
                        idle_at = Some(Instant::now() + idle);
                        Ok(())
                    }
                    Err(XrValidationError::OverRate) => {
                        stats.over_rate += 1;
                        idle_at = Some(Instant::now() + idle);
                        Ok(())
                    }
                    Err(e) => Err((e.code(), e.to_string())),
                }
            }
            Ok(XrClientMessage::Hello(_)) => {
                close(&mut socket, close_code::PROTOCOL, "hello sent twice").await;
                break "protocol error";
            }
            Err(e) => Err(("xr_malformed", format!("Not a valid XR frame: {e}"))),
        };
        if let Err((code, message)) = result {
            stats.rejected += 1;
            tracing::debug!(code, %message, "XR frame rejected");
            if reported.insert(code) {
                let error = XrServerMessage::Error {
                    code: code.into(),
                    message,
                };
                send(&mut socket, &error).await;
            }
        }
    };
    link.unlink_all(ingress.clock.now_us(), |e| ingress.enqueue(e));
    let dropped: u64 = ingress
        .dropped
        .iter()
        .filter(|(id, _)| crate::sources::xr::DEVICE_IDS.contains(&id.as_str()))
        .map(|(_, c)| c.load(Ordering::Relaxed))
        .sum();
    tracing::info!(
        reason = ended,
        frames = stats.frames,
        observations = stats.observations,
        rejected = stats.rejected,
        over_rate = stats.over_rate,
        idle_timeouts = stats.idle,
        queue_full_total = dropped,
        "XR uplink disconnected"
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn xr_remote_peer_is_refused() {
        use super::peer_allowed;
        assert!(peer_allowed(Some(([127, 0, 0, 1], 5000).into())));
        assert!(peer_allowed(Some(
            (std::net::Ipv6Addr::LOCALHOST, 5000).into()
        )));
        assert!(!peer_allowed(Some(([192, 168, 1, 20], 5000).into())));
        assert!(!peer_allowed(Some(([0, 0, 0, 0], 5000).into())));
        assert!(
            !peer_allowed(None),
            "missing connection info is not trusted"
        );
    }
}
