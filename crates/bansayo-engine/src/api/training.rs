//! Training, content, athlete and progress routes (martial-arts 14 §5.1–5.4).
use super::{Api, failure};
use crate::pipeline::command::ControlError;
use crate::store::{Write, read};
use crate::training::Request;
use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection},
    response::{IntoResponse, Response},
};
use bansayo_contracts::*;
use serde::Deserialize;
use serde_json::{Value, json};

fn ok<T: serde::Serialize>(data: T) -> Response {
    Json(ApiResponse::ok(data)).into_response()
}

fn err(e: ControlError) -> Response {
    failure(e.code, &e.message)
}

fn bad(message: &str) -> Response {
    failure(ApiErrorCode::InvalidRequest, message)
}

fn not_found(message: &str) -> Response {
    failure(ApiErrorCode::NotFound, message)
}

fn no_store() -> Response {
    failure(
        ApiErrorCode::Unavailable,
        "The analytics store is unavailable.",
    )
}

/// Runs a store query off the async runtime.
async fn query<T: Send + 'static>(
    api: &Api,
    f: impl FnOnce(&rusqlite::Connection) -> rusqlite::Result<T> + Send + 'static,
) -> Result<T, Response> {
    let Some(w) = api.store.clone() else {
        return Err(no_store());
    };
    tokio::task::spawn_blocking(move || {
        let c = w.read().map_err(|e| e.to_string())?;
        f(&c).map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| no_store())?
    .map_err(|e| {
        tracing::error!(%e, "analytics query failed");
        no_store()
    })
}

// ---- content -------------------------------------------------------------

pub async fn content_index(State(api): State<Api>) -> Response {
    ok(api.content.index())
}

#[derive(Deserialize)]
pub struct VersionQuery {
    version: Option<u32>,
}

pub async fn content_get(
    State(api): State<Api>,
    Path((kind, id)): Path<(String, String)>,
    Query(q): Query<VersionQuery>,
) -> Response {
    if !crate::content_store::KINDS.contains(&kind.as_str()) {
        return not_found("Unknown content kind.");
    }
    let current = api.content.get(&kind, &id).filter(|v| {
        q.version
            .is_none_or(|ver| v["version"].as_u64() == Some(u64::from(ver)))
    });
    if let Some(v) = current {
        return ok(v.clone());
    }
    let Some(version) = q.version else {
        return not_found("No valid definition with this id.");
    };
    // An older version is served from the snapshot stored with the sessions that used it (I3).
    match query(&api, move |c| {
        c.query_row(
            "SELECT body_json FROM content_snapshots WHERE kind = ?1 AND id = ?2 AND version = ?3 LIMIT 1",
            rusqlite::params![kind, id, version],
            |r| r.get::<_, String>(0),
        )
        .map(Some)
        .or_else(|e| if e == rusqlite::Error::QueryReturnedNoRows { Ok(None) } else { Err(e) })
    })
    .await
    {
        Ok(Some(body)) => ok(serde_json::from_str::<Value>(&body).unwrap_or(Value::Null)),
        Ok(None) => not_found("No definition with this id and version."),
        Err(r) => r,
    }
}

pub async fn i18n(State(api): State<Api>, Path(lang): Path<String>) -> Response {
    match api.content.i18n.get(&lang) {
        Some(m) => ok(m.clone()),
        None => not_found("No texts in this language."),
    }
}

// ---- athletes ------------------------------------------------------------

pub async fn athletes(State(api): State<Api>) -> Response {
    match api.training.ask(Request::ListAthletes).await {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

pub async fn athlete_create(
    State(api): State<Api>,
    body: Result<Json<CreateAthleteRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(r)) = body else {
        return bad(
            "Provide an id (a-z 0-9 _ -, at most 32), a display name and a height in metres.",
        );
    };
    match api.training.ask(|tx| Request::CreateAthlete(r, tx)).await {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

fn athlete_id(id: String) -> Result<AthleteId, Box<Response>> {
    AthleteId::new(id)
        .map_err(|_| Box::new(bad("Athlete ids are 1-32 characters from a-z 0-9 _ -.")))
}

pub async fn athlete_get(State(api): State<Api>, Path(id): Path<String>) -> Response {
    let id = match athlete_id(id) {
        Ok(id) => id,
        Err(r) => return *r,
    };
    match api.training.ask(|tx| Request::GetAthlete(id, tx)).await {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

pub async fn athlete_put(
    State(api): State<Api>,
    Path(id): Path<String>,
    body: Result<Json<AthleteProfile>, JsonRejection>,
) -> Response {
    let (Ok(id), Ok(Json(p))) = (athlete_id(id), body) else {
        return bad("Provide the complete athlete profile as JSON.");
    };
    match api
        .training
        .ask(|tx| Request::UpdateAthlete(id, Box::new(p), tx))
        .await
    {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

pub async fn athlete_active(
    State(api): State<Api>,
    body: Result<Json<ActiveAthleteRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(r)) = body else {
        return bad("Provide { \"athlete_id\": <id or null> }.");
    };
    match api
        .training
        .ask(|tx| Request::SetActive(r.athlete_id, tx))
        .await
    {
        // `SystemState` follows within one state publication.
        Ok(a) => ok(json!({ "athlete": a, "state": *api.streams.state.borrow() })),
        Err(e) => err(e),
    }
}

pub async fn athlete_consent(
    State(api): State<Api>,
    Path(id): Path<String>,
    body: Result<Json<ConsentRequest>, JsonRejection>,
) -> Response {
    let (Ok(id), Ok(Json(r))) = (athlete_id(id), body) else {
        return bad("Provide { \"motion_data\": true|false, \"video\": true|false }.");
    };
    match api.training.ask(|tx| Request::Consent(id, r, tx)).await {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

// ---- training ------------------------------------------------------------

pub async fn session_create(
    State(api): State<Api>,
    body: Result<Json<CreateSessionRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(r)) = body else {
        return bad(
            "Provide a technique plan with a technique id and side, or a workout plan with a workout id.",
        );
    };
    match api.training.ask(|tx| Request::CreateSession(r, tx)).await {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

pub async fn training_state(State(api): State<Api>) -> Response {
    ok(api.training_streams.state.borrow().as_deref().cloned())
}

pub async fn training_command(
    State(api): State<Api>,
    body: Result<Json<TrainingCommandRequest>, JsonRejection>,
) -> Response {
    let Ok(Json(r)) = body else {
        return bad("Provide { \"command\": <name>, \"args\": {…} }.");
    };
    match api.training.ask(|tx| Request::Command(r, tx)).await {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

#[derive(Deserialize)]
pub struct SessionsQuery {
    athlete_id: Option<String>,
    from: Option<String>,
    to: Option<String>,
    plan_kind: Option<String>,
    limit: Option<u32>,
}

pub async fn sessions(State(api): State<Api>, Query(q): Query<SessionsQuery>) -> Response {
    let f = read::SessionFilter {
        athlete_id: q.athlete_id,
        from: q.from,
        to: q.to,
        plan_kind: q.plan_kind,
        limit: q.limit,
    };
    match query(&api, move |c| read::sessions(c, &f)).await {
        Ok(v) => ok(v),
        Err(r) => r,
    }
}

fn session_id(id: &str) -> Result<SessionId, Box<Response>> {
    SessionId::new(id)
        .map_err(|_| Box::new(bad("Session ids have the form 20261116T102205Z-a3f9.")))
}

pub async fn session_get(State(api): State<Api>, Path(id): Path<String>) -> Response {
    let id = match session_id(&id) {
        Ok(id) => id,
        Err(r) => return *r,
    };
    match query(&api, move |c| read::session(c, &id)).await {
        Ok(Some(v)) => ok(v),
        Ok(None) => not_found("No stored session with this id."),
        Err(r) => r,
    }
}

pub async fn session_delete(State(api): State<Api>, Path(id): Path<String>) -> Response {
    let id = match session_id(&id) {
        Ok(id) => id,
        Err(r) => return *r,
    };
    let running = api
        .training_streams
        .state
        .borrow()
        .as_ref()
        .is_some_and(|s| {
            s.session_id == id
                && !matches!(s.phase, SessionPhase::Completed | SessionPhase::Aborted)
        });
    if running {
        return failure(
            ApiErrorCode::Conflict,
            "End the session before deleting it.",
        );
    }
    let Some(w) = api.store.clone() else {
        return no_store();
    };
    let dir = api.data_dir.join("sessions").join(id.as_str());
    let (tx, rx) = tokio::sync::oneshot::channel();
    let sent = tokio::task::spawn_blocking(move || w.send(Write::Delete(id, tx)))
        .await
        .unwrap_or(false);
    match (sent, rx.await) {
        (true, Ok(Ok(true))) => {
            let removed = tokio::task::spawn_blocking(move || {
                if dir.exists() {
                    std::fs::remove_dir_all(&dir)
                } else {
                    Ok(())
                }
            })
            .await;
            if !matches!(removed, Ok(Ok(()))) {
                tracing::warn!("session files could not be removed");
            }
            ok(DeletedResponse::default())
        }
        (true, Ok(Ok(false))) => not_found("No stored session with this id."),
        _ => no_store(),
    }
}

#[derive(Deserialize)]
pub struct EventsQuery {
    after_seq: Option<u64>,
    limit: Option<u32>,
}

pub async fn session_events(
    State(api): State<Api>,
    Path(id): Path<String>,
    Query(q): Query<EventsQuery>,
) -> Response {
    let id = match session_id(&id) {
        Ok(id) => id,
        Err(r) => return *r,
    };
    let (after, limit) = (
        q.after_seq.unwrap_or(0),
        q.limit.unwrap_or(1000).min(10_000),
    );
    let live = api
        .training
        .ask(|tx| Request::Events(id.clone(), after, limit, tx))
        .await;
    if let Ok(Some(events)) = live {
        return ok(events);
    }
    match query(&api, move |c| read::events(c, &id, after, limit)).await {
        Ok(v) => ok(v),
        Err(r) => r,
    }
}

pub async fn session_reps(State(api): State<Api>, Path(id): Path<String>) -> Response {
    let id = match session_id(&id) {
        Ok(id) => id,
        Err(r) => return *r,
    };
    match query(&api, move |c| read::reps(c, &id)).await {
        Ok(v) => ok(v),
        Err(r) => r,
    }
}

pub async fn clinic_review(State(api): State<Api>, Path(id): Path<String>) -> Response {
    let id = match session_id(&id) {
        Ok(id) => id,
        Err(r) => return *r,
    };
    match query(&api, move |c| read::clinic_review(c, &id)).await {
        Ok(review) => ok(review),
        Err(r) => r,
    }
}

pub async fn rep_get(State(api): State<Api>, Path((id, rep)): Path<(String, u32)>) -> Response {
    let id = match session_id(&id) {
        Ok(id) => id,
        Err(r) => return *r,
    };
    match query(&api, move |c| read::reps(c, &id)).await {
        Ok(v) => match v.into_iter().find(|r| r.repetition.rep_index == rep) {
            Some(r) => ok(r),
            None => not_found("No repetition with this index."),
        },
        Err(r) => r,
    }
}

pub async fn session_provenance(State(api): State<Api>, Path(id): Path<String>) -> Response {
    let id = match session_id(&id) {
        Ok(id) => id,
        Err(r) => return *r,
    };
    match query(&api, move |c| read::provenance(c, &id)).await {
        Ok(Some(v)) => ok(v),
        Ok(None) => not_found("No stored session with this id."),
        Err(r) => r,
    }
}

pub async fn session_rpe(
    State(api): State<Api>,
    Path(id): Path<String>,
    body: Result<Json<RpeRequest>, JsonRejection>,
) -> Response {
    let (Ok(id), Ok(Json(r))) = (session_id(&id), body) else {
        return bad("Provide { \"rpe\": 1..10 }.");
    };
    if !(1..=10).contains(&r.rpe) {
        return bad("RPE is between 1 and 10.");
    }
    let Some(w) = api.store.clone() else {
        return no_store();
    };
    let (tx, rx) = tokio::sync::oneshot::channel();
    let sent = tokio::task::spawn_blocking(move || w.send(Write::Rpe(id, r.rpe, tx)))
        .await
        .unwrap_or(false);
    match (sent, rx.await) {
        (true, Ok(Ok(()))) => ok(json!({ "ok": true })),
        (true, Ok(Err(_))) => failure(
            ApiErrorCode::Conflict,
            "RPE can be recorded only for a finished, stored session.",
        ),
        _ => no_store(),
    }
}

// ---- progress ------------------------------------------------------------

#[derive(Deserialize)]
pub struct SeriesParams {
    athlete_id: String,
    technique: String,
    metric: String,
    side: Option<String>,
    class: Option<String>,
    from: Option<String>,
    to: Option<String>,
    origin: Option<String>,
}

fn direction(api: &Api, metric: &str) -> String {
    api.content
        .library
        .metrics
        .get(metric)
        .and_then(|m| serde_json::to_value(m.direction).ok())
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "neutral".into())
}

fn progress_config(api: &Api) -> bansayo_training::progress::ProgressConfig {
    bansayo_training::progress::ProgressConfig {
        baseline_sessions: api.progress.baseline_sessions as usize,
        baseline_window_days: api.progress.baseline_window_days,
        trend_sessions: api.progress.trend_sessions as usize,
    }
}

pub async fn progress_series(State(api): State<Api>, Query(p): Query<SeriesParams>) -> Response {
    if AthleteId::new(p.athlete_id.as_str()).is_err() {
        return bad("Provide a valid athlete_id.");
    }
    if p.origin
        .as_deref()
        .is_some_and(|o| !matches!(o, "live" | "simulated" | "fixture"))
    {
        return bad("origin is live, simulated or fixture.");
    }
    let dir = direction(&api, &p.metric);
    let cfg = progress_config(&api);
    let q = read::SeriesQuery {
        athlete_id: p.athlete_id,
        technique: p.technique,
        metric: p.metric,
        side: p.side,
        class: p.class,
        from: p.from,
        to: p.to,
        origin: p.origin,
    };
    match query(&api, move |c| read::progress_series(c, &q, &dir, &cfg)).await {
        Ok(v) => ok(v),
        Err(r) => r,
    }
}

#[derive(Deserialize)]
pub struct SummaryParams {
    athlete_id: String,
    origin: Option<String>,
}

pub async fn progress_summary(State(api): State<Api>, Query(p): Query<SummaryParams>) -> Response {
    let Ok(athlete) = AthleteId::new(p.athlete_id.as_str()) else {
        return bad("Provide a valid athlete_id.");
    };
    let directions: std::collections::BTreeMap<String, String> = api
        .content
        .library
        .metrics
        .keys()
        .map(|m| (m.clone(), direction(&api, m)))
        .collect();
    let cfg = progress_config(&api);
    let origin = p.origin.clone();
    let result = query(&api, move |c| {
        let mut by: std::collections::BTreeMap<String, TechniqueSummary> = Default::default();
        for (technique, metric, side) in
            read::athlete_series(c, athlete.as_str(), origin.as_deref())?
        {
            let q = read::SeriesQuery {
                athlete_id: athlete.to_string(),
                technique: technique.clone(),
                metric: metric.clone(),
                side: Some(side),
                origin: origin.clone(),
                ..Default::default()
            };
            let dir = directions
                .get(&metric)
                .cloned()
                .unwrap_or_else(|| "neutral".into());
            let s = read::progress_series(c, &q, &dir, &cfg)?;
            for class in s.classes {
                let Some(latest) = class.points.last().cloned() else {
                    continue;
                };
                let entry = by
                    .entry(technique.clone())
                    .or_insert_with(|| TechniqueSummary {
                        technique: technique.clone(),
                        last_session_id: latest.session_id.clone(),
                        last_session_utc: latest.date_utc.clone(),
                        sessions: 0,
                        headline: vec![],
                        new_records: vec![],
                    });
                if latest.date_utc > entry.last_session_utc {
                    entry.last_session_id = latest.session_id.clone();
                    entry.last_session_utc = latest.date_utc.clone();
                }
                entry.sessions = entry.sessions.max(class.points.len() as u32);
                if class
                    .best
                    .as_ref()
                    .is_some_and(|b| b.session_id == latest.session_id)
                {
                    entry.new_records.push(metric.clone());
                }
                entry.headline.push(HeadlineMetric {
                    metric: metric.clone(),
                    comparability_class: class.comparability_class,
                    latest,
                    classification: class.classification,
                });
            }
        }
        Ok(ProgressSummary {
            athlete_id: athlete,
            techniques: by.into_values().collect(),
        })
    })
    .await;
    match result {
        Ok(v) => ok(v),
        Err(r) => r,
    }
}

// ---- review (motion.mcap, indexed) ------------------------------------------

async fn rep_window(api: &Api, id: &SessionId, rep: u32) -> Result<RepetitionResult, Response> {
    let sid = id.clone();
    let reps = query(api, move |c| read::reps(c, &sid)).await?;
    reps.into_iter()
        .find(|r| r.repetition.rep_index == rep)
        .ok_or_else(|| not_found("No repetition with this index."))
}

#[derive(Deserialize)]
pub struct TraceQuery {
    observables: Option<String>,
}

pub async fn rep_trace(
    State(api): State<Api>,
    Path((id, rep)): Path<(String, u32)>,
    Query(q): Query<TraceQuery>,
) -> Response {
    let id = match session_id(&id) {
        Ok(id) => id,
        Err(r) => return *r,
    };
    let r = match rep_window(&api, &id, rep).await {
        Ok(r) => r,
        Err(r) => return r,
    };
    let side = if r.repetition.side == Side::Left {
        "left"
    } else {
        "right"
    };
    let ids: Vec<String> = match q.observables {
        Some(list) => list
            .split(',')
            .filter(|s| !s.is_empty())
            .take(32)
            .map(String::from)
            .collect(),
        None => [
            "knee_height",
            "ankle_height",
            "ankle_speed",
            "knee_flexion",
            "knee_extension_rate",
            "thigh_elevation",
        ]
        .iter()
        .map(|n| format!("{n}.{side}"))
        .collect(),
    };
    let path = api
        .data_dir
        .join("sessions")
        .join(id.as_str())
        .join("motion.mcap");
    let (t0, t1) = (r.repetition.t_start_us, r.repetition.t_end_us);
    let rows = tokio::task::spawn_blocking(move || {
        crate::recording::motion::read_range(&path, crate::recording::motion::OBSERVABLES, t0, t1)
    })
    .await;
    let rows = match rows {
        Ok(Ok(rows)) => rows,
        _ => return not_found("Motion was not recorded for this session or has expired."),
    };
    let mut trace = RepTrace {
        t_us: vec![],
        series: ids
            .iter()
            .map(|i| TraceSeries {
                id: i.clone(),
                values: vec![],
                method: vec![],
            })
            .collect(),
    };
    for (t, bytes) in rows {
        let Ok(v) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        let values: Vec<ObservableValue> =
            serde_json::from_value(v["values"].clone()).unwrap_or_default();
        trace.t_us.push(t);
        for s in &mut trace.series {
            let o = values.iter().find(|x| x.id == s.id);
            s.values.push(o.and_then(|o| o.value));
            s.method
                .push(o.filter(|o| o.value.is_some()).map(|o| o.evidence.method));
        }
    }
    ok(trace)
}

pub async fn rep_motion(State(api): State<Api>, Path((id, rep)): Path<(String, u32)>) -> Response {
    let id = match session_id(&id) {
        Ok(id) => id,
        Err(r) => return *r,
    };
    let r = match rep_window(&api, &id, rep).await {
        Ok(r) => r,
        Err(r) => return r,
    };
    let path = api
        .data_dir
        .join("sessions")
        .join(id.as_str())
        .join("motion.mcap");
    let (t0, t1) = (r.repetition.t_start_us, r.repetition.t_end_us);
    let rows = tokio::task::spawn_blocking(move || {
        crate::recording::motion::read_range(&path, crate::recording::motion::FRAME, t0, t1)
    })
    .await;
    let rows = match rows {
        Ok(Ok(rows)) => rows,
        _ => return not_found("Motion was not recorded for this session or has expired."),
    };
    let ankle = BODY_KEYPOINT_NAMES
        .iter()
        .position(|n| {
            *n == if r.repetition.side == Side::Left {
                "left_ankle"
            } else {
                "right_ankle"
            }
        })
        .unwrap_or(0);
    let mut clip = MotionClip {
        rate_hz: 60,
        duration_us: t1 - t0,
        skeleton_rev: 0,
        samples: vec![],
        phases: r.repetition.phases.clone(),
        strike_t_us: r.repetition.strike_t_us,
        paths: vec![KeypointPath {
            keypoint: BODY_KEYPOINT_NAMES[ankle].into(),
            points_m: vec![],
        }],
    };
    // Segment origins come from the engine's forward kinematics so clients draw without any.
    // shortcut: uses the current skeleton definition, not the one recorded with the session;
    // upgrade when skeleton definitions are stored per session.
    let definition = api.streams.skeleton.borrow().clone();
    let skeleton =
        crate::skeleton::Skeleton::builtin(&definition.model_id, f64::from(definition.height_m))
            .ok();
    for (_, bytes) in rows {
        let Ok(s) = serde_json::from_slice::<MotionSample>(&bytes) else {
            continue;
        };
        let Some(b) = s.body else { continue };
        clip.skeleton_rev = b.skeleton_rev;
        let rotations: Option<Vec<_>> = b
            .segments
            .iter()
            .map(|g| crate::math::rot_from_wire(g.rotation))
            .collect();
        let positions_m = match (&skeleton, rotations) {
            (Some(sk), Some(r)) if r.len() == sk.len() => crate::skeleton::forward_kinematics(
                sk,
                &r,
                crate::math::v3_from_wire(b.root.position_m),
            )
            .iter()
            .map(crate::math::v3_to_wire)
            .collect(),
            _ => continue,
        };
        clip.samples.push(ClipSample {
            t_us: s.t_us,
            root_m: b.root.position_m,
            rotations: b.segments.iter().map(|g| g.rotation).collect(),
            positions_m,
            inferred: b
                .segments
                .iter()
                .map(|g| {
                    !matches!(
                        g.axis.provenance,
                        Provenance::Measured | Provenance::Derived
                    )
                })
                .collect(),
        });
        if let Some(Some(k)) = b.keypoints.get(ankle) {
            clip.paths[0].points_m.push(k.position_m);
        }
    }
    ok(clip)
}

// ---- re-analysis ---------------------------------------------------------------

/// Starts a re-analysis of a stored session (12 §7.2) on the blocking pool; the result is a new
/// session row linked by `reanalysis_of`, whose id is the job id.
pub async fn session_reanalyse(
    State(api): State<Api>,
    Path(id): Path<String>,
    body: Result<Json<ReanalyseRequest>, JsonRejection>,
) -> Response {
    let (Ok(id), Ok(Json(r))) = (session_id(&id), body) else {
        return bad("Provide { \"content\": \"pinned\", \"source_mask\": [device ids] }.");
    };
    if r.content == ReanalyseContent::Current {
        return bad("Re-analysis with current content is not available yet; use pinned.");
    }
    let sid = id.clone();
    let loaded = query(&api, move |c| {
        Ok((
            read::session(c, &sid)?,
            read::events(c, &sid, 0, 1_000_000)?,
            read::snapshots(c, &sid)?,
        ))
    })
    .await;
    let (detail, events, snapshots) = match loaded {
        Ok((Some(d), e, s)) => (d, e, s),
        Ok((None, ..)) => return not_found("No stored session with this id."),
        Err(r) => return r,
    };
    // Until the session row is closed its event log may still be in the writer's queue, and a
    // replay of a partial log would miss commands.
    if detail.session.status == SessionStatus::Active {
        return failure(
            ApiErrorCode::Conflict,
            "This session is still being stored; try again shortly.",
        );
    }
    if !detail.session.has_inputs {
        return failure(
            ApiErrorCode::Conflict,
            "This session has no input recording to re-analyse.",
        );
    }
    let library = match crate::content_store::ContentStore::library_from_snapshots(&snapshots) {
        Ok(l) => l,
        Err(_) => {
            return failure(
                ApiErrorCode::Conflict,
                "The session's content snapshots could not be read.",
            );
        }
    };
    let profile = crate::training::athletes::Athletes::new(&api.data_dir)
        .get(&detail.session.athlete_id)
        .ok();
    let athlete = crate::training::reanalyse::AthleteSettings {
        id: detail.session.athlete_id.clone(),
        stance: profile.as_ref().and_then(|p| p.default_stance),
        coaching: profile
            .as_ref()
            .map_or(CoachingLevel::Standard, |p| p.coaching.level),
        tolerance_scale: profile.as_ref().map_or(1.0, |p| p.coaching.tolerance_scale),
    };
    let Plan::Technique(plan) = detail.plan.clone() else {
        return bad("Re-analysis requires a tracked technique session.");
    };
    let original = crate::training::reanalyse::Original::from_events(plan, athlete, &events);
    let Some(writer) = api.store.clone() else {
        return no_store();
    };
    let job = crate::training::new_session_id();
    let job_id = job.clone();
    let (data_dir, mask) = (api.data_dir.clone(), r.source_mask.unwrap_or_default());
    tokio::task::spawn_blocking(move || {
        let result = (|| -> Result<(), String> {
            let recording = crate::recording::reader::load(&data_dir, &id)?;
            let recording = crate::training::reanalyse::mask(recording, &mask);
            let out = crate::training::reanalyse::run(recording, &original, &library, job.clone())?;
            let (tx, rx) = tokio::sync::oneshot::channel();
            let begin = crate::store::SessionBegin {
                session_id: job.clone(),
                athlete_id: detail.session.athlete_id.clone(),
                started_utc: crate::clock::utc_now_rfc3339(),
                plan: detail.plan.clone(),
                plan_ref: detail.session.plan_ref.clone(),
                origin: detail.session.origin,
                capability_profile: out.context.capability.profile,
                tracking_config: out.context.tracking.clone(),
                calibration_set: out.context.calibration.clone(),
                snapshots,
                has_inputs: false,
                has_motion: false,
                reanalysis_of: Some(id.clone()),
                analysis_rev: detail.session.analysis_rev + 1,
                source_mask: (!mask.is_empty()).then_some(mask),
            };
            if !writer.send(Write::Begin(Box::new(begin), tx)) {
                return Err("store stopped".into());
            }
            rx.blocking_recv().map_err(|e| e.to_string())??;
            for e in out.events {
                writer.send(Write::Event(
                    job.clone(),
                    Box::new(e),
                    detail.session.title.clone(),
                ));
            }
            Ok(())
        })();
        if let Err(e) = result {
            tracing::error!(session=%id, %e, "re-analysis failed");
        }
    });
    ok(ReanalyseResponse { job_id })
}
