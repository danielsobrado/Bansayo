# Phase 01 — OpenXR uplink: hardware validation report

**Outcome: BLOCKED — physical OpenXR hardware unavailable.** No headset or controllers were
connected to the implementation machine. Nothing below the "machine checks" section was run;
none of it may be read as passed.

| Field | Value |
|---|---|
| Roadmap phase | Phase 01, OpenXR observation pipeline (2026-11-20 roadmap; not historical Phase 1) |
| Engine commit | `178e69d` (VR client) on `main`; see the Phase 01 entry in [13](../13-implementation-status.md) |
| Headset / controllers | **none available** |
| OpenXR runtime | **none** |
| Godot | 4.5.1.stable.official.f62fdbde1 (project features 4.4) |
| OS | Windows 11 Pro 10.0.26300 |
| Engine config | `config/xr-local.example.yaml` (port and data dir changed for the run) |

## Machine checks that did run (no headset)

These prove the Godot sender and the engine endpoint agree; they do not prove headset tracking,
role mapping on real controllers, or real observation rates.

`tests/xr_uplink_live.gd` (real `xr_pose_uplink.gd` and `xr_pose_capture.gd`, synthetic nodes
under an origin at (0, 0, 3), right grip "occluded" for the middle third) against the engine
binary with `xr.enabled: true`:

| Measure | Result |
|---|---|
| Handshake | `ready`, `time_quality: receive_time` |
| Frames in ~4 s | 237 accepted; 0 rejected, 0 over-rate, 0 queue-full (engine disconnect summary) |
| Measured rate (engine status) | HMD 60.1 Hz, left 60.1 Hz, right 59.9 Hz |
| Mid-run status | HMD and left `tracking`; right `disconnected` during occlusion |
| Right grip observations | 157 of 237 (occluded frames sent as untracked, no pose recorded) |
| Client close | all three `disconnected` |
| Second connection | seq 0 accepted after the new handshake |

## Manual protocol (spec §7) — status

| # | Step | Status |
|---|---|---|
| 1 | Engine with `xr-local.example.yaml`, health + status | Machine part done (above) |
| 2 | Local override `bansayo/xr_uplink_enabled=true`, launch VR | BLOCKED — no headset |
| 3 | Three devices acquiring → tracking with a headset | BLOCKED |
| 4 | 10 s still recording, ≥ 45 Hz per device, no socket growth | BLOCKED |
| 5 | Move head / left / right only; only that device changes | BLOCKED (left/right mapping on real controllers unverified) |
| 6 | Right controller out of view → disconnected, recovers | BLOCKED (synthetic equivalent passed) |
| 7 | Recenter: raw stage values may change, no world transform applied | BLOCKED |
| 8 | Kill/restart Godot with engine running; seq 0 accepted | BLOCKED on headset (synthetic equivalent passed) |
| 9 | Record a short session, inspect MCAP, replay, compare | BLOCKED on headset (automated: `xr_ingest_recording_roundtrip_and_replay_refuses_live`) |
| 10 | Stop/restart engine with Godot open; bounded reconnect | BLOCKED |
| 11 | Simulator camera-only and desktop Godot unchanged | Automated parts pass (gates 1–2, workspace tests, Godot smokes) |
| 12 | Ingest time per frame, status latency | BLOCKED |

To complete: fill this table on a headset, record headset/controller/runtime/firmware, frame-rate
median and p95, per-device received/dropped, and a short recording's replay comparison, then
change the outcome. The ≥ 45 Hz target is a provisional operational threshold, not an accuracy
claim; XR poses are not aligned to camera world (Phase 02).
