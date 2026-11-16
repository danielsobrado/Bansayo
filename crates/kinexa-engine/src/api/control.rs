use super::{Api, control_error, failure};
use crate::pipeline::command::ControlRequest;
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    response::{IntoResponse, Response},
};
use kinexa_contracts::*;
async fn execute(api: Api, request: ControlRequest) -> Response {
    match api.client.control(request).await {
        Ok(data) => Json(ApiResponse::ok(data)).into_response(),
        Err(e) => control_error(e),
    }
}
fn bad() -> Response {
    failure(
        ApiErrorCode::InvalidRequest,
        "Provide a valid JSON request with supported identifiers and values.",
    )
}
pub async fn assignment(
    State(api): State<Api>,
    Path(id): Path<String>,
    body: Result<Json<AssignmentRequest>, JsonRejection>,
) -> Response {
    let (Ok(id), Ok(Json(body))) = (DeviceId::new(id), body) else {
        return bad();
    };
    execute(api, ControlRequest::Assignment(id, body.segment)).await
}
pub async fn fault(
    State(api): State<Api>,
    Path(id): Path<String>,
    body: Result<Json<FaultRequest>, JsonRejection>,
) -> Response {
    let (Ok(id), Ok(Json(body))) = (DeviceId::new(id), body) else {
        return bad();
    };
    execute(api, ControlRequest::Fault(id, body)).await
}
pub async fn delete(State(api): State<Api>, Path(id): Path<String>) -> Response {
    let Ok(id) = ProfileId::new(id) else {
        return bad();
    };
    execute(api, ControlRequest::Delete(id)).await
}
pub async fn sessions(State(api): State<Api>) -> Response {
    execute(api, ControlRequest::Sessions).await
}
pub async fn record_start(
    State(api): State<Api>,
    body: Result<Json<RecordingStartRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return bad();
    };
    execute(api, ControlRequest::RecordStart(body)).await
}
pub async fn record_stop(State(api): State<Api>) -> Response {
    execute(api, ControlRequest::RecordStop).await
}
pub async fn playback_start(
    State(api): State<Api>,
    body: Result<Json<PlaybackStartRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return bad();
    };
    execute(api, ControlRequest::PlaybackStart(body)).await
}
pub async fn playback_update(
    State(api): State<Api>,
    body: Result<Json<PlaybackUpdateRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(body)) = body else {
        return bad();
    };
    execute(api, ControlRequest::PlaybackUpdate(body)).await
}
pub async fn playback_stop(State(api): State<Api>) -> Response {
    execute(api, ControlRequest::PlaybackStop).await
}
