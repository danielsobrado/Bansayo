use super::{Api, failure};
use axum::{
    Json,
    extract::{Path, Query, State},
    response::{IntoResponse, Response},
};
use bansayo_contracts::{ApiErrorCode, ApiResponse, DeviceId, EngineMode};

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
