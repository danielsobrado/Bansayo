use super::{Api, failure};
use axum::{
    Json,
    extract::{Path, State},
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
            return failure(ApiErrorCode::Conflict, "Live inspection is unavailable during replay.");
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
