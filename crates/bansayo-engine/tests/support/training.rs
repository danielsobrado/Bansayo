//! Shared helpers for training integration tests: HTTP, a simulator rig, a scripted session.
#![allow(dead_code, clippy::expect_used, clippy::unwrap_used)]
use bansayo_engine::{EngineHandle, config, start};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::SocketAddr;
use std::time::{Duration, Instant};

pub async fn http(addr: SocketAddr, method: &str, path: &str, body: Value) -> (u16, Value) {
    let (method, path) = (method.to_owned(), path.to_owned());
    let body = if body.is_null() {
        String::new()
    } else {
        body.to_string()
    };
    tokio::task::spawn_blocking(move || {
        let mut s = std::net::TcpStream::connect(addr).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(20))).unwrap();
        write!(s, "{method} /api/v1/{path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        let mut r = String::new();
        s.read_to_string(&mut r).unwrap();
        let (h, b) = r.split_once("\r\n\r\n").unwrap();
        let status = h.split_whitespace().nth(1).unwrap().parse().unwrap();
        // Chunked bodies: take the JSON between the first `{`/`[` and the last `}`/`]`.
        let start = b.find(['{', '[']).unwrap_or(0);
        let end = b.rfind(['}', ']']).map_or(b.len(), |e| e + 1);
        (status, serde_json::from_str(&b[start..end]).unwrap_or(Value::Null))
    })
    .await
    .unwrap()
}

pub async fn ok(addr: SocketAddr, method: &str, path: &str, body: Value) -> Value {
    let (status, v) = http(addr, method, path, body).await;
    assert_eq!(status, 200, "{method} {path}: {v}");
    v["data"].clone()
}

pub fn repo() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The training rig with a temporary data directory and the given extra YAML.
pub fn rig(data: &std::path::Path, extra: &str) -> String {
    let base = std::fs::read_to_string(repo().join("config/sim-training.yaml")).unwrap();
    base.replace(
        "data_dir: ./data",
        &format!("data_dir: {:?}", data.display().to_string()),
    )
    .replace("bind: 127.0.0.1:7700", "bind: 127.0.0.1:0")
    .replace(
        "dirs: [./content]",
        &format!("dirs: [{:?}]", repo().join("content").display().to_string()),
    )
    .replace("countdown_s: 3", "countdown_s: 1")
        + extra
}

pub async fn engine(yaml: &str) -> EngineHandle {
    start(config::parse(yaml).unwrap()).await.unwrap()
}

pub async fn athlete(addr: SocketAddr, id: &str, consent: bool) {
    ok(
        addr,
        "POST",
        "athletes",
        json!({ "id": id, "display_name": "Ana", "height_m": 1.75, "default_stance": "orthodox" }),
    )
    .await;
    if consent {
        ok(
            addr,
            "POST",
            &format!("athletes/{id}/consent"),
            json!({ "motion_data": true }),
        )
        .await;
    }
    ok(addr, "PUT", "athletes/active", json!({ "athlete_id": id })).await;
}

pub async fn wait_state(
    addr: SocketAddr,
    secs: u64,
    mut done: impl FnMut(&Value) -> bool,
) -> Value {
    let until = Instant::now() + Duration::from_secs(secs);
    loop {
        let s = ok(addr, "GET", "training/state", Value::Null).await;
        if done(&s) {
            return s;
        }
        assert!(Instant::now() < until, "timed out; last state: {}", s);
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

pub async fn motion(addr: SocketAddr, m: &str) {
    ok(addr, "PUT", "simulation", json!({ "motion": m })).await;
}

/// Creates, acknowledges and starts a roundhouse session; kicks only while active.
/// Returns the session id when it completes.
pub async fn run_session(
    addr: SocketAddr,
    sets: u32,
    reps: u32,
    rest_s: u32,
    during: impl AsyncFnMut(&Value),
) -> String {
    let mut during = during;
    motion(addr, "standing").await;
    let s = ok(addr, "POST", "training/sessions", json!({ "plan": { "kind": "technique", "technique": "taekwondo.dollyeo_chagi", "side": "right", "sets": sets, "reps": reps, "rest_s": rest_s } })).await;
    let id = s["session_id"].as_str().unwrap().to_owned();
    if s["readiness"]
        .as_array()
        .unwrap()
        .iter()
        .any(|g| g["gate"] == "safety_ack" && g["state"] == "fail")
    {
        ok(addr, "POST", "training/command", json!({ "command": "ack_safety", "args": { "content_refs": ["taekwondo.dollyeo_chagi@1"] } })).await;
    }
    wait_state(addr, 20, |s| s["phase"] == "ready").await;
    ok(
        addr,
        "POST",
        "training/command",
        json!({ "command": "start" }),
    )
    .await;
    let mut kicking = false;
    let until = Instant::now()
        + Duration::from_secs(60 + u64::from(sets * reps) * 4 + u64::from(sets * rest_s));
    loop {
        let s = ok(addr, "GET", "training/state", Value::Null).await;
        let active = s["phase"] == "active";
        if active != kicking {
            motion(
                addr,
                if active {
                    "roundhouse_kick"
                } else {
                    "standing"
                },
            )
            .await;
            kicking = active;
        }
        during(&s).await;
        if s["phase"] == "completed" || s["phase"] == "aborted" {
            motion(addr, "standing").await;
            return id;
        }
        assert!(Instant::now() < until, "session did not finish: {s}");
        tokio::time::sleep(Duration::from_millis(40)).await;
    }
}

/// The stored session once its row is closed: the analytics writer runs behind the live state.
pub async fn stored(addr: SocketAddr, id: &str) -> Value {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        let (status, v) = http(addr, "GET", &format!("training/sessions/{id}"), Value::Null).await;
        if status == 200 && v["data"]["session"]["status"] != "active" {
            return v["data"].clone();
        }
        assert!(Instant::now() < until, "session {id} not stored: {v}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}
