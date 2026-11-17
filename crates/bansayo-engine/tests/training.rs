//! Roadmap Phase 1 on the engine: athletes, consent, a guided session on the simulator, stored
//! rows, event refill, provenance and progress.
#![allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "support/training.rs"]
mod support;
use serde_json::{Value, json};
use support::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn guided_roundhouse_session_is_counted_stored_and_reviewable() {
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
    let detail = ok(addr, "GET", &format!("training/sessions/{id}"), Value::Null).await;
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
