//! Queries on a read connection (WAL allows them beside the writer).
use bansayo_contracts::*;
use bansayo_training::progress::{ProgressConfig, Row, SessionInfo, series};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

fn parse<T: serde::de::DeserializeOwned>(s: &str) -> Option<T> {
    serde_json::from_value(Value::String(s.to_owned())).ok()
}

#[derive(Debug, Clone, Default)]
pub struct SessionFilter {
    pub athlete_id: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub plan_kind: Option<String>,
    pub limit: Option<u32>,
}

const SUMMARY_COLUMNS: &str = "s.id, s.athlete_id, s.started_utc, s.ended_utc, s.status, s.plan_kind, s.plan_ref, s.plan_json, s.origin, s.capability_profile, s.analysis_rev, s.reanalysis_of, s.has_inputs, s.has_motion, s.rpe, s.summary_json,
  (SELECT COUNT(*) FROM reps r WHERE r.session_id = s.id AND r.outcome = 'completed'),
  (SELECT COUNT(*) FROM reps r WHERE r.session_id = s.id AND r.outcome != 'completed'),
  (SELECT COUNT(*) FROM sets t WHERE t.session_id = s.id AND t.status IN ('completed','capped'))";

fn summary(
    r: &rusqlite::Row,
) -> rusqlite::Result<(TrainingSessionSummary, String, Option<String>)> {
    let plan_json: String = r.get(7)?;
    let summary_json: Option<String> = r.get(15)?;
    let plan: Option<Plan> = serde_json::from_str(&plan_json).ok();
    let title = summary_json
        .as_deref()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .and_then(|v| v["title"].as_str().map(String::from))
        .or_else(|| plan.as_ref().map(|Plan::Technique(t)| t.technique.clone()))
        .unwrap_or_default();
    let id: String = r.get(0)?;
    let athlete: String = r.get(1)?;
    let reanalysis: Option<String> = r.get(11)?;
    Ok((
        TrainingSessionSummary {
            id: SessionId::new(id).map_err(|_| rusqlite::Error::InvalidQuery)?,
            athlete_id: AthleteId::new(athlete).map_err(|_| rusqlite::Error::InvalidQuery)?,
            started_utc: r.get(2)?,
            ended_utc: r.get(3)?,
            status: parse(&r.get::<_, String>(4)?).unwrap_or(SessionStatus::Interrupted),
            plan_kind: parse(&r.get::<_, String>(5)?).unwrap_or(PlanKind::Technique),
            plan_ref: r.get(6)?,
            title,
            origin: parse(&r.get::<_, String>(8)?).unwrap_or(Origin::Live),
            capability_profile: parse(&r.get::<_, String>(9)?).unwrap_or(CapabilityProfileId::None),
            reps_completed: r.get(16)?,
            reps_other: r.get(17)?,
            sets: r.get(18)?,
            analysis_rev: r.get(10)?,
            reanalysis_of: reanalysis.and_then(|s| SessionId::new(s).ok()),
            has_inputs: r.get(12)?,
            has_motion: r.get(13)?,
            rpe: r.get(14)?,
        },
        plan_json,
        summary_json,
    ))
}

pub fn sessions(
    c: &Connection,
    f: &SessionFilter,
) -> rusqlite::Result<Vec<TrainingSessionSummary>> {
    let sql = format!(
        "SELECT {SUMMARY_COLUMNS} FROM training_sessions s
         WHERE (?1 IS NULL OR s.athlete_id = ?1) AND (?2 IS NULL OR s.started_utc >= ?2) AND (?3 IS NULL OR s.started_utc <= ?3)
           AND (?4 IS NULL OR s.plan_kind = ?4)
         ORDER BY s.started_utc DESC, s.id DESC LIMIT ?5"
    );
    let mut q = c.prepare(&sql)?;
    let rows = q.query_map(
        params![
            f.athlete_id,
            f.from,
            f.to,
            f.plan_kind,
            i64::from(f.limit.unwrap_or(100).min(1000))
        ],
        summary,
    )?;
    rows.map(|r| r.map(|x| x.0)).collect()
}

