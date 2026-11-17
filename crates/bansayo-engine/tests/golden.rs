//! Synthetic golden sessions (martial-arts 28 §2–5, ADR-038). `regress` over the checked-in set
//! runs in the normal test run; the generator records new ones from the simulator and is run on
//! purpose: `cargo test -p bansayo-engine --test golden -- --ignored generate`, then
//! `bansayo-engine regress golden/synthetic/manifest.yaml --bless` and review the diff.
#![allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "support/training.rs"]
mod support;
use bansayo_contracts::*;
use bansayo_engine::regress::{TrainingFile, run_manifest};
use bansayo_engine::training::reanalyse::{AthleteSettings, Original};
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use support::*;

fn manifest() -> std::path::PathBuf {
    repo().join("golden/synthetic/manifest.yaml")
}

#[test]
fn synthetic_golden_sessions_pass_regress() {
    let report = run_manifest(&manifest()).expect("manifest runs");
    let failed: Vec<String> = report
        .checks
        .iter()
        .filter(|c| !c.pass)
        .map(|c| format!("{} [{}] {}: {}", c.session, c.run, c.name, c.detail))
        .collect();
    assert!(failed.is_empty(), "{failed:#?}");
    assert!(report.checks.len() >= 20);
}

async fn wait_closed(addr: std::net::SocketAddr, id: &str) {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        let d = ok(addr, "GET", &format!("training/sessions/{id}"), Value::Null).await;
        if d["session"]["status"] != "active"
            && ok(addr, "GET", "state", Value::Null).await["recording"].is_null()
        {
            return;
        }
        assert!(Instant::now() < until);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Copies a recorded session into the golden set with its pinned content and replay facts.
fn export(data: &std::path::Path, id: &str, plan: TechniquePlan) {
    let src = data.join("sessions").join(id);
    let dst = repo().join("golden/synthetic/sessions").join(id);
    std::fs::create_dir_all(&dst).unwrap();
    bansayo_engine::recording::recompress(&src.join("recording.mcap"), &dst.join("recording.mcap"))
        .unwrap();
    std::fs::copy(src.join("session.json"), dst.join("session.json")).unwrap();
    let c = bansayo_engine::store::open(&data.join("db/bansayo.sqlite")).unwrap();
    let sid = SessionId::new(id).unwrap();
    let events = bansayo_engine::store::read::events(&c, &sid, 0, 1_000_000).unwrap();
    let snapshots = bansayo_engine::store::read::snapshots(&c, &sid).unwrap();
    let athlete = AthleteSettings {
        id: AthleteId::new("ana").unwrap(),
        stance: Some(Stance::Orthodox),
        coaching: CoachingLevel::Standard,
        tolerance_scale: 1.5,
    };
    let file = TrainingFile {
        original: Original::from_events(plan, athlete, &events),
        snapshots,
    };
    std::fs::write(
        dst.join("training.json"),
        serde_json::to_vec_pretty(&file).unwrap(),
    )
    .unwrap();
}

/// The pipeline refuses control while it finishes a recording; retry briefly.
async fn fault(addr: std::net::SocketAddr, action: &str) {
    for _ in 0..100 {
        let (status, _) = http(
            addr,
            "POST",
            "simulation/devices/SIM-CAM2/fault",
            json!({ "action": action }),
        )
        .await;
        if status == 200 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("fault {action} refused");
}

fn plan(side: PlanSide, reps: u32) -> TechniquePlan {
    TechniquePlan {
        technique: "taekwondo.dollyeo_chagi".into(),
        side,
        stages: None,
        reference: None,
        coaching: None,
        scoring: None,
        sets: Some(1),
        reps: Some(reps),
        rest_s: Some(0),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "records new golden sessions; run on purpose and bless"]
async fn generate() {
    let dir = tempfile::tempdir().unwrap();
    let engine = engine(&rig(dir.path(), "")).await;
    let addr = engine.bound_addr();
    athlete(addr, "ana", true).await;
    // 1. Three right roundhouse kicks with every camera.
    let a = run_session(addr, 1, 3, 0, async |_: &Value| {}).await;
    wait_closed(addr, &a).await;
    export(dir.path(), &a, plan(PlanSide::Right, 3));
    // 2. The same, with one camera disconnected after the first repetition.
    let mut faulted = false;
    let b = run_session(addr, 1, 3, 0, async |s: &Value| {
        if !faulted && s["totals"]["reps_completed"] == 1 {
            faulted = true;
            fault(addr, "disconnect").await;
        }
    })
    .await;
    fault(addr, "reconnect").await;
    wait_closed(addr, &b).await;
    export(dir.path(), &b, plan(PlanSide::Right, 3));
    // 3. A left plan performed with the right leg: wrong side, ended by the athlete.
    motion(addr, "standing").await;
    let s = ok(addr, "POST", "training/sessions", json!({ "plan": { "kind": "technique", "technique": "taekwondo.dollyeo_chagi", "side": "left", "sets": 1, "reps": 3, "rest_s": 0 } })).await;
    let c = s["session_id"].as_str().unwrap().to_owned();
    wait_state(addr, 20, |s| s["phase"] == "ready").await;
    ok(
        addr,
        "POST",
        "training/command",
        json!({ "command": "start" }),
    )
    .await;
    wait_state(addr, 20, |s| s["phase"] == "active").await;
    motion(addr, "roundhouse_kick").await;
    wait_state(addr, 30, |s| {
        s["totals"]["reps_other"].as_u64().unwrap_or(0) >= 3
    })
    .await;
    ok(
        addr,
        "POST",
        "training/command",
        json!({ "command": "end" }),
    )
    .await;
    motion(addr, "standing").await;
    wait_closed(addr, &c).await;
    export(dir.path(), &c, plan(PlanSide::Left, 3));
    println!("generated {a} {b} {c}");
    engine.shutdown().await.unwrap();
}
