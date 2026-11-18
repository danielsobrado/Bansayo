//! Roadmap Phase 2 on one camera without hardware or model weights: an image-folder camera
//! replays a roundhouse-kick sequence through a guided session at capability profile `basic`.
//! The folder carries recorded COCO-17 landmarks (`landmarks.jsonl`): the simulator's scripted
//! roundhouse projected into a front-facing 30 fps camera, one second of standing before each
//! kick. Pose inference is the only stage not exercised; everything from the camera's capture
//! thread to stored, graded repetitions is.
#![allow(clippy::expect_used, clippy::unwrap_used)]
#[path = "support/training.rs"]
mod support;
use bansayo_contracts::DeviceId;
use bansayo_engine::config::CameraSpec;
use bansayo_engine::skeleton::Skeleton;
use bansayo_engine::sources::simulated::{body::body_pose, camera::project, motions::Motion};
use serde_json::{Value, json};
use std::path::Path;
use std::time::{Duration, Instant};
use support::*;

const RATE_HZ: f64 = 30.0;
const SIZE: [u32; 2] = [320, 240];
/// Canonical point index of each COCO-17 landmark; eyes and nose sit on the head.
const COCO_FROM_CANONICAL: [usize; 17] =
    [4, 4, 4, 4, 4, 6, 10, 7, 11, 8, 12, 13, 16, 14, 17, 15, 18];

/// One cycle: 1 s standing, then one 2.8 s scripted roundhouse with the right leg.
fn write_kicks(dir: &Path) -> usize {
    std::fs::create_dir_all(dir).unwrap();
    let sk = Skeleton::builtin("bansayo.body.v1", 1.75).unwrap();
    let camera = CameraSpec {
        id: DeviceId::new("CAM-FRONT").unwrap(),
        rate_hz: RATE_HZ,
        resolution_px: SIZE,
        fov_y_deg: 60.0,
        position_m: [0.0, 1.0, 4.0],
        look_at_m: [0.0, 0.9, 0.0],
        noise_px: 0.0,
    };
    let still = (1.0 * RATE_HZ) as usize;
    let kick = (2.8 * RATE_HZ) as usize;
    let mut rows = String::new();
    let blank = image::RgbImage::new(SIZE[0], SIZE[1]);
    for k in 0..still + kick {
        let t = if k < still {
            0.0
        } else {
            (k - still) as f64 / RATE_HZ
        };
        let pose = body_pose(&sk, Motion::RoundhouseKick, t);
        let points = project(&camera, 0, k as u64, 0, &pose.keypoints(&sk)).points;
        let row: Vec<Option<[f32; 3]>> = COCO_FROM_CANONICAL
            .iter()
            .enumerate()
            .map(|(c, &i)| {
                let p = points[i].as_ref()?;
                // Ears either side of the head so their midpoint is the head point.
                let dx = match c {
                    3 => 4.0,
                    4 => -4.0,
                    _ => 0.0,
                };
                Some([p.x_px + dx, p.y_px, 0.9])
            })
            .collect();
        rows.push_str(&serde_json::to_string(&row).unwrap());
        rows.push('\n');
        blank.save(dir.join(format!("{k:04}.png"))).unwrap();
    }
    std::fs::write(dir.join("landmarks.jsonl"), rows).unwrap();
    still + kick
}

fn yaml(data: &Path, frames: &Path) -> String {
    format!(
        r#"schema_version: 2
engine: {{ publish_rate_hz: 60, stale_after_ms: 250, data_dir: {data:?} }}
api: {{ bind: "127.0.0.1:0" }}
content: {{ dirs: [{content:?}] }}
training: {{ countdown_s: 1 }}
sources:
  - id: cams
    kind: camera
    models_dir: {models:?}
    cameras:
      - {{ id: CAM-FRONT, frames_dir: {frames:?}, rate_hz: {RATE_HZ} }}
"#,
        data = data.display().to_string(),
        content = repo().join("content").display().to_string(),
        models = data.join("no-models").display().to_string(),
        frames = frames.display().to_string(),
    )
}

/// Renames with retries: the capture thread may be reading an image at that instant.
fn rename(from: &Path, to: &Path) {
    for _ in 0..50 {
        if std::fs::rename(from, to).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("could not rename {}", from.display());
}

fn measurement<'a>(rep: &'a Value, metric: &str) -> &'a Value {
    rep["measurements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| {
            m["metric"]
                .as_str()
                .unwrap()
                .starts_with(&format!("{metric}@"))
        })
        .unwrap_or_else(|| panic!("{metric} missing: {rep}"))
}

fn first_reason(m: &Value) -> &str {
    m["reasons"][0]["code"].as_str().unwrap_or_default()
}

