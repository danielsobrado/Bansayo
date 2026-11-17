#![allow(clippy::expect_used, clippy::unwrap_used)]
use super::read::*;
use super::*;
use serde_json::json;

fn sid(k: u32) -> SessionId {
    SessionId::new(format!("2026111{}T100000Z-t{k:03}", k % 10)).unwrap()
}

fn begin(k: u32, origin: Origin, calibration: &str, detector: &str) -> SessionBegin {
    SessionBegin {
        session_id: sid(k),
        athlete_id: AthleteId::new("ana").unwrap(),
        started_utc: format!("2026-11-1{}T10:00:00Z", k % 10),
        plan: Plan::Technique(TechniquePlan {
            technique: "taekwondo.dollyeo_chagi".into(),
            side: PlanSide::Right,
            stages: None,
            reference: None,
            coaching: None,
            scoring: None,
            sets: Some(1),
            reps: Some(3),
            rest_s: None,
        }),
        plan_ref: None,
        origin,
        capability_profile: CapabilityProfileId::MultiView,
        tracking_config: (
            format!("tc-{detector}"),
            "multi_view".into(),
            json!({ "detectors": [{ "detector": detector }] }),
        ),
        calibration_set: (
            calibration.into(),
            "not_run".into(),
            json!({ "profiles": [calibration] }),
        ),
        snapshots: vec![crate::content_store::Snapshot {
            hash: "h1".into(),
            kind: "metrics".into(),
            id: "kick.total_time".into(),
            version: 1,
            body_json: r#"{"direction":"neutral","id":"kick.total_time","version":1}"#.into(),
        }],
        has_inputs: false,
        has_motion: false,
        reanalysis_of: None,
        analysis_rev: 1,
        source_mask: None,
    }
}

fn ev(seq: u64, kind: &str, data: Value) -> TrainingEvent {
    TrainingEvent {
        event_seq: seq,
        t_us: seq * 1000,
        event_type: kind.into(),
        position: PlanPosition {
            block: Some(0),
            item: Some(0),
            set: Some(1),
            round: None,
            step: None,
        },
        data,
    }
}

fn rep(k: u32, value: f64, grade: &str, class: &str) -> Value {
    json!({
        "repetition": { "rep_index": k, "technique": "taekwondo.dollyeo_chagi@1", "side": "right", "stance": null,
            "outcome": "completed", "t_start_us": k * 1000, "t_end_us": k * 1000 + 900, "phases": [], "strike_t_us": null,
            "recognition_confidence": 0.9, "capability_profile": "multi_view" },
        "measurements": [{ "metric": "kick.total_time@1", "value": value, "norm": null, "unit": "ms", "grade": grade,
            "evidence": { "provenance": "derived", "method": "fk", "confidence": 0.9, "sigma": null, "sigma_basis": null, "t_obs_us": null, "sources": [] },
            "comparability_class": class, "reasons": [] }],
        "score": { "model": "scoring.kick.basic@1", "components": [], "reference_match": null, "coverage": 0.0, "comparability_class": "s" },
        "cue": null, "targets": [], "scored": true, "excluded": false
    })
}

/// Writes one session of three repetitions; returns after the writer has applied it.
fn store_session(w: &Writer, b: SessionBegin, values: [f64; 3], grade: &str, class: &str) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    assert!(w.send(Write::Begin(Box::new(b.clone()), tx)));
    rx.blocking_recv().unwrap().unwrap();
    let id = b.session_id;
    let mut seq = 0;
    let mut send = |kind: &str, data: Value| {
        seq += 1;
        assert!(w.send(Write::Event(
            id.clone(),
            Box::new(ev(seq, kind, data)),
            "Roundhouse kick".into()
        )));
    };
    send(
        "set_started",
        json!({ "set_index": 1, "technique": "taekwondo.dollyeo_chagi@1", "side": "right", "target": { "reps": 3 } }),
    );
    for (k, v) in values.iter().enumerate() {
        send("rep_completed", rep(k as u32 + 1, *v, grade, class));
    }
    send(
        "set_completed",
        json!({ "set_index": 1, "status": "completed" }),
    );
    send(
        "session_completed",
        json!({ "totals": { "active_ms": 1000 } }),
    );
    let (tx, rx) = tokio::sync::oneshot::channel();
    assert!(w.send(Write::Flush(tx)));
    rx.blocking_recv().unwrap();
}

