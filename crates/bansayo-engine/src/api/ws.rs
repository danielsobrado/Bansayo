use super::{Api, failure};
use crate::pipeline::encoded::Encoded;
use axum::extract::ws::rejection::WebSocketUpgradeRejection;
use axum::{
    extract::{
        Query, State, WebSocketUpgrade,
        rejection::QueryRejection,
        ws::{Message, WebSocket},
    },
    response::{IntoResponse, Response},
};
use bansayo_contracts::*;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
#[derive(Default, Deserialize)]
pub struct StreamQuery {
    topics: Option<String>,
    max_rate_hz: Option<f64>,
    /// Extra observable ids for the `observables` topic, comma-separated, at most 32.
    observables: Option<String>,
}

/// Feature flags announced in `hello` (14 §4).
pub const CAPABILITIES: &[&str] = &["training", "content", "athletes", "progress"];
pub async fn upgrade(
    State(api): State<Api>,
    query: Result<Query<StreamQuery>, QueryRejection>,
    ws: Result<WebSocketUpgrade, WebSocketUpgradeRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return failure(ApiErrorCode::InvalidRequest, "Invalid stream query.");
    };
    let topics = query
        .topics
        .unwrap_or_else(|| "frame,state".into())
        .split(',')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if topics.iter().any(|t| {
        !matches!(
            t.as_str(),
            "frame" | "state" | "landmarks_2d" | "landmarks_3d" | "training" | "observables"
        )
    }) {
        return failure(
            ApiErrorCode::InvalidRequest,
            "Stream topics must be frame, state, landmarks_2d, landmarks_3d, training or observables.",
        );
    }
    let extra: Vec<String> = query
        .observables
        .as_deref()
        .map(|s| {
            s.split(',')
                .filter(|x| !x.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    if extra.len() > 32 {
        return failure(
            ApiErrorCode::InvalidRequest,
            "At most 32 extra observables.",
        );
    }
    let rate = query.max_rate_hz.unwrap_or(api.publish_rate);
    if !(rate.is_finite() && rate > 0.0 && rate <= 1000.0) {
        return failure(
            ApiErrorCode::InvalidRequest,
            "Stream maximum rate must be greater than 0 and at most 1000 Hz.",
        );
    }
    let Ok(ws) = ws else {
        return failure(
            ApiErrorCode::InvalidRequest,
            "A WebSocket upgrade is required.",
        );
    };
    ws.on_upgrade(move |socket| stream(socket, api, topics, rate, extra))
        .into_response()
}
async fn send(
    socket: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: &ServerMessage,
) -> bool {
    let Ok(text) = serde_json::to_string(message) else {
        return false;
    };
    matches!(
        tokio::time::timeout(
            Duration::from_secs(1),
            socket.send(Message::Text(text.into()))
        )
        .await,
        Ok(Ok(()))
    )
}
async fn send_encoded(
    socket: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    message: &Encoded,
) -> bool {
    let Ok(text) = axum::extract::ws::Utf8Bytes::try_from(message.bytes.clone()) else {
        return false;
    };
    matches!(
        tokio::time::timeout(Duration::from_secs(1), socket.send(Message::Text(text))).await,
        Ok(Ok(()))
    )
}
async fn stream(
    socket: WebSocket,
    mut api: Api,
    topics: Vec<String>,
    rate: f64,
    extra: Vec<String>,
) {
    let (mut sink, mut incoming) = socket.split();
    let hello = ServerMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        engine_version: env!("CARGO_PKG_VERSION").into(),
        session_epoch_utc: api.streams.epoch.clone(),
        conventions: Conventions::default(),
        skeleton: api.streams.skeleton.borrow().clone(),
        state: api.streams.state.borrow().clone(),
        capabilities: Some(CAPABILITIES.iter().map(|s| (*s).to_owned()).collect()),
        observables_rev: Some(bansayo_training::observables::OBSERVABLES_REV),
    };
    if !send(&mut sink, &hello).await {
        return;
    }
    let wants_training = topics.iter().any(|t| t == "training");
    let wants_observables = topics.iter().any(|t| t == "observables");
    let mut training_events = api.training_streams.events.subscribe();
    if wants_training {
        // training_state follows hello immediately (14 §4).
        let state = api
            .training_streams
            .state
            .borrow_and_update()
            .as_deref()
            .cloned();
        if !send(&mut sink, &ServerMessage::TrainingState { state }).await {
            return;
        }
    }
    let mut last_observables = None;
    let wants_frame = topics.iter().any(|t| t == "frame");
    let wants_state = topics.iter().any(|t| t == "state");
    let wants_2d = topics.iter().any(|t| t == "landmarks_2d");
    let wants_3d = topics.iter().any(|t| t == "landmarks_3d");
    let mut landmarks = api.streams.landmarks.subscribe();
    let mut notices = api.streams.notices.subscribe();
    // shortcut: one ticker thread per socket, fine for a few local clients; share one ticker
    // per rate if many-client load ever matters.
    let mut timer =
        crate::ticker::ticker(Duration::from_secs_f64(1.0 / rate.min(api.publish_rate)));
    let mut last_seq = None;
    let mut last_skeleton_rev = api.streams.skeleton.borrow().rev;
    let mut last_state_rev = api.streams.state.borrow().rev;
    loop {
        // Only one event exists per socket task; keeping it inline avoids an allocation.
        #[allow(clippy::large_enum_variant)]
        enum Event {
            Stop,
            Notice(Result<Arc<Encoded>, tokio::sync::broadcast::error::RecvError>),
            Input(Option<Result<Message, axum::Error>>),
            State(Result<(), tokio::sync::watch::error::RecvError>),
            Frame,
            Landmark(Result<Arc<Encoded>, tokio::sync::broadcast::error::RecvError>),
            Training(Result<(), tokio::sync::watch::error::RecvError>),
            TrainingIncrement(
                Result<Arc<(SessionId, TrainingEvent)>, tokio::sync::broadcast::error::RecvError>,
            ),
        }
        let event = tokio::select! {
            _=api.cancel.cancelled()=>Event::Stop,
            notice=notices.recv()=>Event::Notice(notice),
            input=incoming.next()=>Event::Input(input),
            changed=api.streams.state.changed(),if wants_state=>Event::State(changed),
            _=timer.recv(),if wants_frame || wants_observables=>Event::Frame,
            changed=api.training_streams.state.changed(),if wants_training=>Event::Training(changed),
            e=training_events.recv(),if wants_training=>Event::TrainingIncrement(e),
            point=landmarks.recv(),if wants_2d || wants_3d=>Event::Landmark(point),
        };
        match event {
            Event::Notice(Ok(message)) => {
                if !send_encoded(&mut sink, &message).await {
                    break;
                }
            }
            Event::Notice(Err(_)) => {}
            Event::Stop => {
                let _ = tokio::time::timeout(
                    Duration::from_millis(200),
                    sink.send(Message::Close(None)),
                )
                .await;
                break;
            }
            Event::Input(None | Some(Err(_)) | Some(Ok(Message::Close(_)))) => break,
            Event::Input(_) => {}
            Event::State(Err(_)) => break,
            Event::State(Ok(())) => {
                let state = api.streams.state.borrow_and_update().clone();
                last_state_rev = state.rev;
                let packet = api.streams.state_wire.borrow().clone();
                if !send_encoded(&mut sink, &packet).await {
                    break;
                }
            }
            Event::Training(Err(_)) => break,
            Event::Training(Ok(())) => {
                let state = api
                    .training_streams
                    .state
                    .borrow_and_update()
                    .as_deref()
                    .cloned();
                if !send(&mut sink, &ServerMessage::TrainingState { state }).await {
                    break;
                }
            }
            Event::TrainingIncrement(Ok(e)) => {
                let (session_id, event) = (e.0.clone(), e.1.clone());
                if !send(
                    &mut sink,
                    &ServerMessage::TrainingEvent { session_id, event },
                )
                .await
                {
                    break;
                }
            }
            // A lagging client sees an `event_seq` gap and refills over HTTP (14 §3).
            Event::TrainingIncrement(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
            Event::TrainingIncrement(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                break;
            }
            Event::Frame if !wants_frame => {
                if !send_observables(&mut sink, &api, &extra, &mut last_observables).await {
                    break;
                }
            }
            Event::Frame => {
                if wants_observables
                    && !send_observables(&mut sink, &api, &extra, &mut last_observables).await
                {
                    break;
                }
                let skeleton = api.streams.skeleton.borrow().clone();
                if skeleton.rev != last_skeleton_rev {
                    last_skeleton_rev = skeleton.rev;
                    if !send(&mut sink, &ServerMessage::Skeleton { skeleton }).await {
                        break;
                    }
                }

                let state = api.streams.state.borrow().clone();
                if state.rev != last_state_rev {
                    last_state_rev = state.rev;
                    let packet = api.streams.state_wire.borrow().clone();
                    if !send_encoded(&mut sink, &packet).await {
                        break;
                    }
                }

                let frame = api.streams.frame.borrow().clone();
                if last_seq == Some(frame.sequence) {
                    continue;
                }
                last_seq = Some(frame.sequence);
                if !send_encoded(&mut sink, &frame).await {
                    break;
                }
            }
            Event::Landmark(Ok(landmarks)) => {
                // Both kinds share one channel; an internally tagged message serialises `type` first.
                let wanted = if landmarks.bytes.starts_with(br#"{"type":"landmarks_3d""#) {
                    wants_3d
                } else {
                    wants_2d
                };
                if wanted && !send_encoded(&mut sink, &landmarks).await {
                    break;
                }
            }
            Event::Landmark(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
            Event::Landmark(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
        }
    }
}

/// Latest observables, at most once per frame tick, filtered to the session's live ids plus the
/// requested extras (14 §4).
async fn send_observables(
    sink: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    api: &Api,
    extra: &[String],
    last: &mut Option<u64>,
) -> bool {
    let snapshot = api.training_streams.observables.borrow().clone();
    let Some(o) = snapshot else { return true };
    if *last == Some(o.seq) {
        return true;
    }
    *last = Some(o.seq);
    let live: Vec<String> = api
        .training_streams
        .state
        .borrow()
        .as_ref()
        .map(|s| s.live_observables.clone())
        .unwrap_or_default();
    let values = o
        .values
        .iter()
        .filter(|v| live.contains(&v.id) || extra.contains(&v.id))
        .cloned()
        .collect();
    send(
        sink,
        &ServerMessage::Observables {
            seq: o.seq,
            t_us: o.t_us,
            values,
        },
    )
    .await
}