pub fn session(c: &Connection, id: &SessionId) -> rusqlite::Result<Option<TrainingSessionDetail>> {
    let sql = format!("SELECT {SUMMARY_COLUMNS} FROM training_sessions s WHERE s.id = ?1");
    let Some((s, plan_json, summary_json)) = c
        .query_row(&sql, params![id.as_str()], summary)
        .optional()?
    else {
        return Ok(None);
    };
    let mut q = c.prepare("SELECT set_index, content_ref, side, status, started_t_us, ended_t_us, summary_json FROM sets WHERE session_id = ?1 ORDER BY set_index")?;
    let sets = q
        .query_map(params![id.as_str()], |r| {
            let side: Option<String> = r.get(2)?;
            let summary: Option<String> = r.get(6)?;
            Ok(SetRecord {
                set_index: r.get(0)?,
                content_ref: r.get(1)?,
                side: side.and_then(|s| parse(&s)),
                status: r.get(3)?,
                started_t_us: r.get::<_, i64>(4)? as u64,
                ended_t_us: r.get::<_, Option<i64>>(5)?.map(|t| t as u64),
                summary: summary.and_then(|s| serde_json::from_str(&s).ok()),
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(Some(TrainingSessionDetail {
        plan: serde_json::from_str(&plan_json).map_err(|_| rusqlite::Error::InvalidQuery)?,
        summary: summary_json.and_then(|s| serde_json::from_str(&s).ok()),
        session: s,
        sets,
    }))
}

pub fn events(
    c: &Connection,
    id: &SessionId,
    after: u64,
    limit: u32,
) -> rusqlite::Result<Vec<TrainingEvent>> {
    let mut q = c.prepare("SELECT body_json FROM events WHERE session_id = ?1 AND event_seq > ?2 ORDER BY event_seq LIMIT ?3")?;
    let rows = q.query_map(
        params![id.as_str(), after as i64, i64::from(limit.min(10_000))],
        |r| r.get::<_, String>(0),
    )?;
    Ok(rows
        .filter_map(Result::ok)
        .filter_map(|s| serde_json::from_str(&s).ok())
        .collect())
}

/// Every stored repetition result, from its `rep_completed` / `rep_rejected` event.
pub fn reps(c: &Connection, id: &SessionId) -> rusqlite::Result<Vec<RepetitionResult>> {
    let mut q = c.prepare("SELECT type, body_json FROM events WHERE session_id = ?1 AND type IN ('rep_completed','rep_rejected') ORDER BY event_seq")?;
    let rows = q.query_map(params![id.as_str()], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    Ok(rows
        .filter_map(Result::ok)
        .filter_map(|(t, b)| {
            let e: TrainingEvent = serde_json::from_str(&b).ok()?;
            let body = if t == "rep_completed" {
                e.data
            } else {
                e.data["result"].clone()
            };
            serde_json::from_value(body).ok()
        })
        .collect())
}

pub fn provenance(c: &Connection, id: &SessionId) -> rusqlite::Result<Option<SessionProvenance>> {
    let row = c
        .query_row(
            "SELECT s.engine_version, s.observables_rev, s.analysis_rev, s.origin, s.capability_profile, s.reanalysis_of,
                    t.id, t.first_used_utc, t.body_json, k.id, k.first_used_utc, k.body_json, k.qa
             FROM training_sessions s JOIN tracking_configs t ON t.id = s.tracking_config_id JOIN calibration_sets k ON k.id = s.calibration_set_id
             WHERE s.id = ?1",
            params![id.as_str()],
            |r| {
                Ok((
                    r.get::<_, String>(0)?, r.get::<_, u32>(1)?, r.get::<_, u32>(2)?, r.get::<_, String>(3)?, r.get::<_, String>(4)?,
                    r.get::<_, Option<String>>(5)?, r.get::<_, String>(6)?, r.get::<_, String>(7)?, r.get::<_, String>(8)?,
                    r.get::<_, String>(9)?, r.get::<_, String>(10)?, r.get::<_, String>(11)?, r.get::<_, String>(12)?,
                ))
            },
        )
        .optional()?;
    let Some(r) = row else { return Ok(None) };
    let mut q = c.prepare("SELECT c.hash, c.kind, c.id, c.version FROM content_snapshots c JOIN session_content s ON s.snapshot_hash = c.hash WHERE s.session_id = ?1 ORDER BY c.kind, c.id")?;
    let snapshots: Vec<ContentSnapshotRef> = q
        .query_map(params![id.as_str()], |r| {
            Ok(ContentSnapshotRef {
                hash: r.get(0)?,
                kind: r.get(1)?,
                id: r.get(2)?,
                version: r.get(3)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(Some(SessionProvenance {
        session_id: id.clone(),
        engine_version: r.0,
        observables_rev: r.1,
        analysis_rev: r.2,
        origin: parse(&r.3).unwrap_or(Origin::Live),
        capability_profile: parse(&r.4).unwrap_or(CapabilityProfileId::None),
        reanalysis_of: r.5.and_then(|s| SessionId::new(s).ok()),
        tracking_config: ProvenanceRecord {
            id: r.6,
            first_used_utc: r.7,
            body: serde_json::from_str(&r.8).unwrap_or(Value::Null),
        },
        calibration_set: ProvenanceRecord {
            id: r.9,
            first_used_utc: r.10,
            body: serde_json::from_str(&r.11).unwrap_or(Value::Null),
        },
        calibration_qa: r.12,
        content: snapshots
            .iter()
            .map(|s| format!("{}@{}", s.id, s.version))
            .collect(),
        snapshots,
        validation_reports: vec![],
    }))
}

/// Stored snapshots of a session (I3), for pinned re-analysis.
pub fn snapshots(
    c: &Connection,
    id: &SessionId,
) -> rusqlite::Result<Vec<crate::content_store::Snapshot>> {
    let mut q = c.prepare("SELECT c.hash, c.kind, c.id, c.version, c.body_json FROM content_snapshots c JOIN session_content s ON s.snapshot_hash = c.hash WHERE s.session_id = ?1")?;
    q.query_map(params![id.as_str()], |r| {
        Ok(crate::content_store::Snapshot {
            hash: r.get(0)?,
            kind: r.get(1)?,
            id: r.get(2)?,
            version: r.get(3)?,
            body_json: r.get(4)?,
        })
    })?
    .collect()
}

#[derive(Debug, Clone, Default)]
pub struct SeriesQuery {
    pub athlete_id: String,
    pub technique: String,
    pub metric: String,
    pub side: Option<String>,
    pub class: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    /// C5/I9: non-live origins appear only when named.
    pub origin: Option<String>,
}

fn day(utc: &str) -> f64 {
    // `YYYY-MM-DDTHH:MM:SS…` → days since 2000-01-01, enough for slopes and windows.
    let n = |a: usize, b: usize| {
        utc.get(a..b)
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0)
    };
    let (y, m, d) = (n(0, 4), n(5, 7), n(8, 10));
    let (hh, mm, ss) = (n(11, 13), n(14, 16), n(17, 19));
    let a = (14 - m) / 12;
    let (yy, mmm) = (y + 4800 - a, m + 12 * a - 3);
    let jdn = d + (153 * mmm + 2) / 5 + 365 * yy + yy / 4 - yy / 100 + yy / 400 - 32045;
    (jdn - 2_451_545) as f64 + (hh * 3600 + mm * 60 + ss) as f64 / 86_400.0
}

pub fn series_rows(c: &Connection, q: &SeriesQuery) -> rusqlite::Result<(Vec<Row>, String)> {
    let mut stmt = c.prepare(
        "SELECT m.session_id, s.started_utc, m.rep_index, m.value, m.norm, m.grade, m.sigma, m.method, m.comparability_class, m.unit,
                s.capability_profile, s.tracking_config_id, s.calibration_set_id, k.qa, r.technique_version, m.metric_version, s.origin,
                s.reanalysis_of, t.body_json
         FROM measurements m
         JOIN training_sessions s ON s.id = m.session_id
         JOIN reps r ON r.session_id = m.session_id AND r.rep_index = m.rep_index
         JOIN calibration_sets k ON k.id = s.calibration_set_id
         JOIN tracking_configs t ON t.id = s.tracking_config_id
         WHERE s.athlete_id = ?1 AND m.technique_id = ?2 AND m.metric_id = ?3 AND m.value IS NOT NULL
           AND (?4 IS NULL OR m.side = ?4) AND (?5 IS NULL OR m.comparability_class = ?5)
           AND (?6 IS NULL OR s.started_utc >= ?6) AND (?7 IS NULL OR s.started_utc <= ?7)
           AND (s.origin = 'live' OR s.origin = ?8) AND s.status != 'active' AND r.outcome = 'completed'
         ORDER BY s.started_utc, m.session_id, m.rep_index",
    )?;
    let mut unit = String::new();
    let rows = stmt
        .query_map(
            params![
                q.athlete_id,
                q.technique,
                q.metric,
                q.side,
                q.class,
                q.from,
                q.to,
                q.origin
            ],
            |r| {
                let sid: String = r.get(0)?;
                let started: String = r.get(1)?;
                let method: Option<String> = r.get(7)?;
                let method: MethodId = method.and_then(|m| parse(&m)).unwrap_or(MethodId::RestFill);
                let body: String = r.get(18)?;
                let detector = serde_json::from_str::<Value>(&body)
                    .ok()
                    .and_then(|v| v["detectors"].as_array().and_then(|a| a.first().cloned()))
                    .and_then(|d| d["detector"].as_str().map(String::from))
                    .unwrap_or_else(|| "-".into());
                let reanalysis: Option<String> = r.get(17)?;
                Ok((
                    Row {
                        session: SessionInfo {
                            day: day(&started),
                            point: SeriesPoint {
                                session_id: SessionId::new(sid)
                                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                                date_utc: started,
                                value: 0.0,
                                norm: None,
                                spread: 0.0,
                                count: 0,
                                basis: PointBasis::A,
                                grade_mix: GradeMix::default(),
                                comparability_class: String::new(),
                                capability_profile: parse(&r.get::<_, String>(10)?)
                                    .unwrap_or(CapabilityProfileId::None),
                                tracking_config_id: r.get(11)?,
                                calibration_set_id: r.get(12)?,
                                calibration_qa: r.get(13)?,
                                technique_version: r.get(14)?,
                                metric_version: r.get(15)?,
                                origin: parse(&r.get::<_, String>(16)?).unwrap_or(Origin::Live),
                                reanalysis_of: reanalysis.and_then(|s| SessionId::new(s).ok()),
                            },
                        },
                        rep_index: r.get(2)?,
                        value: r.get(3)?,
                        norm: r.get(4)?,
                        grade: parse(&r.get::<_, String>(5)?).unwrap_or(Grade::X),
                        sigma: r.get(6)?,
                        method_class: bansayo_training::metrics::method_class(method).into(),
                        detector_id: detector,
                        comparability_class: r.get(8)?,
                    },
                    r.get::<_, String>(9)?,
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    if let Some((_, u)) = rows.first() {
        unit = u.clone();
    }
    Ok((rows.into_iter().map(|(r, _)| r).collect(), unit))
}

pub fn progress_series(
    c: &Connection,
    q: &SeriesQuery,
    direction: &str,
    cfg: &ProgressConfig,
) -> rusqlite::Result<SeriesResponse> {
    let (rows, unit) = series_rows(c, q)?;
    Ok(SeriesResponse {
        technique: q.technique.clone(),
        metric: q.metric.clone(),
        side: q.side.as_deref().and_then(parse),
        unit,
        direction: direction.into(),
        classes: series(&rows, direction, cfg),
    })
}

/// Distinct (technique, metric, side) series of an athlete, newest session first.
pub fn athlete_series(
    c: &Connection,
    athlete: &str,
    origin: Option<&str>,
) -> rusqlite::Result<Vec<(String, String, String)>> {
    let mut q = c.prepare(
        "SELECT m.technique_id, m.metric_id, m.side, MAX(s.started_utc) FROM measurements m JOIN training_sessions s ON s.id = m.session_id
         WHERE s.athlete_id = ?1 AND (s.origin = 'live' OR s.origin = ?2) AND m.value IS NOT NULL
         GROUP BY m.technique_id, m.metric_id, m.side ORDER BY 4 DESC",
    )?;
    q.query_map(params![athlete, origin], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    })?
    .collect()
}

/// Personal targets of an athlete for scoring: grade-A session values per (metric, class).
pub fn personal_targets(
    c: &Connection,
    athlete: &str,
    technique: &str,
    side: &str,
) -> rusqlite::Result<bansayo_training::scoring::PersonalTargets> {
    let mut q = c.prepare(
        "SELECT m.metric_id, m.comparability_class, m.session_id, m.value, m.norm FROM measurements m JOIN training_sessions s ON s.id = m.session_id
         WHERE s.athlete_id = ?1 AND m.technique_id = ?2 AND m.side = ?3 AND m.grade = 'A' AND s.origin = 'live' AND s.status = 'completed'
         ORDER BY s.started_utc",
    )?;
    type TargetRow = (String, String, String, Option<f64>, Option<f64>);
    let rows: Vec<TargetRow> = q
        .query_map(params![athlete, technique, side], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<Result<_, _>>()?;
    let mut out = bansayo_training::scoring::PersonalTargets::new();
    for norm in [false, true] {
        let mut grouped: std::collections::BTreeMap<(String, String), Vec<(String, f64)>> =
            Default::default();
        for (m, class, sid, v, n) in &rows {
            if let Some(x) = if norm { *n } else { *v } {
                grouped
                    .entry((m.clone(), class.clone()))
                    .or_default()
                    .push((sid.clone(), x));
            }
        }
        for ((m, class), values) in grouped {
            let mut sessions: Vec<(String, Vec<f64>)> = vec![];
            for (sid, x) in values {
                match sessions.last_mut() {
                    Some((s, v)) if *s == sid => v.push(x),
                    _ => sessions.push((sid, vec![x])),
                }
            }
            let session_values: Vec<f64> = sessions
                .iter()
                .filter(|(_, v)| v.len() >= bansayo_training::progress::MIN_SESSION_VALUES)
                .filter_map(|(_, v)| bansayo_training::progress::median(v))
                .collect();
            let best = sessions
                .iter()
                .flat_map(|(_, v)| v.iter().copied())
                .reduce(f64::max);
            out.insert(
                (m, class, norm),
                bansayo_training::scoring::Personal {
                    session_values,
                    best,
                },
            );
        }
    }
    Ok(out)
}