/// 18 §3 Phase 2 acceptance on one image-folder camera: counts, phase durations, normalised
/// image-plane measurements, every metre- and speed-based metric suppressed with its reason;
/// the readiness reasons of 22 §5; unplugging pauses after 3 s and replugging resumes (08 §10).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn guided_roundhouse_on_one_image_folder_camera() {
    let dir = tempfile::tempdir().unwrap();
    let frames = dir.path().join("frames");
    write_kicks(&frames);
    let engine = engine(&yaml(dir.path(), &frames)).await;
    let addr = engine.bound_addr();
    athlete(addr, "ana", true).await;
    let s = ok(addr, "POST", "training/sessions", json!({ "plan": { "kind": "technique", "technique": "taekwondo.dollyeo_chagi", "side": "right", "sets": 1, "reps": 3, "rest_s": 0 } })).await;
    let id = s["session_id"].as_str().unwrap().to_owned();
    ok(addr, "POST", "training/command", json!({ "command": "ack_safety", "args": { "content_refs": ["taekwondo.dollyeo_chagi@1"] } })).await;
    // Ready once the planar reference is captured during a still second.
    let ready = wait_state(addr, 30, |s| s["phase"] == "ready").await;
    assert_eq!(ready["capability_profile"], "basic", "{ready}");
    let availability = |metric: &str| {
        ready["metric_availability"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["metric"] == metric)
            .unwrap()
            .clone()
    };
    for (metric, grade, code) in [
        ("kick.total_time", "B", "estimated"),
        ("kick.extension_time", "C", "camera_rate"),
        ("kick.peak_ankle_height", "B", "normalised_only"),
        ("kick.peak_ankle_speed", "X", "rate_too_low"),
        ("kick.hip_rotation_range", "X", "needs_calibrated_cameras"),
        ("kick.support_pivot", "X", "detector_no_feet"),
        ("kick.guard_at_strike", "X", "needs_calibrated_cameras"),
    ] {
        let a = availability(metric);
        assert_eq!(
            (a["best_grade"].as_str().unwrap(), first_reason(&a)),
            (grade, code),
            "{metric}: {a}"
        );
    }
    assert_eq!(
        availability("kick.extension_time")["reasons"][0]["message"],
        "camera rate 30 fps"
    );
    ok(
        addr,
        "POST",
        "training/command",
        json!({ "command": "start" }),
    )
    .await;

    // After the first counted kick, unplug the camera: no observables, tracking lost, paused 3 s
    // later; plug it back: the capture thread reopens it and the session resumes by itself.
    wait_state(addr, 40, |s| s["totals"]["reps_completed"] == 1).await;
    let unplugged = dir.path().join("frames-unplugged");
    rename(&frames, &unplugged);
    let lost_at = Instant::now();
    let paused = wait_state(addr, 20, |s| s["phase"] == "paused").await;
    let waited = lost_at.elapsed();
    assert!(
        waited >= Duration::from_secs(3),
        "paused after {waited:?}: {paused}"
    );
    rename(&unplugged, &frames);
    wait_state(addr, 20, |s| s["phase"] == "active").await;
    let done = wait_state(addr, 60, |s| s["phase"] == "completed").await;
    assert_eq!(done["totals"]["reps_completed"], 3, "{done}");

    let events = ok(
        addr,
        "GET",
        &format!("training/sessions/{id}/events?after_seq=0&limit=10000"),
        Value::Null,
    )
    .await;
    let events = events.as_array().unwrap();
    let kinds: Vec<&str> = events
        .iter()
        .map(|e| e["type"].as_str().unwrap_or_default())
        .collect();
    let paused = events
        .iter()
        .find(|e| e["type"] == "session_paused")
        .unwrap();
    assert_eq!(paused["data"]["cause"], "tracking_lost", "{kinds:?}");
    let resumed = events
        .iter()
        .find(|e| e["type"] == "session_resumed")
        .unwrap();
    assert_eq!(resumed["data"]["cause"], "tracking_restored", "{kinds:?}");
    let reps: Vec<&Value> = events
        .iter()
        .filter(|e| e["type"] == "rep_completed")
        .map(|e| &e["data"])
        .collect();
    assert_eq!(reps.len(), 3, "{kinds:?}");
    for rep in reps {
        let r = &rep["repetition"];
        assert_eq!(r["capability_profile"], "basic");
        assert!(r["phases"].as_array().unwrap().len() >= 3, "{r}");
        let layout: Vec<String> = r["phases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| {
                let ms =
                    (p["t_end_us"].as_u64().unwrap() - p["t_start_us"].as_u64().unwrap()) / 1000;
                format!("{} {ms} ms", p["phase"].as_str().unwrap())
            })
            .collect();
        eprintln!("rep {}: {}", r["rep_index"], layout.join(", "));
        // A phase entered on the same sample as the next one has no samples of its own.
        for p in r["phases"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["t_end_us"] != p["t_start_us"])
        {
            assert_eq!(p["evidence"]["method"], "image_plane", "{p}");
            assert_eq!(p["evidence"]["sources"], json!(["CAM-FRONT"]));
        }
        // Counts and timing.
        let total = measurement(rep, "kick.total_time");
        assert_eq!(total["grade"], "B", "{total}");
        assert!(total["value"].as_f64().unwrap() > 500.0);
        let ext = measurement(rep, "kick.extension_time");
        assert!(
            ext["grade"] == "C" || first_reason(ext) == "anchor_not_detected",
            "{ext}"
        );
        // Normalised image-plane measurements: grade B, no metres.
        for metric in ["kick.peak_knee_height", "kick.peak_ankle_height"] {
            let m = measurement(rep, metric);
            assert_eq!(m["grade"], "B", "{m}");
            assert!(
                m["value"].is_null() && m["norm"].as_f64().unwrap() > 0.1,
                "{m}"
            );
            assert_eq!(first_reason(m), "normalised_only");
        }
        // Every metre- and speed-based metric suppressed with its reason.
        for (metric, code) in [
            ("kick.peak_ankle_speed", "rate_too_low"),
            ("kick.hip_rotation_range", "needs_calibrated_cameras"),
            ("kick.support_pivot", "detector_no_feet"),
            ("kick.recovery_settle_time", "metric_kind_not_implemented"),
        ] {
            let m = measurement(rep, metric);
            assert_eq!(
                (m["grade"].as_str().unwrap(), first_reason(m)),
                ("X", code),
                "{m}"
            );
            assert!(m["value"].is_null() && m["norm"].is_null());
        }
        let guard = measurement(rep, "kick.guard_at_strike");
        assert_eq!(guard["grade"], "X", "{guard}");
    }
    engine.shutdown().await.unwrap();
}
