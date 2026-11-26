use super::{Api, failure};
use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Response},
};
use bansayo_contracts::{ApiErrorCode, ApiResponse, DeviceId, DeviceKind, EngineMode};
use std::collections::BTreeSet;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    device_ids: Vec<DeviceId>,
    duration_s: u64,
}

pub async fn status(State(api): State<Api>) -> impl IntoResponse {
    Json(ApiResponse::ok(api.diagnostic_recorder.status()))
}

pub async fn start(State(api): State<Api>, Json(body): Json<Request>) -> Response {
    {
        let state = api.streams.state.borrow();
        if matches!(state.engine.mode, EngineMode::Replay) {
            return failure(ApiErrorCode::Conflict, "Recording requires live mode.");
        }
        let unique: BTreeSet<_> = body.device_ids.iter().collect();
        if unique.len() != body.device_ids.len()
            || body.device_ids.is_empty()
            || body.device_ids.len() > 4
            || !(1..=3600).contains(&body.duration_s)
            || body.device_ids.iter().any(|id| {
                !state
                    .devices
                    .iter()
                    .any(|d| d.descriptor.id == *id && d.descriptor.kind != DeviceKind::Camera)
            })
        {
            return failure(
                ApiErrorCode::InvalidRequest,
                "Select 1–4 distinct non-camera devices and 1–3600 seconds.",
            );
        }
    }
    match api
        .diagnostic_recorder
        .start(body.device_ids, body.duration_s)
    {
        Ok(status) => Json(ApiResponse::ok(status)).into_response(),
        Err(message) => failure(ApiErrorCode::Conflict, message),
    }
}

pub async fn stop(State(api): State<Api>) -> impl IntoResponse {
    Json(ApiResponse::ok(api.diagnostic_recorder.stop()))
}
