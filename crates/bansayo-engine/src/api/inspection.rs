use super::{Api, failure};
use axum::{
    Json,
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
};
use bansayo_contracts::{ApiErrorCode, ApiResponse, DeviceId, EngineMode};

pub async fn clock(State(api): State<Api>) -> Response {
    if matches!(api.streams.state.borrow().engine.mode, EngineMode::Replay) {
        return failure(
            ApiErrorCode::Conflict,
            "Live capture is unavailable during replay.",
        );
    }
    let clock = api.diagnostic_recorder.clock();
    Json(ApiResponse::ok(serde_json::json!({
        "session_epoch_utc": clock.epoch_utc(),
        "t_us": clock.now_us()
    })))
    .into_response()
}

pub async fn snapshot(State(api): State<Api>, Path(id): Path<String>) -> Response {
    let Ok(id) = DeviceId::new(id.as_str()) else {
        return failure(ApiErrorCode::NotFound, "Unknown device.");
    };
    {
        let state = api.streams.state.borrow();
        if matches!(state.engine.mode, EngineMode::Replay) {
            return failure(
                ApiErrorCode::Conflict,
                "Live inspection is unavailable during replay.",
            );
        }
        if !state.devices.iter().any(|d| d.descriptor.id == id) {
            return failure(ApiErrorCode::NotFound, "Unknown device.");
        }
    }
    Json(ApiResponse::ok(crate::sources::inspection::snapshot(
        &api.runtime,
        &id,
    )))
    .into_response()
}

#[derive(serde::Deserialize)]
pub struct BatchQuery {
    epoch: Option<u64>,
    #[serde(default)]
    after: u64,
}

pub async fn batch(
    State(api): State<Api>,
    Path(id): Path<String>,
    Query(query): Query<BatchQuery>,
) -> Response {
    let Ok(id) = DeviceId::new(id.as_str()) else {
        return failure(ApiErrorCode::NotFound, "Unknown device.");
    };
    {
        let state = api.streams.state.borrow();
        if matches!(state.engine.mode, EngineMode::Replay) {
            return failure(
                ApiErrorCode::Conflict,
                "Live capture is unavailable during replay.",
            );
        }
        if !state.devices.iter().any(|d| d.descriptor.id == id) {
            return failure(ApiErrorCode::NotFound, "Unknown device.");
        }
    }
    Json(ApiResponse::ok(crate::sources::inspection::batch(
        &api.runtime,
        &id,
        query.epoch,
        query.after,
    )))
    .into_response()
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SolverRequest {
    device_ids: Vec<DeviceId>,
}

pub async fn solver(
    State(api): State<Api>,
    body: Result<Json<SolverRequest>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return failure(ApiErrorCode::InvalidRequest, "Provide selected device ids.");
    };
    match api
        .client
        .control(crate::pipeline::command::ControlRequest::InspectSolver(
            body.device_ids,
        ))
        .await
    {
        Ok(data) => Json(ApiResponse::ok(data)).into_response(),
        Err(error) => super::control_error(error),
    }
}

pub async fn performance(State(api): State<Api>) -> Response {
    Json(ApiResponse::ok(api.performance.borrow().as_ref().clone())).into_response()
}

#[derive(serde::Deserialize)]
pub struct ReliabilityQuery {
    before_ms: Option<u64>,
}

pub async fn reliability(State(api): State<Api>, Query(query): Query<ReliabilityQuery>) -> Response {
    let path = crate::reliability::path(&api.data_dir);
    let result =
        tokio::task::spawn_blocking(move || crate::reliability::read(&path, query.before_ms)).await;
    match result {
        Ok(Ok(rows)) => Json(ApiResponse::ok(serde_json::json!({
            "enabled": api.reliability_enabled, "rows": rows
        })))
        .into_response(),
        Ok(Err(error)) => {
            tracing::warn!(%error, "Reliability history read failed");
            failure(ApiErrorCode::Unavailable, "Reliability history unavailable.")
        }
        Err(error) => {
            tracing::warn!(%error, "Reliability history worker failed");
            failure(ApiErrorCode::Internal, "Reliability history worker failed.")
        }
    }
}
