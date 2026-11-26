//! HTTP boundary: only pipeline commands and shared contracts cross into the engine.
mod control;
mod inspection;
mod training;
mod ws;
pub mod xr;
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
    pub fiducials: crate::sources::camera::FiducialPreviews,
    pub camera_settings: crate::sources::camera::CameraSettings,
    pub runtime: crate::sources::RuntimeMap,
    /// Frames of network cameras (phones), by camera id (ADR-042).
    pub feeds: crate::sources::camera::capture::NetworkFeeds,
    /// LAN mode token; `None` when LAN mode is off (loopback only).
    pub lan_token: Option<Arc<str>>,
    pub bind: std::net::SocketAddr,
    /// Latest `diagnostics` message; `None` while timing is off or before the first window.
    pub diagnostics: tokio::sync::watch::Receiver<Option<Arc<crate::pipeline::encoded::Encoded>>>,
    pub training: crate::training::TrainingClient,
    pub training_streams: crate::training::TrainingStreams,
    pub content: Arc<crate::content_store::ContentStore>,
    pub store: Option<crate::store::Writer>,
    pub data_dir: std::path::PathBuf,
    pub progress: crate::config::ProgressSection,
    /// OpenXR pose ingest; `None` when `xr.enabled` is false (the route answers 404).
    pub xr: Option<Arc<xr::XrIngress>>,
}
pub fn router(api: Api) -> Router {
    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/state", get(state))
        .route("/api/v1/devices/{id}/inspection", get(inspection::snapshot))
        .route("/api/v1/devices/{id}/inspection/batch", get(inspection::batch))
        .route("/api/v1/inspection/solver", post(inspection::solver))
        .route("/api/v1/skeleton", get(skeleton))
        .route("/api/v1/simulation", put(simulation))
        .route("/api/v1/calibration/reference-pose", post(capture))
        .route("/api/v1/calibration/profiles", post(import_profiles))
        .route("/api/v1/stream", get(ws::upgrade))
        .route("/api/v1/cameras/{id}/preview.jpg", get(preview))
        .route("/api/v1/cameras/{id}/fiducials", get(fiducials))
        .route("/api/v1/cameras/{id}/settings", put(camera_settings))
        .route(
            "/api/v1/cameras/{id}/identity/confirm",
            post(confirm_camera_identity),
        )
        .route("/api/v1/lan/pairing", get(lan_pairing))
        .route("/api/v1/ingest/camera/{id}", get(ingest_camera))
        .route("/api/v1/ingest/xr", get(xr::upgrade))
        .route("/api/v1/devices/{id}/assignment", put(control::assignment))
        .route("/api/v1/rig", put(control::rig))
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
        .route("/api/v1/content/index", get(training::content_index))
        .route("/api/v1/content/i18n/{lang}", get(training::i18n))
        .route("/api/v1/content/{kind}/{id}", get(training::content_get))
        .route(
            "/api/v1/athletes",
            get(training::athletes).post(training::athlete_create),
        )
        .route("/api/v1/athletes/active", put(training::athlete_active))
        .route(
            "/api/v1/athletes/{id}",
            get(training::athlete_get).put(training::athlete_put),
        )
        .route(
            "/api/v1/athletes/{id}/consent",
            post(training::athlete_consent),
        )
        .route(
            "/api/v1/training/sessions",
            get(training::sessions).post(training::session_create),
        )
        .route("/api/v1/training/state", get(training::training_state))
        .route("/api/v1/training/command", post(training::training_command))
        .route(
            "/api/v1/training/sessions/{id}",
            get(training::session_get).delete(training::session_delete),
        )
        .route(
            "/api/v1/training/sessions/{id}/events",
            get(training::session_events),
        )
        .route(
            "/api/v1/training/sessions/{id}/reps",
            get(training::session_reps),
        )
        .route(
            "/api/v1/training/sessions/{id}/provenance",
            get(training::session_provenance),
        )
        .route(
            "/api/v1/training/sessions/{id}/rpe",
            post(training::session_rpe),
        )
        .route(
            "/api/v1/training/sessions/{id}/reanalyse",
            post(training::session_reanalyse),
        )
        .route("/api/v1/training/reps/{id}/{rep}", get(training::rep_get))
        .route(
            "/api/v1/training/sessions/{id}/clinic-review",
            get(training::clinic_review),
        )
        .route(
            "/api/v1/training/reps/{id}/{rep}/trace",
            get(training::rep_trace),
        )
        .route(
            "/api/v1/training/reps/{id}/{rep}/motion",
            get(training::rep_motion),
        )
        .route("/api/v1/progress/series", get(training::progress_series))
        .route("/api/v1/progress/summary", get(training::progress_summary))
        .fallback(|| async {
            failure(
                ApiErrorCode::NotFound,
                "This endpoint is not available in Stage 1.",
            )
        })
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(api.clone(), origin))
        .layer(middleware::from_fn_with_state(api.clone(), lan_auth))
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
async fn fiducials(
    State(api): State<Api>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let value = DeviceId::new(id.as_str()).ok().and_then(|id| {
        let all = api.fiducials.read().ok()?;
        let (seen, data) = all.get(&id)?;
        (seen.elapsed() <= std::time::Duration::from_millis(500)).then(|| data.clone())
    });
    match value {
        Some(data) => Json(ApiResponse::ok(data)).into_response(),
        None => failure(
            ApiErrorCode::NotFound,
            "No fresh AprilTag observations for this camera.",
        ),
    }
}
/// `POST /cameras/{id}/identity/confirm`: the user states that the connected hardware is this
/// camera, so its calibration is used again. Only after a deliberate act, never automatically.
async fn confirm_camera_identity(
    State(api): State<Api>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let Ok(id) = DeviceId::new(id.as_str()) else {
        return failure(ApiErrorCode::NotFound, "No camera with this id.");
    };
    let (dir, runtime) = (api.data_dir.clone(), api.runtime.clone());
    let done = tokio::task::spawn_blocking(move || {
        crate::sources::camera::lifecycle::Lifecycle::confirm(&dir, &runtime, &id)
    })
    .await;
    match done {
        Ok(Ok(())) => Json(ApiResponse::ok(true)).into_response(),
        Ok(Err(e)) => failure(ApiErrorCode::Unavailable, &e),
        Err(_) => failure(ApiErrorCode::Internal, "Identity could not be saved."),
    }
}
/// `PUT /cameras/{id}/settings` (14 §5.5): manual exposure and gain, applied by the capture thread
/// on its next frame. Answers the camera's status once frames taken after the change arrive.
async fn camera_settings(
    State(api): State<Api>,
    axum::extract::Path(id): axum::extract::Path<String>,
    body: Result<Json<CameraSettingsRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(request)) = body else {
        return failure(
            ApiErrorCode::InvalidRequest,
            "Provide a JSON object with exposure_s and/or gain.",
        );
    };
    let Ok(id) = DeviceId::new(id.as_str()) else {
        return failure(ApiErrorCode::NotFound, "No camera with this id.");
    };
    if request.resolution_px.is_some() || request.rate_hz.is_some() {
        return failure(
            ApiErrorCode::InvalidRequest,
            "resolution_px and rate_hz are changed in the configuration with a restart: camera calibration depends on them.",
        );
    }
    if let Some(e) = request.exposure_s
        && !(e.is_finite() && (1e-5..=1.0).contains(&e))
    {
        return failure(
            ApiErrorCode::InvalidRequest,
            "exposure_s must be between 0.00001 and 1 second.",
        );
    }
    if request.gain.is_some_and(|g| !g.is_finite()) {
        return failure(ApiErrorCode::InvalidRequest, "gain must be a number.");
    }
    let changed = api.camera_settings.write().ok().and_then(|mut all| {
        let s = all.get_mut(&id)?;
        s.generation += 1;
        s.exposure_s = request.exposure_s.or(s.exposure_s);
        s.gain = request.gain.or(s.gain);
        Some(())
    });
    if changed.is_none() {
        return failure(
            ApiErrorCode::NotFound,
            "No physical or image-folder camera with this id.",
        );
    }
    let seen = |api: &Api| {
        api.streams
            .state
            .borrow()
            .devices
            .iter()
            .find(|d| d.descriptor.id == id)
            .cloned()
    };
    let before = seen(&api).and_then(|d| d.stream.last_seen_t_us);
    // Two frames after the change carry the new settings; a stalled camera answers after 2 s.
    for _ in 0..20 {
        if let (Some(a), Some(b)) = (before, seen(&api).and_then(|d| d.stream.last_seen_t_us))
            && b > a + 70_000
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    match seen(&api) {
        Some(status) => Json(ApiResponse::ok(status)).into_response(),
        None => failure(ApiErrorCode::NotFound, "No camera with this id."),
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
/// Peers on this machine; requests served without connection info (in-process) count as local.
fn local_peer(request: &Request) -> bool {
    request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .is_none_or(|c| c.0.ip().is_loopback())
}

/// LAN mode (ADR-031, ADR-042): a request from another machine needs the bearer token; the
/// engine never falls back to open access (17 §5).
async fn lan_auth(State(api): State<Api>, request: Request, next: Next) -> Response {
    let Some(token) = api.lan_token.as_deref() else {
        return next.run(request).await;
    };
    if local_peer(&request) {
        return next.run(request).await;
    }
    let given = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    if given.is_some_and(|g| crate::lan::matches(g.trim(), token)) {
        next.run(request).await
    } else {
        failure(
            ApiErrorCode::Unauthorized,
            "This engine is in LAN mode: pair the device with the code shown on the desktop (Devices).",
        )
    }
}

/// `GET /ingest/camera/{id}` (ADR-042): a phone streams one JPEG per binary WebSocket message to
/// a camera configured with `network: true`. The engine sends nothing; closing the socket (or
/// 2 s without a frame) makes the camera `disconnected`.
async fn ingest_camera(
    State(api): State<Api>,
    axum::extract::Path(id): axum::extract::Path<String>,
    ws: Result<
        axum::extract::WebSocketUpgrade,
        axum::extract::ws::rejection::WebSocketUpgradeRejection,
    >,
) -> Response {
    let Some(feed) = api.feeds.get(&id).cloned() else {
        return failure(
            ApiErrorCode::NotFound,
            "No camera with this id is configured with network: true.",
        );
    };
    let Ok(ws) = ws else {
        return failure(
            ApiErrorCode::InvalidRequest,
            "A WebSocket upgrade is required.",
        );
    };
    let cancel = api.cancel.clone();
    ws.max_message_size(8 << 20)
        .on_upgrade(move |mut socket| async move {
            use axum::extract::ws::Message;
            loop {
                let message = tokio::select! {
                    _ = cancel.cancelled() => break,
                    m = socket.recv() => m,
                };
                match message {
                    Some(Ok(Message::Binary(jpeg))) => feed.push(jpeg),
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(_)) => {}
                }
            }
        })
        .into_response()
}

/// `GET /lan/pairing` (ADR-042): the pairing link for the desktop to show as text and QR code.
/// Answered only to this machine, so the token never leaves it except by the user's hand.
async fn lan_pairing(State(api): State<Api>, request: Request) -> Response {
    if !local_peer(&request) {
        return failure(
            ApiErrorCode::ForbiddenOrigin,
            "Pairing is shown only on the engine's own machine.",
        );
    }
    let Some(token) = api.lan_token.as_deref() else {
        return Json(ApiResponse::ok(serde_json::json!({
            "enabled": false,
            "message": "LAN mode is off. Set api.bind to 0.0.0.0:7700 and api.lan.enabled: true, then restart.",
        })))
        .into_response();
    };
    let host = crate::lan::lan_address(api.bind).map(|ip| ip.to_string());
    let port = api.bind.port();
    let uri = host
        .as_ref()
        .map(|h| format!("bansayo://pair?host={h}&port={port}&token={token}"));
    Json(ApiResponse::ok(serde_json::json!({
        "enabled": true,
        "host": host,
        "port": port,
        "token": token,
        "uri": uri,
        "warning": "LAN mode has no encryption: use it on a trusted home network only.",
    })))
    .into_response()
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
            HeaderValue::from_static("Content-Type, Authorization"),
        );
    }
    response
}
