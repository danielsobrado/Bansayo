//! HTTP boundary: only pipeline commands and shared contracts cross into the engine.
mod control;
mod ws;
use crate::pipeline::{
    Streams,
    command::{Client, ControlError},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Request, State, rejection::JsonRejection},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use bansayo_contracts::*;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
#[derive(Clone)]
pub struct Api {
    pub client: Client,
    pub streams: Streams,
    pub allowed_origins: Arc<Vec<String>>,
    pub publish_rate: f64,
    pub cancel: CancellationToken,
    pub previews: crate::sources::camera::Previews,
}
pub fn router(api: Api) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/state", get(state))
        .route("/api/v1/skeleton", get(skeleton))
        .route("/api/v1/simulation", put(simulation))
        .route("/api/v1/calibration/reference-pose", post(capture))
        .route("/api/v1/calibration/profiles", post(import_profiles))
        .route("/api/v1/stream", get(ws::upgrade))
        .route("/api/v1/cameras/{id}/preview.jpg", get(preview))
        .route("/api/v1/devices/{id}/assignment", put(control::assignment))
        .route(
            "/api/v1/simulation/devices/{id}/fault",
            post(control::fault),
        )
        .route(
            "/api/v1/calibration/{id}",
            axum::routing::delete(control::delete),
        )
        .route("/api/v1/sessions", get(control::sessions))
        .route("/api/v1/recording/start", post(control::record_start))
        .route("/api/v1/recording/stop", post(control::record_stop))
        .route("/api/v1/playback/start", post(control::playback_start))
        .route("/api/v1/playback", put(control::playback_update))
        .route("/api/v1/playback/stop", post(control::playback_stop))
        .fallback(|| async {
            failure(
                ApiErrorCode::NotFound,
                "This endpoint is not available in Stage 1.",
            )
        })
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(api.clone(), origin))
        .with_state(api)
}
async fn health() -> impl IntoResponse {
    Json(ApiResponse::ok(HealthResponse::default()))
}
async fn state(State(api): State<Api>) -> impl IntoResponse {
    Json(ApiResponse::ok(api.streams.state.borrow().clone()))
}
async fn skeleton(State(api): State<Api>) -> impl IntoResponse {
    Json(ApiResponse::ok(api.streams.skeleton.borrow().clone()))
}
async fn simulation(
    State(api): State<Api>,
    body: Result<Json<SimulationUpdateRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(request)) = body else {
        return failure(
            ApiErrorCode::InvalidRequest,
            "Provide a valid JSON simulation request with Content-Type: application/json.",
        );
    };
    match api.client.simulation(request).await {
        Ok(data) => Json(ApiResponse::ok(data)).into_response(),
        Err(e) => control_error(e),
    }
}
async fn capture(
    State(api): State<Api>,
    body: Result<Json<ReferencePoseRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(request)) = body else {
        return failure(
            ApiErrorCode::InvalidRequest,
            "Provide valid tracker identifiers in a JSON reference-pose request.",
        );
    };
    match api.client.capture(request).await {
        Ok(data) => Json(ApiResponse::ok(data)).into_response(),
        Err(e) => control_error(e),
    }
}
/// Newest camera frame as JPEG, downscaled; refreshed at most every 100 ms.
async fn preview(
    State(api): State<Api>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let jpeg = DeviceId::new(id.as_str())
        .ok()
        .and_then(|id| api.previews.read().ok()?.get(&id).cloned());
    match jpeg {
        Some(jpeg) => (
            [
                (header::CONTENT_TYPE, "image/jpeg"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            jpeg,
        )
            .into_response(),
        None => failure(
            ApiErrorCode::NotFound,
            "No preview: the device is not a physical camera or has not delivered a frame yet.",
        ),
    }
}
async fn import_profiles(
    State(api): State<Api>,
    body: Result<Json<Vec<CalibrationProfile>>, JsonRejection>,
) -> Response {
    let Ok(Json(profiles)) = body else {
        return failure(
            ApiErrorCode::InvalidRequest,
            "Provide a JSON array of camera calibration profiles.",
        );
    };
    match api.client.import(profiles).await {
        Ok(data) => Json(ApiResponse::ok(data)).into_response(),
        Err(e) => control_error(e),
    }
}
pub fn failure(code: ApiErrorCode, message: &str) -> Response {
    (
        StatusCode::from_u16(code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        Json(ApiResponse::<()>::err(ApiError {
            code,
            message: message.into(),
            details: None,
        })),
    )
        .into_response()
}
fn control_error(e: ControlError) -> Response {
    failure(e.code, &e.message)
}
async fn origin(State(api): State<Api>, request: Request, next: Next) -> Response {
    let origin = request.headers().get(header::ORIGIN).cloned();
    if let Some(value) = origin.as_ref()
        && value
            .to_str()
            .ok()
            .is_none_or(|o| !api.allowed_origins.iter().any(|a| a == o))
    {
        return failure(
            ApiErrorCode::ForbiddenOrigin,
            "This origin is not permitted to access the local engine.",
        );
    }
    let mut response = if request.method() == Method::OPTIONS {
        StatusCode::NO_CONTENT.into_response()
    } else {
        next.run(request).await
    };
    if let Some(value) = origin {
        response
            .headers_mut()
            .insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
        response
            .headers_mut()
            .insert(header::VARY, HeaderValue::from_static("Origin"));
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_METHODS,
            HeaderValue::from_static("GET, PUT, POST, DELETE, OPTIONS"),
        );
        response.headers_mut().insert(
            header::ACCESS_CONTROL_ALLOW_HEADERS,
            HeaderValue::from_static("Content-Type"),
        );
    }
    response
}
