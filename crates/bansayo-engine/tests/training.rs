//! Roadmap Phase 1 on the engine: athletes, consent, a guided session on the simulator, stored
//! rows, event refill, provenance and progress.
#![allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "support/training.rs"]
mod support;
use serde_json::{Value, json};
use support::*;

// Real-time simulator tests share CPU resources; run one rig at a time.
static TRAINING_TEST: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn guided_roundhouse_session_is_counted_stored_and_reviewable() {
    let _guard = TRAINING_TEST.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = engine(&rig(dir.path(), "")).await;
    let addr = engine.bound_addr();
    // Content and capabilities.
    let index = ok(addr, "GET", "content/index", Value::Null).await;
    assert!(
        index["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["valid"] == true),
        "{index}"
    );
    assert_eq!(
        ok(
            addr,
            "GET",
            "content/techniques/taekwondo.dollyeo_chagi",
            Value::Null
        )
        .await["family"],
        "kick.roundhouse"
    );
    assert!(
        ok(addr, "GET", "content/i18n/en", Value::Null).await["cue.kick.pivot.observed"]
            .is_string()
    );
    // No athlete: no session.
    let (status, _) = http(addr, "POST", "training/sessions", json!({ "plan": { "kind": "technique", "technique": "taekwondo.dollyeo_chagi", "side": "right" } })).await;
    assert_eq!(status, 409);
    athlete(addr, "ana", true).await;
    let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while ok(addr, "GET", "state", Value::Null).await["athlete"]["id"] != "ana" {
        assert!(
            std::time::Instant::now() < until,
            "SystemState.athlete not published"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let (status, _) = http(
        addr,
        "PUT",
        "athletes/active",
        json!({ "athlete_id": "nobody" }),
    )
    .await;
    assert_eq!(status, 404);
    let mut max_cue_per_rep = 0;
    let id = run_session(addr, 1, 3, 0, async |s: &Value| {
        if let Some(c) = s["last_rep"]["cue"].as_object() {
            max_cue_per_rep = max_cue_per_rep.max(usize::from(!c.is_empty()));
        }
    })
    .await;
    let s = ok(addr, "GET", "training/state", Value::Null).await;
    assert_eq!(s["phase"], "completed", "{s}");
    assert_eq!(s["totals"]["reps_completed"], 3, "{s}");
    // Stored rows: session, repetitions, provenance, events.
    let detail = stored(addr, &id).await;
    assert_eq!(detail["session"]["status"], "completed");
    assert_eq!(detail["session"]["origin"], "simulated");
    assert_eq!(detail["session"]["reps_completed"], 3);
    let reps = ok(
        addr,
        "GET",
        &format!("training/sessions/{id}/reps"),
        Value::Null,
    )
    .await;
    let completed: Vec<&Value> = reps
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["repetition"]["outcome"] == "completed")
        .collect();
    assert_eq!(completed.len(), 3);
    for r in &completed {
        let phases: Vec<&str> = r["repetition"]["phases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["phase"].as_str().unwrap())
            .collect();
        assert_eq!(
            phases,
            ["chamber", "pivot", "extension", "retraction", "recovery"],
            "{r}"
        );
        let m = |id: &str| {
            r["measurements"]
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["metric"].as_str().unwrap().starts_with(id))
                .unwrap()
                .clone()
        };
        assert_eq!(
            m("kick.total_time")["grade"],
            "B",
            "{}",
            m("kick.total_time")
        );
        assert_eq!(m("kick.peak_ankle_height")["grade"], "X");
        assert_eq!(
            m("kick.peak_ankle_height")["reasons"][0]["code"],
            "calibration_missing"
        );
    }
    let one = ok(addr, "GET", &format!("training/reps/{id}/2"), Value::Null).await;
    assert_eq!(one["repetition"]["rep_index"], 2);
    let p = ok(
        addr,
        "GET",
        &format!("training/sessions/{id}/provenance"),
        Value::Null,
    )
    .await;
    assert_eq!(p["calibration_qa"], "not_run");
    assert!(
        p["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c == "taekwondo.dollyeo_chagi@1")
    );
    assert_eq!(
        p["tracking_config"]["body"]["detectors"][0]["detector"],
        "simulated-projection"
    );
    let events = ok(
        addr,
        "GET",
        &format!("training/sessions/{id}/events?after_seq=0&limit=10000"),
        Value::Null,
    )
    .await;
    let seqs: Vec<u64> = events
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["event_seq"].as_u64().unwrap())
        .collect();
    assert!(!seqs.is_empty() && seqs.windows(2).all(|w| w[1] == w[0] + 1) && seqs[0] == 1);
    // Progress: simulated sessions only when asked (C5); a B-basis point for the session.
    let q = "progress/series?athlete_id=ana&technique=taekwondo.dollyeo_chagi&metric=kick.total_time&side=right".to_owned();
    assert!(
        ok(addr, "GET", &q, Value::Null).await["classes"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let sim = ok(addr, "GET", &format!("{q}&origin=simulated"), Value::Null).await;
    assert_eq!(
        sim["classes"][0]["points"].as_array().unwrap().len(),
        1,
        "{sim}"
    );
    assert_eq!(sim["classes"][0]["points"][0]["basis"], "B");
    let summary = ok(
        addr,
        "GET",
        "progress/summary?athlete_id=ana&origin=simulated",
        Value::Null,
    )
    .await;
    assert_eq!(
        summary["techniques"][0]["technique"],
        "taekwondo.dollyeo_chagi"
    );
    assert!(max_cue_per_rep <= 1);
    engine.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn without_consent_a_session_runs_and_nothing_is_stored() {
    let _guard = TRAINING_TEST.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = engine(&rig(dir.path(), "")).await;
    let addr = engine.bound_addr();
    athlete(addr, "bo", false).await;
    let id = run_session(addr, 1, 1, 0, async |_: &Value| {}).await;
    let (status, _) = http(addr, "GET", &format!("training/sessions/{id}"), Value::Null).await;
    assert_eq!(status, 404);
    let list = ok(addr, "GET", "training/sessions", Value::Null).await;
    assert!(list.as_array().unwrap().is_empty());
    // Live events are still served for the running engine's session.
    let events = ok(
        addr,
        "GET",
        &format!("training/sessions/{id}/events"),
        Value::Null,
    )
    .await;
    assert!(!events.as_array().unwrap().is_empty());
    engine.shutdown().await.unwrap();
}

async fn wait_session(addr: std::net::SocketAddr, id: &str) -> Value {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let (status, v) = http(addr, "GET", &format!("training/sessions/{id}"), Value::Null).await;
        let events = ok(
            addr,
            "GET",
            &format!("training/sessions/{id}/events?limit=100000"),
            Value::Null,
        )
        .await;
        let done = events.as_array().is_some_and(|e| {
            e.last()
                .is_some_and(|l| l["type"] == "session_completed" || l["type"] == "session_aborted")
        });
        if status == 200 && done {
            return v["data"].clone();
        }
        assert!(
            std::time::Instant::now() < until,
            "re-analysis {id} did not finish"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn recorded_session_is_reviewable_and_reanalyses_are_identical() {
    let _guard = TRAINING_TEST.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = engine(&rig(dir.path(), "")).await;
    let addr = engine.bound_addr();
    athlete(addr, "ana", true).await;
    let id = run_session(addr, 1, 3, 0, async |_: &Value| {}).await;
    // Recordings close shortly after the session ends.
    let mut detail = Value::Null;
    for _ in 0..100 {
        detail = ok(addr, "GET", &format!("training/sessions/{id}"), Value::Null).await;
        let rec = ok(addr, "GET", "state", Value::Null).await;
        // Stored (all events written) and the input recording closed.
        if detail["session"]["status"] == "completed" && rec["recording"].is_null() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(detail["session"]["has_inputs"], true, "{detail}");
    assert_eq!(detail["session"]["has_motion"], true);
    let trace = ok(
        addr,
        "GET",
        &format!("training/reps/{id}/2/trace"),
        Value::Null,
    )
    .await;
    let n = trace["t_us"].as_array().unwrap().len();
    assert!(n > 30, "{n} samples");
    let ankle = trace["series"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "ankle_speed.right")
        .unwrap();
    assert_eq!(ankle["values"].as_array().unwrap().len(), n);
    let clip = ok(
        addr,
        "GET",
        &format!("training/reps/{id}/2/motion"),
        Value::Null,
    )
    .await;
    assert!(clip["samples"].as_array().unwrap().len() > 30);
    // The clip carries its repetition's phases. Phase accuracy is pinned by the deterministic
    // session and golden tests, not by this live run whose timing depends on machine load.
    let rep = ok(addr, "GET", &format!("training/reps/{id}/2"), Value::Null).await;
    assert_eq!(clip["phases"], rep["repetition"]["phases"]);
    assert_eq!(
        clip["samples"][0]["rotations"].as_array().unwrap().len(),
        19
    );
    // Segment origins are forward kinematics from the root, one per segment.
    assert_eq!(
        clip["samples"][0]["positions_m"].as_array().unwrap().len(),
        19
    );
    assert_eq!(
        clip["samples"][0]["positions_m"][0],
        clip["samples"][0]["root_m"]
    );
    // Two re-analyses with pinned content give identical event logs (acceptance 2).
    let a = ok(
        addr,
        "POST",
        &format!("training/sessions/{id}/reanalyse"),
        json!({ "content": "pinned" }),
    )
    .await["job_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let b = ok(
        addr,
        "POST",
        &format!("training/sessions/{id}/reanalyse"),
        json!({ "content": "pinned" }),
    )
    .await["job_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let da = wait_session(addr, &a).await;
    wait_session(addr, &b).await;
    assert_eq!(da["session"]["reanalysis_of"], id.as_str());
    assert_eq!(da["session"]["analysis_rev"], 2);
    let ea = ok(
        addr,
        "GET",
        &format!("training/sessions/{a}/events?limit=100000"),
        Value::Null,
    )
    .await;
    let eb = ok(
        addr,
        "GET",
        &format!("training/sessions/{b}/events?limit=100000"),
        Value::Null,
    )
    .await;
    assert_eq!(ea, eb);
    // Phase 1 guarantees identical re-analyses (above), not that a re-analysis reproduces the live
    // count: the replayed solver can count one repetition fewer (measured at the Phase 1 commit
    // too); reproducing measurements is roadmap Phase 3 (docs/13, Phase 2 contradictions).
    let reps = da["session"]["reps_completed"].as_u64().unwrap();
    assert!((2..=3).contains(&reps), "{da}");
    let (status, _) = http(
        addr,
        "POST",
        &format!("training/sessions/{id}/reanalyse"),
        json!({ "content": "current" }),
    )
    .await;
    assert_eq!(status, 400);
    // Deleting the original removes it, its re-analyses and its files.
    ok(
        addr,
        "DELETE",
        &format!("training/sessions/{id}"),
        Value::Null,
    )
    .await;
    assert_eq!(
        http(addr, "GET", &format!("training/sessions/{a}"), Value::Null)
            .await
            .0,
        404
    );
    assert!(!dir.path().join("sessions").join(&id).exists());
    engine.shutdown().await.unwrap();
}

/// Acceptance 4 (18 §3): a second session appears beside the first, and a session run with a
/// different pose-model id is a separate series rather than a continuation.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_second_session_and_a_different_pose_model_appear_in_progress() {
    let _guard = TRAINING_TEST.lock().await;
    let dir = tempfile::tempdir().unwrap();
    let q = "progress/series?athlete_id=ana&technique=taekwondo.dollyeo_chagi&metric=kick.total_time&side=right&origin=simulated";
    let engine = engine(&rig(dir.path(), "")).await;
    let addr = engine.bound_addr();
    athlete(addr, "ana", true).await;
    // A session point needs at least three values (12 §6).
    run_session(addr, 1, 3, 0, async |_: &Value| {}).await;
    run_session(addr, 1, 3, 0, async |_: &Value| {}).await;
    let s = series(addr, q, |s| points(s) == 2).await;
    assert_eq!(s["classes"].as_array().unwrap().len(), 1, "{s}");
    engine.shutdown().await.unwrap();
    // Same data directory, another pose model.
    let other = rig(dir.path(), "").replace(
        "detector_id: simulated-projection",
        "detector_id: simulated-projection-b",
    );
    let engine = self::engine(&other).await;
    let addr = engine.bound_addr();
    ok(
        addr,
        "PUT",
        "athletes/active",
        json!({ "athlete_id": "ana" }),
    )
    .await;
    run_session(addr, 1, 3, 0, async |_: &Value| {}).await;
    let s = series(addr, q, |s| points(s) == 3).await;
    let classes = s["classes"].as_array().unwrap();
    assert_eq!(classes.len(), 2, "{s}");
    let detectors: Vec<&str> = classes
        .iter()
        .map(|c| c["detector_id"].as_str().unwrap())
        .collect();
    assert!(
        detectors.contains(&"simulated-projection-b"),
        "{detectors:?}"
    );
    assert!(classes.iter().all(|c| c["points"].as_array().unwrap().len()
        == if c["detector_id"] == "simulated-projection-b" {
            1
        } else {
            2
        }));
    engine.shutdown().await.unwrap();
}

fn points(s: &Value) -> usize {
    s["classes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["points"].as_array().unwrap().len())
        .sum()
}

/// Session points are written when the session closes, after the state reports completion.
async fn series(addr: std::net::SocketAddr, q: &str, done: impl Fn(&Value) -> bool) -> Value {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let s = ok(addr, "GET", q, Value::Null).await;
        if done(&s) {
            return s;
        }
        assert!(
            std::time::Instant::now() < until,
            "series never filled: {s}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