fn count(c: &Connection, table: &str) -> i64 {
    c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

#[test]
fn migration_round_trip_and_settings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("db/bansayo.sqlite");
    let c = open(&path).unwrap();
    let version: usize = c
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, MIGRATIONS.len());
    let mode: String = c
        .pragma_query_value(None, "journal_mode", |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    let fk: i64 = c
        .pragma_query_value(None, "foreign_keys", |r| r.get(0))
        .unwrap();
    assert_eq!(fk, 1);
    let schema = |c: &Connection| -> Vec<String> {
        let mut q = c
            .prepare("SELECT sql FROM sqlite_master WHERE sql IS NOT NULL ORDER BY name")
            .unwrap();
        q.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    let first = schema(&c);
    for t in [
        "athletes",
        "content_snapshots",
        "tracking_configs",
        "calibration_sets",
        "training_sessions",
        "session_content",
        "sets",
        "reps",
        "measurements",
        "scores",
        "events",
        "baselines",
        "personal_records",
    ] {
        assert!(
            first
                .iter()
                .any(|s| s.contains(&format!("CREATE TABLE {t} "))),
            "{t}"
        );
    }
    drop(c);
    let again = open(&path).unwrap();
    assert_eq!(schema(&again), first, "reopening applies nothing twice");
    // A broken migration leaves the file untouched.
    let mut c = Connection::open(dir.path().join("other.sqlite")).unwrap();
    c.execute_batch("CREATE TABLE athletes (id TEXT)").unwrap();
    assert!(matches!(migrate(&mut c), Err(StoreError::Migration { .. })));
    let version: usize = c
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .unwrap();
    assert_eq!(version, 0);
}

#[test]
fn invariants_and_deletion() {
    let dir = tempfile::tempdir().unwrap();
    let w = Writer::spawn(dir.path().join("bansayo.sqlite")).unwrap();
    store_session(
        &w,
        begin(1, Origin::Live, "cal-a", "det-a"),
        [900.0, 910.0, 920.0],
        "A",
        "c1",
    );
    store_session(
        &w,
        begin(2, Origin::Live, "cal-a", "det-b"),
        [800.0, 810.0, 820.0],
        "A",
        "c2",
    );
    store_session(
        &w,
        begin(3, Origin::Simulated, "cal-a", "det-a"),
        [700.0, 710.0, 720.0],
        "B",
        "c1",
    );
    let c = w.read().unwrap();
    assert_eq!(count(&c, "reps"), 9);
    assert_eq!(count(&c, "measurements"), 9);
    // I1: measurements and scores are never updated.
    assert!(c.execute("UPDATE measurements SET value = 1", []).is_err());
    assert!(c.execute("UPDATE scores SET coverage = 1", []).is_err());
    // I3: the definitions a session used are retrievable in exactly that version.
    let snaps = snapshots(&c, &sid(1)).unwrap();
    assert_eq!(
        snaps[0].body_json,
        r#"{"direction":"neutral","id":"kick.total_time","version":1}"#
    );
    // I4: the calibration set a session used is retrievable with its profiles.
    let p = provenance(&c, &sid(1)).unwrap().unwrap();
    assert_eq!(p.calibration_set.id, "cal-a");
    assert_eq!(p.calibration_set.body["profiles"][0], "cal-a");
    assert_eq!(p.content, ["kick.total_time@1"]);
    // I6: series never span two comparability classes; I9: simulated hidden unless requested.
    let q = SeriesQuery {
        athlete_id: "ana".into(),
        technique: "taekwondo.dollyeo_chagi".into(),
        metric: "kick.total_time".into(),
        ..Default::default()
    };
    let s = progress_series(&c, &q, "neutral", &Default::default()).unwrap();
    assert_eq!(s.classes.len(), 2);
    for class in &s.classes {
        assert!(
            class
                .points
                .iter()
                .all(|p| p.comparability_class == class.comparability_class)
        );
        assert!(class.points.iter().all(|p| p.origin == Origin::Live));
    }
    let sim = progress_series(
        &c,
        &SeriesQuery {
            origin: Some("simulated".into()),
            ..q.clone()
        },
        "neutral",
        &Default::default(),
    )
    .unwrap();
    let c1 = sim
        .classes
        .iter()
        .find(|x| x.comparability_class == "c1")
        .unwrap();
    assert_eq!(c1.points.len(), 2);
    assert_eq!(c1.points[1].basis, PointBasis::B);
    // Reps are rebuilt from stored events; events support gap refill.
    assert_eq!(reps(&c, &sid(1)).unwrap().len(), 3);
    assert_eq!(events(&c, &sid(1), 2, 100).unwrap()[0].event_seq, 3);
    // Personal records from grade-A live values only.
    assert_eq!(count(&c, "personal_records"), 2);
    // A deleted session leaves no row.
    let (tx, rx) = tokio::sync::oneshot::channel();
    assert!(w.send(Write::Delete(sid(1), tx)));
    assert!(rx.blocking_recv().unwrap().unwrap());
    for t in [
        "reps",
        "measurements",
        "scores",
        "events",
        "sets",
        "session_content",
    ] {
        let n: i64 = c
            .query_row(
                &format!("SELECT COUNT(*) FROM {t} WHERE session_id = ?1"),
                params![sid(1).as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0, "{t}");
    }
    assert!(session(&c, &sid(1)).unwrap().is_none());
    assert_eq!(sessions(&c, &SessionFilter::default()).unwrap().len(), 2);
}
