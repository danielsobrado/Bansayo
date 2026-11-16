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
use futures_util::{SinkExt, StreamExt};
use kinexa_contracts::*;
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
#[derive(Default, Deserialize)]
pub struct StreamQuery {
    topics: Option<String>,
    max_rate_hz: Option<f64>,
}
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
    if topics
        .iter()
        .any(|t| !matches!(t.as_str(), "frame" | "state" | "landmarks_2d"))
    {
        return failure(
            ApiErrorCode::InvalidRequest,
            "Stream topics must be frame, state or landmarks_2d.",
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
    ws.on_upgrade(move |socket| stream(socket, api, topics, rate))
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
async fn stream(socket: WebSocket, mut api: Api, topics: Vec<String>, rate: f64) {
    let (mut sink, mut incoming) = socket.split();
    let hello = ServerMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
        engine_version: env!("CARGO_PKG_VERSION").into(),
        session_epoch_utc: api.streams.epoch.clone(),
        conventions: Conventions::default(),
        skeleton: api.streams.skeleton.borrow().clone(),
        state: api.streams.state.borrow().clone(),
    };
    if !send(&mut sink, &hello).await {
        return;
    }
    let wants_frame = topics.iter().any(|t| t == "frame");
    let wants_state = topics.iter().any(|t| t == "state");
    let wants_landmarks = topics.iter().any(|t| t == "landmarks_2d");
    let mut landmarks = api.streams.landmarks.subscribe();
    let mut notices = api.streams.notices.subscribe();
    let mut timer =
        tokio::time::interval(Duration::from_secs_f64(1.0 / rate.min(api.publish_rate)));
    timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
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
        }
        let event = tokio::select! {
            _=api.cancel.cancelled()=>Event::Stop,
            notice=notices.recv()=>Event::Notice(notice),
            input=incoming.next()=>Event::Input(input),
            changed=api.streams.state.changed(),if wants_state=>Event::State(changed),
            _=timer.tick(),if wants_frame=>Event::Frame,
            point=landmarks.recv(),if wants_landmarks=>Event::Landmark(point),
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
            Event::Frame => {
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
                if !send_encoded(&mut sink, &landmarks).await {
                    break;
                }
            }
            Event::Landmark(Err(tokio::sync::broadcast::error::RecvError::Closed)) => break,
            Event::Landmark(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
        }
    }
}
