# Bansayo Phase 02 — XR room alignment and time synchronization

**Status:** Implementation specification, not implementation code.  
**Date:** 2026-11-20.  
**Target repository:** https://github.com/danielsobrado/Bansayo (commit directly to main).  
**Verified target baseline:** https://github.com/danielsobrado/Bansayo/commit/104a8864649ad99d52e8b68e64d7dac1cf080ef7  
**Reference baseline:** https://github.com/maceq687/FullBodyPoseEstimation/commit/be895709c0bf86d35f64eb2cd5d2ffb4c6838593  
**Prerequisite:** [Phase 01 detailed implementation plan](phase-01-openxr-observation-pipeline-implementation.md). The verified baseline contains the **Phase 01 document only**, not its code. Implement and verify Phase 01 before Phase 02; its planned new files are not currently present.
**Next phases:** Phase 03 body calibration and Phase 04 camera/HMD/controller fusion.  
**License:** The upstream example is GPL-3.0. Use its ideas for independent implementation, not its C# source or assets.

> **Main-branch safety gate:** At the start of every implementation commit, fetch main and check which listed paths and methods still exist. Rebase the source map to live code without overwriting newer functionality. Never force-push, revert or silently replace concurrent changes. The immutable line links below are verified at the target snapshot, not promised on future main.

## 0 — Fixed goal and scope

Implement a trustworthy relationship between **raw OpenXR stage coordinates** and the **metric camera world** defined by Bansayo's floor ChArUco calibration, plus an uncertainty-aware mapping from Godot's monotonic clock to the engine's session clock.

The deliverable is **not** skeleton fusion. Phase 02 provides four independent capabilities:

1. A robust four-timestamp OpenXR client-to-engine clock offset estimator, including jitter uncertainty, drift estimate, reset detection and fallback to receive time.
2. A measurable camera-world-to-XR-stage rigid transform, solved with a pivot-calibrated right-controller contact point and four known floor marks (A/B/C fit, D independent validation).
3. An engine-owned, persistent, per-room XR alignment profile that is invalidated on XR recenter/restart, room change, camera calibration replacement or camera identity mismatch.
4. A compact, guided Godot calibration workflow that applies an **engine-published** transform once to BodyRoot and never moves the native headset camera.

**Exit target:** Independent holdout position error at most **3 cm**, quality-gated estimated timestamps (not claimed hardware-synchronized), accurate calibration state and recorded/replayed XR observations that retain their accepted session timestamps. The 3 cm target is **provisional** and must be measured with physical hardware; no synthetic test can satisfy the real-world validation gate.

**Explicitly prohibited:** guessed shoulder/head anchors, uncalibrated grip origin as a physical contact point, copying upstream fixed Euler wrist offsets, treating a controller as an anatomical wrist, silently using Godot's old local alignment file as a measured profile, altering camera-only or IMU-only skeleton fusion, changing protocol version 1.

## 1 — Code references in BOTH repositories

### 1.1 Upstream: FullBodyPoseEstimation

| Immutable source code reference | Verified behavior | Exact Bansayo interpretation |
|---|---|---|
| [VNectModel.cs lines 122–132](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L122-L132) | Stores Unity XR headset and controller input devices/poses | The tracking devices are inputs with their own coordinate frame, not the human skeleton. |
| [VNectModel.cs lines 169–183](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L169-L183) | Reads HMD/controller position and rotation in Update | Acquire in Godot, but explicitly measure clock offset and tracking validity before fusion. |
| [VNectModel.cs lines 188–214](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L188-L214) | Adjusts controller coordinates via HMD/nose offset, hard-coded wrist Euler angles and phantom elbows | **Do not copy or translate.** Those steps do not establish a calibrated physical room transform. |
| [VNectModel.cs lines 603–658](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L603-L658) | Guided T-pose/VR calibration with a five-second delay and controller span | A guided wizard is useful; controller span does not measure shoulder width or camera-world alignment. |
| [PoseVisualizer3D.cs lines 152–173](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/PoseVisualizer3D.cs#L152-L173) | Scales detector landmarks using user dimensions | Detector XYZ is not an independently calibrated metric room position; use ChArUco world anchors instead. |
| [GPL-3.0 LICENSE](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/LICENSE) | Upstream copyleft license | Use only cited concepts and independently implemented mathematics. |

The upstream project does **not** provide a verified four-timestamp clock estimator, multi-point ChArUco room calibration, profile revision fingerprinting, room-scoped persistence or replay-compatible time mapping. Bansayo needs its own implementation.

### 1.2 Target: existing Bansayo modules

Each line link below is pinned to commit 104a886.

| Verified target code | Current fact | Phase 02 change |
|---|---|---|
| [scripts/calibrate_cameras.py lines 1–20](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/scripts/calibrate_cameras.py#L1-L20) | The floor board defines world origin, +Y up, +X first board row | Reuse precisely this world frame for alignment, not a Godot approximation. |
| [scripts/calibrate_cameras.py lines 190–226](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/scripts/calibrate_cameras.py#L190-L226) | solvePnP produces per-camera extrinsics and chooses board up sign | Add exact board corner metadata and immutable capture identity to extrinsics quality fields. |
| [scripts/calibrate_cameras.py lines 339–369](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/scripts/calibrate_cameras.py#L339-L369) | 7×5 ChArUco generator and --square-m | Use a physically measured, sufficiently large board for metric alignment. |
| [contracts/src/frames.rs lines 9–35](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-contracts/src/frames.rs#L9-L35) | Vec3, xyzw Hamilton quaternion, World and XrStage already defined | No new frame conventions or axis flips. |
| [contracts/src/calibration.rs lines 13–44](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-contracts/src/calibration.rs#L13-L44) | CalibrationKind::XrAlignment already exists with documented T_xr_stage_from_world | Reuse it. Never add another enum with inverse meaning. |
| [contracts/src/calibration.rs lines 75–110, 139–164](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-contracts/src/calibration.rs#L75-L164) | Versioned CalibrationProfile, dependencies, CameraExtrinsicsData and room | Add typed XR data/quality; preserve camera data schema. |
| [engine/src/calibration/store.rs lines 19–25, 111–189](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-engine/src/calibration/store.rs#L19-L189) | room_of/in_room are camera-extrinsics-specific; valid_profile rejects XR kind | Extend these functions, including room filtering and typed XR validation. |
| [engine/src/calibration/store.rs lines 116–155](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-engine/src/calibration/store.rs#L116-L155) | Invalidates extrinsics when intrinsics change, and builds summaries | Invalidate dependent XR profiles as well, including camera-content replacement at same ID. |
| [engine/src/pipeline/runtime.rs lines 675–747](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-engine/src/pipeline/runtime.rs#L675-L747) | Camera calibration import currently **rejects non-camera kinds** | Do not post XR profiles through camera import; use a dedicated typed XR solve command. |
| [engine/src/pipeline/runtime.rs lines 335–375, 636–674](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-engine/src/pipeline/runtime.rs#L335-L375) | Atomic save, failure handling, publish_state and record_context exist | Reuse rather than add a second persistence writer. |
| [engine/src/registry/persistence.rs lines 78–149](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-engine/src/registry/persistence.rs#L78-L149) | RuntimeSnapshot stores calibration profiles and active_room | Store one typed XR profile in this snapshot. |
| [engine/src/pipeline/control.rs lines 197–226](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-engine/src/pipeline/control.rs#L197-L226) | Room change handled in single-writer runtime and forbidden while recording | Invalidate previously active XR alignment on room change, preserving recording guard. |
| [engine/src/pipeline/runner.rs lines 125–181](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-engine/src/pipeline/runner.rs#L125-L181) | SystemState has active_room and calibration summaries | Add optional xr_alignment status, no frame protocol change. |
| [engine/src/clock.rs lines 45–157](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-engine/src/clock.rs#L45-L157) | One-way minimum-arrival offset estimator includes unknown path delay | Add distinct four-timestamp XR estimator; keep one-way estimator unchanged for other devices. |
| [contracts/src/time.rs lines 8–21](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-contracts/src/time.rs#L8-L21) | TimeQuality::ReceiveTime, Estimated and Synchronized already exist | Use Estimated after good mapping; fallback to ReceiveTime. No unproven Synchronized. |
| [contracts/src/device.rs lines 227–240](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-contracts/src/device.rs#L227-L240) | TimeSyncStatus already has offset, uncertainty, drift, settled and resets | Reuse for XR devices. |
| [engine/src/sources/mod.rs lines 15–24](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-engine/src/sources/mod.rs#L15-L24) | DeviceRuntime has optional time_sync | Publish per-XR-device quality without modifying existing camera timing. |
| [engine/src/pipeline/runtime.rs lines 572–614](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-engine/src/pipeline/runtime.rs#L572-L614) | Validated SourceEvents are recorded and replayed | Map XR client time **before** creating accepted event; never re-map replay. |
| [engine/src/recording/mod.rs lines 11–65](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-engine/src/recording/mod.rs#L11-L65) | Context includes calibration profiles, device state, config and active_room | Preserve historical deterministic restore and older optional fields. |
| [engine/src/api/mod.rs lines 20–60, 320–350](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-engine/src/api/mod.rs#L20-L60) | Axum REST API, typed envelope and camera import | Add two local-only XR calibration endpoints and route-specific body limit. |
| [apps/vr/scripts/training_room.gd lines 241–268](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/apps/vr/scripts/training_room.gd#L241-L268) | Current right-controller aim-based yaw / local user://xr_alignment.cfg | Replace as alignment authority; label old saved value **assumed**, not measured. |
| [apps/vr/scripts/training_room.gd lines 331–408](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/apps/vr/scripts/training_room.gd#L331-L408) | XR stage origin, camera and focus/recenter callbacks | Generate stage ID on XR initialization; increment generation on recenter. |
| [apps/vr/scripts/body_root.gd](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/apps/vr/scripts/body_root.gd) | Holds engine content; headset camera is separate | Apply one world→stage transform here, never to headset. |
| [apps/vr/scripts/status_panel.gd lines 1–48, 101–145](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/apps/vr/scripts/status_panel.gd#L1-L145) | Existing compact setup/tracking panel | Show valid/stale/assumed alignment, wizard progress, measured residuals. |
| [apps/vr/scripts/engine_actions.gd lines 35–58](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/apps/vr/scripts/engine_actions.gd#L35-L58) | Reusable authenticated/local control request mechanism | Reuse for calibration HTTP; no parallel HTTP client framework. |

**Planned Phase 01 code, currently not present:** crates/bansayo-contracts/src/xr.rs, crates/bansayo-engine/src/api/xr.rs, crates/bansayo-engine/src/sources/xr.rs, apps/vr/scripts/xr_pose_capture.gd and apps/vr/scripts/xr_pose_uplink.gd. Implement Phase 01 first; update these exact paths afterward.

**Relevant official references:** [Godot XRInterface 4.4](https://docs.godotengine.org/en/4.4/classes/class_xrinterface.html), [Godot OpenXR stage and recenter behavior](https://docs.godotengine.org/en/4.4/tutorials/xr/openxr_settings.html), [XRCamera3D 4.4](https://docs.godotengine.org/en/4.4/classes/class_xrcamera3d.html), [RFC 5905 §8](https://datatracker.ietf.org/doc/html/rfc5905#section-8).

## 2 — Fixed architecture and non-negotiable contracts

~~~text
OpenXR HMD + right/left grip nodes
       |
Godot xr_pose_uplink.gd (Phase 01, one WebSocket)
       +-- StageInfo(session_id, generation)
       +-- SyncPing(t0) -> Rust t1,t2 -> SyncPong -> Godot t3 -> SyncResult
       +-- Frame(seq, client_time_us, three raw stage poses)
                   |
           Rust XRClockSync -> estimated session capture time
                   |                  |
                   |                  +--> DeviceRuntime.time_sync status
                   +--> Phase 01 typed SensorObservation -> existing registry -> recorder
                                                                   |
ChArUco calibration script -> camera intrinsics/extrinsics         |
       +-- measured board world A/B/C/D markers                    |
       +-- camera revision fingerprint                             |
                    |                                              |
Godot wizard: pivot then A/B/C/D touches                           |
       |                                                           |
       +--> POST /calibration/xr/solve -----------------------------+
                    |
           Rust pivot + yaw-only floor fit + independent D holdout
                    |
           CalibrationProfile(kind=xr_alignment)
                    |
           existing runtime snapshot, active_room and SystemState
                    |
           Godot BodyRoot <- T_xr_stage_from_world
                    X  no anatomical mapping / solver fusion yet
~~~

**Invariants:** one camera-defined metric world, stage coordinates already from Phase 01, one session clock, one stored transform direction, one calibration writer, one Godot BodyRoot application point, no silent local saved override, no new broker/database, no GPL code reuse, bounded arrays and network messages. FrameRef::XrStage must remain separate from FrameRef::World until Phase 04 consumes a *validated* transform.

## 3 — Workstream A: four-timestamp clock synchronization

### 3.1 New XR uplink message variants

Extend the **Phase 01 planned** XR wire enums additively; keep existing Hello, Ready and Frame messages exactly compatible:

~~~rust
XrClientMessage::StageInfo {
    stage_session_id: String,
    stage_generation: u64,
    play_area_mode: String,
}
XrClientMessage::SyncPing {
    request_id: u64,
    t0_client_us: u64,
}
XrClientMessage::SyncResult {
    request_id: u64,
    t3_client_us: u64,
}
XrServerMessage::SyncPong {
    request_id: u64,
    t0_client_us: u64,
    t1_engine_us: u64,
    t2_engine_us: u64,
}
~~~

Examples:

~~~json
{"type":"stage_info","stage_session_id":"04fba03b1bf64cbd874d3a8d852a2047","stage_generation":0,"play_area_mode":"stage"}
{"type":"sync_ping","request_id":5,"t0_client_us":123000000}
{"type":"sync_pong","request_id":5,"t0_client_us":123000000,"t1_engine_us":48000210,"t2_engine_us":48000245}
{"type":"sync_result","request_id":5,"t3_client_us":123001200}
~~~

**Timing ownership:** t0 = Godot monotonic Time.get_ticks_usec immediately before send; t1 = engine SessionClock::now_us immediately after decoding ping; t2 = SessionClock::now_us immediately before sending pong; t3 = Godot monotonic tick immediately upon receiving pong. Engine retains the pending t0/t1/t2 tuple keyed by request_id; it does not trust client-supplied engine times. Exactly one ping in flight per connection. After a new websocket connection, reset request_id and pending state, not the stage ID if the OpenXR session is unchanged.

### 3.2 Equations and safety

Let theta be **engine clock minus client clock**:

~~~text
roundtrip_us = (t3-t0) - (t2-t1)
theta_us = ((t1-t0)+(t2-t3))/2
uncertainty_bound_us = ceil(roundtrip_us/2)

mapped_engine_time(C) = C + theta_ref + slope*(C-C_ref)
drift_ppm = slope*1_000_000
~~~

Use **signed i128** intermediate arithmetic and check casts; never subtract unsigned clock values directly. Reject negative/implausible roundtrip, unmatched ping, t3<t0, t2<t1, huge/overflowing samples or stale pong. The half-RTT is a conservative bound under nonnegative asymmetric delays, not proof of symmetric transport. If a mapped sample is in the future relative to server arrival or out of sequence, **fall back to ReceiveTime with a reason** rather than silently clamp it and claim Estimated.

The sampled Godot camera pose can lag the render thread. This clock mapping timestamps the **Godot polling instant**, not independently observed OpenXR sensor exposure time. Do not claim motion-to-photon or synchronized camera-exposure accuracy on this basis.

### 3.3 Prescribed algorithm

Create **crates/bansayo-engine/src/xr_clock_sync.rs**, exported from engine lib.rs. Do not refactor the existing one-way clock.rs or change camera/IMU timestamp behavior.

| Parameter (xr.sync) | Default | Legal interval |
|---|---:|---|
| enabled | true | Boolean, effective only if xr.enabled |
| ping_interval_ms | 250 | 100–2000 |
| max_rtt_us | 10000 | 1000–50000 |
| max_uncertainty_us | 5000 | 500–20000 |
| min_samples | 8 | 4–64 |
| history_samples | 64 | 16–256 |
| drift_min_span_s | 20 | 10–120 |
| max_abs_drift_ppm | 200 | 20–1000 |
| status_stale_after_ms | 2000 | 500–10000 |
| max_capture_age_ms | 100 | 20–500 |

Implementation, in exact order:

1. Keep VecDeque of at most 64 **accepted** clock tuples (client midpoint, theta, roundtrip, bound). Reject RTT above 10000us or uncertainty above 5000us.
2. Before 8 good samples, set TimeSyncStatus.settled=false, map frames at server ReceiveTime and retain original device_time_us.
3. On reaching 8 samples, choose minimum RTT among the latest 8 for initial theta. Publish TimeQuality::Estimated with numeric offset and uncertainty.
4. On every subsequent good sample compute median theta of the **three lowest-delay samples** among most recent 16 (or all available once >=8). Set theta_ref at median client midpoint of these samples.
5. Once >=20 seconds of accepted client time are available, bucket observations by one-second client-clock interval; choose lowest RTT per bucket, use centered ordinary least squares for theta as a function of client midpoint. Accept slope only if finite, enough independent buckets and absolute ppm <=200; otherwise slope=0 and drift_ppm=None.
6. If last good ping exceeds 2000ms, stage identity changes, client clock rewinds >1s, or engine restarts, clear/disable estimate; count reset. A simple WebSocket reconnect requires reacquiring clock estimate, even if same stage.
7. Convert incoming **entire Phase 01 Frame** once using client_time_us; all three device SensorObservations share this mapped t_us. Reject abnormal latency >100ms and future mappings. Preserve original client_time_us as device_time_us.
8. Set TimeQuality::Estimated **only** for frames mapped using a fresh, settled estimator; otherwise ReceiveTime. Never set Synchronized without real external clock-sync evidence.
9. Publish the same estimate to existing DeviceRuntime.time_sync for XR-HMD, XR-LEFT-GRIP and XR-RIGHT-GRIP; do not add a second UI time-status contract.
10. Replay uses recorded t_us and time_quality exactly; must not run a live offset estimator.

**Ingest integration point:** Phase 01 planned api/xr.rs and sources/xr.rs translate its frame before [pipeline/runtime.rs::accept](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-engine/src/pipeline/runtime.rs#L572-L614).

### 3.4 Clock test cases (no real sleeps)

- xr_clock_known_positive_offset: synthetic engine=client+5,000,000us; precise symmetric recovery.
- xr_clock_known_negative_offset: synthetic engine=client-3,000,000us, no unsigned underflow.
- xr_clock_asymmetric_latency: 1ms outbound and 3ms inbound; bound covers resulting bias.
- xr_clock_rejects_negative_roundtrip and xr_clock_rejects_unknown_request.
- xr_clock_lowest_rtt_window_rejects_jitter: inject 9ms outliers, choose stable low RTT group.
- xr_clock_fifty_ppm_drift: 30s synthetic clock with +50ppm, preliminary error <=15ppm.
- xr_clock_stale_falls_back_to_receive_time: 2s no valid ping.
- xr_clock_reconnect_resets_estimator: same XR stage, new socket, fresh pending queue.
- xr_clock_stage_change_resets_estimator: new session/generation, stale mapping.
- xr_clock_future_and_reordered_capture_rejected: no future t_us enters registry.
- xr_clock_recording_replay_is_identical: no retimestamp on replay.

## 4 — Workstream B: real floor-board alignment, not a guessed offset

### 4.1 The reference board and exact four points

The engine's camera-world frame is anchored to the ChArUco board's **first inner corner**, with board x mapped to world X and floor normal to world Y. scripts/calibrate_cameras.py computes **up_sign** from the actual floor board/camera geometry in its solvePnP stage. Do not infer the sign from a screen screenshot.

**Physical setup chosen for this phase:** 7 columns × 5 rows, measured square edge **0.20 m**, total mat approximately 1.40m × 1.00m, printed without changing scale and fixed to the floor. This size is intentionally larger than the script's typical small optical calibration board; a 0.04m square 7×5 board cannot provide a sufficiently long anchor baseline for a 3cm spatial target. Require at least two current, calibrated room cameras observing the same origin setup. If the rig was calibrated with a different board, recapture/import extrinsics; do not silently assume equivalence.

The four marker anchors are **inner ChArUco corners**, with grid indices relative to the first inner corner:

| Marker | Inner corner (col,row) | World coordinates |
|---|---|---|
| A | (0,0) | [0,h,0] |
| B | (5,0) | [5s,h,0] |
| C | (0,3) | [0,h,-up_sign*3s] |
| D | (5,3) | [5s,h,-up_sign*3s] |

s is physically measured square edge (default example 0.20m), h is --origin-height-m (normally 0). For 0.20m squares, A–B is 1.00m and A–C is 0.60m. Board row direction into world Z is **-up_sign**: follow [scripts/calibrate_cameras.py lines 209–224](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/scripts/calibrate_cameras.py#L209-L224), not an assumed direction.

### 4.2 Extend camera extrinsics provenance

Modify scripts/calibrate_cameras.py::solve and profile generation to add **quality.xr_anchor_board** (leave CameraExtrinsicsData data fields unchanged):

~~~json
{
  "schema_version": 1,
  "cols": 7,
  "rows": 5,
  "square_m": 0.2,
  "origin_height_m": 0.0,
  "up_sign": -1,
  "room": null,
  "origin_set": "0025",
  "board_capture_id": "SHA256_HEX",
  "points": [
    {"id":"A","position_world_m":[0,0,0]},
    {"id":"B","position_world_m":[1,0,0]},
    {"id":"C","position_world_m":[0,0,0.6]},
    {"id":"D","position_world_m":[1,0,0.6]}
  ]
}
~~~

The above is illustrative for up_sign=-1. The actual solve must populate its measured sign, board scale and image-set identity. Compute board_capture_id from a **canonical string** including (schema version, room, measured square_m as a six-decimal string, board columns/rows, sign, origin-set name and ordered anchors). Hash with Python hashlib.sha256. For engine-side camera fingerprint use [engine/math/hash.rs](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-engine/src/math/hash.rs), which reexports the existing sha256 implementation. Serialize canonical JSON as UTF-8, sorted keys and no extra spaces. Include **actual camera profile content**, not only canonical profile IDs, so replacing extrinsics at the same ID invalidates XR alignment.

Require at least two valid intrinsics/extrinsics pairs in current room, confirmed camera identities, matching board_capture_id and board anchors. Old camera profiles without metadata remain valid for triangulation but **not XR alignment**. Report missing metadata explicitly and direct the user to recapture the floor view. Do not fabricate anchors.

**Engine GET /api/v1/calibration/xr/anchors:** returns ApiResponse::ok containing active room, camera_fingerprint, board_capture_id, four marker IDs/positions, required controller right_grip and current camera profile references. On failures return an existing structured 409 error with code: camera_calibration_missing, camera_identity_unconfirmed, xr_anchor_metadata_missing, camera_board_mismatch or camera_room_invalid.

### 4.3 Pivot-calibrate the physical right-controller contact point

A controller grip origin is a virtual location. A user touching A with the physical housing/pointer does **not** place the grip origin at world A. Therefore calibrate a *repeatable contact tip* on the right controller before anchor measurements.

**Procedure:** Keep one stable physical point of controller housing (or securely attached rigid tip) touching marker A. Rotate gently around that stationary contact point for **5 seconds**, retaining **32 valid, well-spaced pose samples** with varied yaw/pitch/roll and tracked right grip. Do not take 32 adjacent same-orientation frames.

For each sample, p_i is grip position in XR stage and R_i is grip rotation. Unknown tip offset r (in grip local frame) and fixed pivot c (in stage) satisfy:

~~~text
p_i + R_i * r = c

[R_i -I] [r;c] = -p_i   (3N x 6 least-squares system)
~~~

Create **crates/bansayo-engine/src/calibration/xr_pivot.rs** using existing nalgebra QR/SVD, with no OpenCV runtime dependency. Initial linear least squares, compute 3D residual for each sample, discard outliers >max(0.020m,3×median residual), refit if >=28 remain. Require:
- 32 input tracked samples over >=3s;
- >=25° orientation span;
- non-degenerate matrix, finite solution, singular-value condition number <=1000;
- pivot RMS <=0.010m;
- estimated tip offset norm <=0.30m.

If any fail, return an actionable error. No default/grip origin offset may substitute for failed pivot calibration. This offset is **grip-to-physical-tip**, not anatomical wrist offset (Phase 03).

### 4.4 Capture A, B, C, D and solve yaw

After pivot passes, the Godot wizard prompts **A → B → C → D**. For each marker, user touches that printed inner corner with the calibrated tip and collects **12 tracked samples spanning >=0.30s**. Convert each to stage tip:

~~~text
tip_stage_i = position_stage_grip_i
              + rotate(q_stage_from_grip_i, r_grip_from_tip)
~~~

Take the component-wise median. Require every accepted pose <=0.008m 3D distance from that marker's median, or retry that marker. If XR recentered, stage session changed, active room changed, camera fingerprint changed or tracker lost during capture, cancel/restart the full wizard.

**Solve transform only from A, B, C. Hold D completely out of optimization.** With w world XZ and s stage XZ:

~~~text
wx = world_i.x - mean(world.x)
wz = world_i.z - mean(world.z)
sx = stage_i.x - mean(stage.x)
sz = stage_i.z - mean(stage.z)

a = Σ(wx*sx + wz*sz)
b = Σ(wz*sx - wx*sz)
yaw = atan2(b,a)

R_y(yaw) maps XZ:
  sx =  cos(yaw)*wx + sin(yaw)*wz
  sz = -sin(yaw)*wx + cos(yaw)*wz

t_x = mean(stage.x) - cos(yaw)*mean(world.x) - sin(yaw)*mean(world.z)
t_z = mean(stage.z) + sin(yaw)*mean(world.x) - cos(yaw)*mean(world.z)
t_y = median(stage_i.y - world_i.y) over A/B/C

T_xr_stage_from_world = translation(t_x,t_y,t_z) * rotation_y(yaw)
~~~

No scale fit, no pitch/roll fit. Stage and world are both Y-up. A significant vertical tilt/height inconsistency is a failed assumption and must **fail** calibration rather than be hidden in unconstrained six-DoF fitting. The inverse T_world_from_xr_stage is derived only when future consumers need it; never store two independent transforms.

**Required QA checks:**
- A/B/C world non-collinear in XZ, triangle area >=0.10 m².
- Minimum pairwise anchor baseline >=0.60m.
- A/B/C 3D fit RMS <=0.015m.
- Independent **D 3D** holdout error <=0.030m.
- Maximum y residual after t_y <=0.020m.
- Maximum inter-anchor pair distance mismatch <=0.025m.
- All position/quaternion components finite, quaternion unit norm within configured tolerance.
- Same known camera fingerprint and current XR stage generation throughout capture.

**Known 90° test:** World A=[0,0,0], B=[1,0,0], C=[0,0,0.6], D=[1,0,0.6]. T yaw +pi/2 and translation [2,0.04,3] gives stage A=[2,0.04,3], B=[2,0.04,2], C=[2.6,0.04,3], D=[2.6,0.04,2]. Verify sign convention and no mirror. Also test identity, 180° yaw and opposite up_sign board.

### 4.5 Alignment calibration data contracts

Keep existing **CalibrationKind::XrAlignment**, data schema version 1 and existing CalibrationProfile. Add to crates/bansayo-contracts/src/calibration.rs:

~~~rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct XrAlignmentData {
    pub q_xr_stage_from_world: Quat,
    pub t_xr_stage_from_world_m: Vec3,
    pub room: Option<String>,
    pub stage_session_id: String,
    pub stage_generation: u64,
    pub camera_fingerprint: String,
    pub grip_tip_offset_m: Vec3,
    pub reference_camera_profiles: Vec<ProfileId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct XrAlignmentQuality {
    pub pivot_sample_count: u32,
    pub pivot_rms_m: f32,
    pub fit_rms_m: f32,
    pub independent_holdout_error_m: f32,
    pub vertical_residual_max_m: f32,
    pub anchor_baseline_min_m: f32,
    pub captured_t_us: TimeUs,
}
~~~

**Profile ID:** xr-alignment.default for no active room, otherwise xr-alignment.<room>. subject device_id=XR-HMD and segment_id=null. origin=live. method=xr_pivot_four_floor_v1. depends_on includes **camera_intrinsics** and **camera_extrinsics** for each participating camera. Use actual content fingerprint in addition to dependencies because profile IDs can remain the same after recalibration.

**XR stage identity:** On OpenXR initialize, Godot generates random 16-byte lowercase hex stage_session_id using Crypto.generate_random_bytes(16), stage_generation=0. On recenter, increment generation; on OpenXR runtime restart, replace session ID. A network reconnect retains stage identity but restarts clock-offset measurement. Neither a saved profile nor old user://xr_alignment.cfg makes a new stage automatically valid.

**Extend existing calibration store:**
- calibration/store.rs::valid_profile: strict typed and finite validation for XrAlignment.
- calibration/store.rs::room_of and in_room: include XrAlignment room while preserving globally shared intrinsics.
- calibration/store.rs::invalidated_by: XR profiles depending on changed intrinsics/extrinsics also become stale, including replaced profile at same ID.
- calibration/store.rs::summaries and summary: include valid XR alignment data in client-visible summary/status, but only send a transform as active if stage+room+fingerprint match.
- registry/persistence.rs: retain existing atomic runtime snapshot, now containing ordinary XR calibration profile. Do not introduce a new file format or separate local alignment database.

### 4.6 Typed state and two HTTP routes

Add to SystemState in crates/bansayo-contracts/src/state.rs a **serde-defaulted, optional** xr_alignment:Option<XrAlignmentStatus>. Define XrAlignmentStatus { state, profile_id, room, stage_session_id, stage_generation, transform, quality, reason_code, camera_fingerprint } and XrStageTransform { q_xr_stage_from_world, t_xr_stage_from_world_m }. Expose transform **only if state==valid**. This is an additive protocol-v1 state field; old clients ignore it. Never alter ServerMessage::Frame.

**GET /api/v1/calibration/xr/anchors**: returns four verified world positions and camera fingerprint from current room profiles. Fail closed with 409 if fewer than two valid matching cameras, metadata absent or hardware identity unconfirmed.

**POST /api/v1/calibration/xr/solve**: typed request with fields protocol_version=1, room, camera_fingerprint, stage_session_id, stage_generation, controller=right_grip, pivot_samples and an ordered anchors array with A/B/C/D sample groups. Sample fields: t_client_us:u64, p_stage_m:Vec3, q_stage_from_grip:Quat. The client does **not** submit world anchor positions, a fitted transform, or profile status. Rust resolves known anchors from camera profiles, fits pivot/transform, validates and persists.

**Caps:** 64 KiB route-specific HTTP body max, <=64 pivot samples and <=24 per anchor, finite numeric bounds and validated identifiers. Return 400 invalid capture, 403 non-loopback, 409 stage/room/fingerprint changed or recording/replay/concurrent operation, and 503 persistence failure using the existing ApiResponse/ApiError envelope. Do not widen global 16KiB API limit merely for this endpoint.

**Single writer:** Add a typed command to pipeline/command.rs and dispatch through existing runtime.rs pipeline command branch. Compute calibration in a pure helper. Persist via existing snapshot save machinery and only publish valid state after save succeeds. On failure never leave a memory-only valid profile. Prevent XR calibration solve while recording or replaying; room change already refuses while recording.

## 5 — Workstream C: validity and invalidation rules

| Trigger | Result / reason |
|---|---|
| No XR alignment profile | missing; never claim measured |
| Profile exists, room, stage ID/generation and camera fingerprint all match | valid; expose transform |
| XR stage-session changed | stale, no transform |
| XR recentered; generation increased | stale immediately, no transform |
| New WebSocket for same stage, no recenter | profile may remain valid; **clock mapping resets** |
| Room changed from A to B | withdraw A; B profile must pass current stage and camera fingerprint |
| Camera intrinsics/extrinsics replaced, even same ID | stale via content fingerprint |
| Camera hardware identity not confirmed | stale; don't trust stored world |
| Camera moved but user did not declare movement | cannot be automatically guaranteed; explicitly warn, ask to recalibrate |
| Pivot pointer changed | user must recapture; software cannot detect arbitrary physical modifications |
| Calibration request failed QA | no partial save; prior valid profile remains valid only if its dependencies still hold |
| Old Godot user://xr_alignment.cfg | unverified/assumed preview only, **never** authoritative measured calibration |
| Recording/replay | refuse solve; replay must not apply unrelated live headset transform |
| Invalid or corrupt stored profile | invalid; preserve for diagnostics rather than auto-delete |
| No XR runtime/desktop | existing camera-only and simulator behavior unchanged |

**Immediate invalidation:** When Godot receives OpenXR pose_recentered, it must (a) clear BodyRoot's measured transform, (b) mark clinic alignment false, (c) increment stage_generation and send StageInfo, (d) cancel calibration capture and (e) wait for new validated engine state. No old world-aligned overlay may continue while the server processes the signal.

**Calibration profile stores stage identity and camera fingerprint** so a previous profile loaded from disk is **stale until** the current live XR stage is known and matches. Never default to valid after engine restart without StageInfo. Preserve room-specific profile history without confusing another room's extrinsics.

## 6 — Godot implementation: compact guided wizard

Keep existing training_room.gd, engine_client.gd, engine_actions.gd, status_panel.gd, BodyRoot and separate headset camera. Phase 01 already added XR grip nodes and the separate XR uplink; **do not** recreate those nodes or start a second socket.

**New files:**
- apps/vr/scripts/xr_clock_sync_client.gd: t0/t3 monotonic clock capture and sync-message logic, no extra connection.
- apps/vr/scripts/xr_alignment_capture.gd: pure stage sample collector, bounds and tracking validity, no HTTP, no UI.
- apps/vr/scripts/xr_alignment_wizard.gd: one small state machine, current step, progress, API interactions, actionable failures.
- apps/vr/tests/xr_alignment_smoke.gd: headless synthetic transforms and wizard transitions.

**Wizard state sequence:**

~~~text
idle
 -> prerequisites (GET anchors)
 -> pivot_instructions (fixed right grip tip against A)
 -> pivot_capture (5 seconds, >=32 diverse tracked poses)
 -> touch_A (12 stationary samples over >=0.30s)
 -> touch_B
 -> touch_C
 -> touch_D
 -> solving (POST /calibration/xr/solve)
 -> success (new engine SystemState.xr_alignment valid)
 -> idle

Any recenter, bad tracking, room change or invalid metrics
 -> explicit_error -> retry or restart entire capture
~~~

Show the wizard as a compact in-world setup control **Align XR room**, not a separate full-screen application. Consume right trigger for step confirmation **only while wizard open**; preserve ordinary trigger/grip, clinic and teleport behavior otherwise. Present numeric pivot RMS, fit RMS and independent D error returned by Rust. Never invent validation success from client-side geometry alone.

**Apply transform exactly once:** On engine_client.gd state_changed, validate optional xr_alignment data, state=valid and current stage_session_id/generation before mutating _root.transform:

~~~gdscript
var a: Dictionary = state.get("xr_alignment", {})
if a.get("state") == "valid" and matches_current_stage(a):
    var x: Dictionary = a["transform"]
    var qv: Array = x["q_xr_stage_from_world"]
    var tv: Array = x["t_xr_stage_from_world_m"]
    var q := Quaternion(qv[0],qv[1],qv[2],qv[3]).normalized()
    _root.transform = Transform3D(Basis(q), Vector3(tv[0],tv[1],tv[2]))
    _clinic.aligned = true
else:
    _root.transform = Transform3D.IDENTITY
    _clinic.aligned = false
~~~

Pseudocode assumes structural/finite validation before indexing; implementation must fail gracefully on malformed data. Do **not** transform _xr_origin, _xr_camera or user controllers. Current _root is for **engine content** only.

Replace the manual _align_to() from [training_room.gd lines 241–268](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/apps/vr/scripts/training_room.gd#L241-L268) as **source of calibration validity**. Keep the old user:// config file as historical user data; do not delete it, and do not load it as a valid XR profile.

**Runtime-specific caution:** Godot's XRCamera3D node may lag the render thread. Phase 02 timestamps sampled pose acquisition, not actual physical sensor exposure or motion-to-photon latency. Do not use that caveat as a reason to implement custom rendering timing now.

## 7 — YAML configuration: exact fields and bounds

Phase 01 creates engine XrConfig with xr.enabled. Extend **that** section (no duplicate top-level XR config). Create xr.sync and xr.alignment substructs with serde(default,deny_unknown_fields). Off by default when xr.enabled=false. Existing schema_version:1 YAML files without XR remain valid. New enabled example uses schema_version:2.

~~~yaml
schema_version: 2
api:
  bind: 127.0.0.1:7700
engine:
  publish_rate_hz: 60
  stale_after_ms: 250
  data_dir: ./data
rig:
  active_room: null
xr:
  enabled: true
  nominal_rate_hz: 60.0
  max_ingest_rate_hz: 120.0
  max_position_norm_m: 20.0
  max_quaternion_norm_error: 0.005
  max_message_bytes: 4096
  tracking_stale_after_ms: 250
  sync:
    enabled: true
    ping_interval_ms: 250
    max_rtt_us: 10000
    max_uncertainty_us: 5000
    min_samples: 8
    history_samples: 64
    drift_min_span_s: 20
    max_abs_drift_ppm: 200
    status_stale_after_ms: 2000
    max_capture_age_ms: 100
  alignment:
    enabled: true
    min_calibrated_cameras: 2
    board_cols: 7
    board_rows: 5
    min_anchor_baseline_m: 0.60
    min_anchor_triangle_area_m2: 0.10
    pivot_samples: 32
    pivot_capture_s: 5.0
    pivot_min_orientation_span_deg: 25.0
    pivot_max_rms_m: 0.010
    grip_tip_max_offset_m: 0.30
    anchor_samples: 12
    anchor_max_spread_m: 0.008
    fit_max_rms_m: 0.015
    holdout_max_error_m: 0.030
    vertical_max_residual_m: 0.020
    anchor_distance_max_error_m: 0.025
sources: []
~~~

Update Phase 01 planned **config/xr-local.example.yaml** only after Phase 01 exists. Validate all finite/range/shape values with path-qualified xr.sync.* / xr.alignment.* errors. Use 7×5 0.20m board in actual camera calibration; square_m is from measured camera profile, **not** a hardcoded assumed 0.20m in the solver.

Godot keeps native project settings:

~~~ini
[bansayo]
xr_alignment_wizard_enabled=true
xr_sync_ping_interval_ms=250
~~~

Both are subordinate to the existing Phase 01 project setting **xr_uplink_enabled=false**. Never initiate XR network input in normal desktop mode with an off config.

## 8 — Exact file change manifest

All new paths are prescriptive, subject to mandatory main refresh. Phase 01 planned files must exist first.

| Change | File | Exact responsibility |
|---|---|---|
| Edit (Phase 01) | crates/bansayo-contracts/src/xr.rs | Add StageInfo, SyncPing, SyncResult, SyncPong, typed XR solve request and anchors response |
| Edit | crates/bansayo-contracts/src/calibration.rs | Add XrAlignmentData and XrAlignmentQuality; reuse current XrAlignment |
| Edit | crates/bansayo-contracts/src/state.rs | Optional XrAlignmentStatus; no existing field changes |
| Edit | crates/bansayo-contracts/src/lib.rs | Export new shared types |
| **New** | crates/bansayo-engine/src/xr_clock_sync.rs | Bounded four-timestamp sync, jitter uncertainty, drift; deterministic Rust tests |
| Edit | crates/bansayo-engine/src/lib.rs | Export sync helper |
| Edit (Phase 01) | crates/bansayo-engine/src/api/xr.rs | StageInfo and sync handlers; map session timestamps before events |
| Edit (Phase 01) | crates/bansayo-engine/src/sources/xr.rs | Stage tracking and mapped SensorObservation timestamps |
| Edit | crates/bansayo-engine/src/config/schema.rs | Add nested XrSyncConfig and XrAlignmentConfig |
| Edit | crates/bansayo-engine/src/config/validate.rs | Strict bounds and schema gating |
| Edit | scripts/calibrate_cameras.py | Emit quality.xr_anchor_board, sign/anchors/capture ID and extend selftest |
| **New** | crates/bansayo-engine/src/calibration/xr_pivot.rs | Robust grip physical tip pivot solution via nalgebra |
| **New** | crates/bansayo-engine/src/calibration/xr_alignment.rs | 3-anchor yaw/translation and independent D holdout |
| Edit | crates/bansayo-engine/src/calibration/mod.rs | Export solver modules |
| Edit | crates/bansayo-engine/src/calibration/store.rs | XR profile validation, room/fingerprint invalidation, status and summaries |
| Edit | crates/bansayo-engine/src/pipeline/command.rs | Typed single-writer XR solve and stage-change commands |
| Edit | crates/bansayo-engine/src/pipeline/runtime.rs | Solve, quality gates, atomic persistence and state publication |
| Edit | crates/bansayo-engine/src/pipeline/control.rs | Room/recenter/camera invalidation hooks and recording guard |
| Edit | crates/bansayo-engine/src/pipeline/runner.rs | Publish optional SystemState.xr_alignment |
| Edit | crates/bansayo-engine/src/api/mod.rs | GET anchors and POST solve; local-only, route-specific 64KiB |
| Edit | crates/bansayo-engine/src/registry/persistence.rs | Restore validated XR profiles through existing RuntimeSnapshot |
| Edit | crates/bansayo-engine/src/sources/mod.rs | Update existing runtime TimeSyncStatus |
| **New** | crates/bansayo-engine/tests/xr_sync_alignment.rs | Network, status, persistence, replay, room/recenter tests |
| Edit | crates/bansayo-contracts/tests/contracts.rs | Strict types/golden protocol-v1 regression |
| Edit | crates/bansayo-contracts/tests/support/examples.rs | XR profile, anchors, ping and state examples |
| Edit | crates/bansayo-contracts/tests/support/generated.rs | TypeScript exporter and BANSAYO_BLESS |
| Edit (Phase 01) | apps/vr/scripts/xr_pose_uplink.gd | Same socket handles sync/stage messages; no second socket |
| **New** | apps/vr/scripts/xr_clock_sync_client.gd | t0/t3 capture and pending clock requests |
| **New** | apps/vr/scripts/xr_alignment_capture.gd | Bounded tracked stage-pose collection, no HTTP |
| **New** | apps/vr/scripts/xr_alignment_wizard.gd | Compact wizard and API request lifecycle |
| Edit | apps/vr/scripts/training_room.gd | Run wizard, stage ID/recenter generation, engine-owned BodyRoot transform |
| Edit | apps/vr/scripts/status_panel.gd | Alignment status and actionable failure text |
| Edit | apps/vr/scripts/engine_actions.gd | Typed calibration REST helpers |
| Edit | apps/vr/project.godot | Wizard config, preserve disabled XR uplink default |
| **New** | apps/vr/tests/xr_alignment_smoke.gd | Headless fake-stage transform and wizard state tests |
| Edit (Phase 01) | config/xr-local.example.yaml | Enabled-only example with sync/alignment sections |
| Edit | apps/vr/README.md | Physical and test instructions, expected states |
| Edit | docs/13-implementation-status.md | Real/virtual Phase 02 validation, do not overstate hardware |
| Edit | .github/workflows/ci.yml | New Godot smoke test with pinned installation |

**Explicitly leave alone:** pipeline/solve.rs, pipeline/fusion.rs, pipeline/triangulate.rs, body proportions, camera optical detector algorithms, avatar_renderer.gd and martial arts coaching/training scoring.

## 9 — Tests and acceptance gates

### 9.1 Rust clock tests

- [ ] xr_clock_zero_delay_no_offset.
- [ ] xr_clock_positive_and_negative_offset_i128_no_underflow.
- [ ] xr_clock_asymmetric_1ms_3ms_delay_with_bound.
- [ ] xr_clock_rejects_unmatched_or_reused_request_id.
- [ ] xr_clock_rejects_negative_roundtrip.
- [ ] xr_clock_low_rtt_selection_stable_under_9ms_jitter.
- [ ] xr_clock_drift_50ppm_over_30_seconds within provisional 15ppm.
- [ ] xr_clock_unsettled_and_stale_fall_back_to_receive_time.
- [ ] xr_clock_reconnect_resets_clock_but_preserves_same_stage_id.
- [ ] xr_clock_new_stage_id_invalidates_clock_and_alignment.
- [ ] xr_clock_mapped_sample_never_claims_future_or_reordered_time.
- [ ] xr_clock_recording_replay_preserves_original_t_us.

### 9.2 Alignment solver unit tests

- [ ] xr_pivot_known_local_tip_offset recovered within 2mm under zero noise.
- [ ] xr_pivot_32_diverse_samples_passes, same-orientation input fails.
- [ ] xr_pivot_pointer_slip_and_large_outliers_fail.
- [ ] xr_alignment_identity, yaw_90, yaw_180.
- [ ] xr_alignment_both_board_normal_signs and no mirrored axis.
- [ ] xr_alignment_swapped_B_C and wrong scale fail.
- [ ] xr_alignment_collinear_A_B_C fails.
- [ ] xr_alignment_holdout_D_31mm_fails, 29mm passes (other gates satisfied).
- [ ] xr_alignment_vertical_inconsistency fails.
- [ ] xr_alignment_rejects_nonfinite_samples and near-zero quaternion.
- [ ] xr_alignment_profile_fingerprint_changes_with_same_id_new_extrinsics.
- [ ] xr_alignment_no_anatomical_wrist_or_skeleton_mutation.

### 9.3 Engine API, persistence and replay

New **crates/bansayo-engine/tests/xr_sync_alignment.rs**, using actual Axum router, in-memory deterministic XR tuples, tempfile data dir and stable synthetic camera profiles:

- [ ] xr_anchors_require_two_current_matching_camera_profiles.
- [ ] xr_anchors_missing_metadata_does_not_break_camera_triangulation.
- [ ] xr_solve_requires_live_stage_id_matching_generation.
- [ ] xr_solve_rejects_wrong_room_or_camera_fingerprint.
- [ ] xr_solve_refuses_recording_replay_and_simultaneous_calibration.
- [ ] xr_solve_validates_all_four_anchor_samples_and_body_limit.
- [ ] xr_solve_persistence_failure_keeps_state_unmodified.
- [ ] xr_profile_restart_is_stale_until_same_live_stage_known.
- [ ] xr_recenter_clears_valid_transform_immediately.
- [ ] xr_switch_room_does_not_apply_previous_room_alignment.
- [ ] xr_camera_intrinsics_change_invalidates_dependent_rooms.
- [ ] xr_camera_extrinsics_change_invalidates_matching_room.
- [ ] xr_camera_identity_unconfirmed_marks_alignment_stale.
- [ ] xr_old_protocol1_state_recording_without_new_field_still_loads.
- [ ] xr_accepted_event_recording_replay_preserves_estimated_t_us.
- [ ] xr_camera_only_and_imu_only_fixtures_have_identical_poses.
- [ ] xr_alignment_endpoints_refuse_non_loopback_peer.

### 9.4 Python + Godot tests

Run **python scripts/calibrate_cameras.py selftest** with new assertions for both up signs, board geometry, A/B/C/D and consistent capture IDs. Old camera profiles remain loadable; their lack of anchor metadata only affects XR alignment readiness.

Godot new **apps/vr/tests/xr_alignment_smoke.gd** must test identity, nonzero XR origin offset (currently 0,0,3), 90-degree world/stage transform, only BodyRoot moved, stale/reset removes old transform, stage recenter increments generation and clears wizard arrays, tracking loss retries step, desktop mode unchanged and legacy user:// file not used as measured. It must not require a headset for the synthetic tests.

**All commands after implementing Phase 01:**

~~~bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
BANSAYO_BLESS=1 cargo test -p bansayo-contracts
cargo test --workspace
node scripts/verify-gate1.mjs
node scripts/verify-gate2.mjs
python scripts/calibrate_cameras.py selftest
godot --headless --path apps/vr --editor --quit
godot --headless --path apps/vr --xr-mode off --script tests/contract_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/spatial_ui_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/xr_observation_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/xr_alignment_smoke.gd
~~~

Run contract test **again without BANSAYO_BLESS** and verify zero generated changes. OpenCV>=4.7 and numpy are needed for Python camera selftest. If a tool or physical headset is unavailable, report **BLOCKED**, not PASS. Update CI to include Godot test with reproducible pinned Godot environment; never silently skip.

## 10 — Physical hardware validation protocol

Save at **docs/validation/YYYY-MM-DD-phase02-xr-alignment-time-sync.md**.

1. Record commit SHA, OS, Godot and OpenXR versions, headset/controller, camera IDs/FPS, active room and camera fingerprint.
2. Assemble 7×5 board with physically measured square 0.20m. Calibrate and import at least two real room cameras using same floor board, verify existing camera RMS threshold.
3. Confirm Phase 01 engine sees HMD and both controller grips and receives stage poses.
4. Run 5s pivot: require 32 valid varied samples, RMS <=10mm; record fitted right-grip local tip offset.
5. Capture A, B, C, D in order. Record per-anchor spread, A/B/C fit RMS, D independent 3D holdout error. Provisional target D<=30mm.
6. Independently retouch D and then A/B/C, checking transform agreement beyond fit samples and no sign reversal.
7. Verify stage Y/floor and physical distance within expected units. Do not pretend a 0.04m board is a 0.20m board.
8. Recenter via headset: alignment state becomes stale and old BodyRoot overlay clears before further use. Recalibrate, then reconnect WS with same stage ID; clock estimate must resettle while valid alignment can remain.
9. Restart Godot/OpenXR: new stage ID must make saved profile stale. Restart engine: persisted profile readable but only valid after matching live session evidence.
10. Switch camera rooms, reimport camera extrinsics at same profile ID, and mark camera hardware identity unconfirmed; each must revoke old matching alignment.
11. Measure 60s of loopback four-timestamp RTT, offset, uncertainty and drift; aim qualifying RTT<=10ms and bound<=5ms but log measured p50/p95. Inject lag and ensure receive-time fallback.
12. Record XR observations and replay; verify mapped session t_us, device_time_us and time_quality equal the accepted recording.
13. Run all camera-only, SlimeVR and Godot desktop regression scenarios. Do not move headset camera or alter native aim interactions.
14. Report measured error against independent physical contact marks and actual latencies. Distinguish **PASS**, **FAIL** and **BLOCKED** per test.

**Important truthfulness limit:** The camera pipeline's [sources/camera/timing.rs](https://github.com/danielsobrado/Bansayo/blob/104a8864649ad99d52e8b68e64d7dac1cf080ef7/crates/bansayo-engine/src/sources/camera/timing.rs) records arrival intervals and does not prove exposure start timestamps. Therefore alignment and clock mapping do **not** establish a fully synchronized physical multi-camera+XR capture pipeline. That end-to-end temporal accuracy must be measured in later physical fusion validation.

## 11 — Exact implementation order and seven direct-to-main commits

**Gate 0:** Confirm Phase 01 is *actually implemented*, tests green, XR grip and head devices present. If not, complete Phase 01 before any Phase 02 changes. This document commit does not implement either phase.

### Commit 01 — Shared contracts
- [ ] Refresh main, record target SHA and exact current paths.
- [ ] Add XR sync/stage messages, typed XR alignment request/result/data/quality, optional SystemState.xr_alignment.
- [ ] Regenerate TypeScript/golden JSON using BANSAYO_BLESS.
- [ ] Run cargo fmt, tests and clippy.
- [ ] Commit **contracts: define XR clock synchronization and room alignment state**.
- **Gate:** No protocol-version change and old client/golden tests pass.

### Commit 02 — Timestamp mapping
- [ ] Implement bounded XRClockSync estimator; integrate into **same** Phase 01 XR uplink socket.
- [ ] Map each XR frame once before pipeline registry/recording; publish TimeSyncStatus.
- [ ] Test RTT, jitter, drift, stale fallback, restart and replay.
- [ ] Commit **engine: estimate XR capture timestamps with bounded uncertainty**.
- **Gate:** No ungrounded Synchronized flag or invalid future timestamp; no camera/IMU regression.

### Commit 03 — Camera-board anchor provenance
- [ ] Add quality.xr_anchor_board to Python camera extrinsics profiles.
- [ ] Add Rust camera fingerprint, matching camera readiness and GET anchors.
- [ ] Run Python selftest with +1/-1 signs; confirm old camera profiles unchanged for triangulation.
- [ ] Commit **calibration: derive room-specific XR anchors from the ChArUco board**.
- **Gate:** Exact metric anchor geometry, two current cameras, deterministic fingerprint.

### Commit 04 — Pure pivot + yaw solvers
- [ ] Implement calibration/xr_pivot.rs and calibration/xr_alignment.rs, no networking/UI dependencies.
- [ ] Fit A/B/C, validate independent D, reject invalid/degenerate captures.
- [ ] Execute complete numerical test matrix.
- [ ] Commit **calibration: solve measured right-grip contact point and XR stage transform**.
- **Gate:** Identity/90°/180°, sign, fit residual and holdout errors all deterministic.

### Commit 05 — Engine-owned persistence and invalidation
- [ ] Extend existing calibration Store typed validation, room filtering and invalidation.
- [ ] Add POST solve using existing command single-writer and atomic snapshot.
- [ ] Add stage recenter/generation and camera/room revision invalidation.
- [ ] Test persistence rollback, restart/replay/recording guards.
- [ ] Commit **engine: persist and invalidate room-specific XR alignment profiles**.
- **Gate:** No valid profile survives a mismatched room/stage/camera revision.

### Commit 06 — Guided Godot capture and transform
- [ ] Add clock client helper, capture component, compact wizard and headless smoke tests.
- [ ] Preserve XR aim controller UI, native camera, mirror, clinic and desktop.
- [ ] Apply state-published transform **once** to BodyRoot; old local cfg is assumed only.
- [ ] Handle recenter immediately and cancel pending captures.
- [ ] Commit **vr: guide physical XR room calibration and apply measured alignment**.
- **Gate:** Godot headless and legacy UI pass, no false measured state.

### Commit 07 — Documentation, CI and hardware signoff
- [ ] Update apps/vr/README.md, docs/13-implementation-status.md, CI and validation report.
- [ ] Run complete command list; check clean generated contracts.
- [ ] Record physical hardware outcomes or explicit BLOCKED fields.
- [ ] Verify latest main and no unrelated file overwritten.
- [ ] Commit **docs: verify Phase 02 XR room alignment and clock synchronization**.
- **Gate:** Reproducible test evidence, honest physical limits, no protocol regression.

## 12 — Non-goals and completion report

**Defer:** anatomical wrist/head offsets and body dimensions to Phase 03; camera/XR/SlimeVR pose fusion, elbow IK and confidence weighting to Phase 04; humanoid skinned mesh retargeting to Phase 05; 33-point heels/toes to Phase 06. Do not add new network brokers, custom tracking drivers, camera processing frameworks, Kinect, OSC/VMT, Unreal/Unity packages or GPL upstream source.

**Implementation AI must report:** Phase 01 prerequisite SHA, Phase 02 before/after main SHA, seven commit links, file manifest and actual code reference deviations; exact cargo/Godot/Python command outcomes; wire examples and clock offset/uncertainty; camera anchor/fingerprint provenance; pivot fitted tip, A/B/C RMS, D holdout error; room/recenter/restart invalidation proof; recording/replay timestamp equivalence; physical headset validation PASS/FAIL/BLOCKED; and any remaining **TODO** with owner and file path.

**Definition of done:** Rust owns a verifiable T_xr_stage_from_world and a bounded Estimated timestamp mapper, persistent as ordinary calibration/recorded observations. Godot applies that transform only to engine body content and invalidates it safely. Existing camera/IMU/simulator output, historical recordings and protocol-v1 clients remain compatible. **No Phase 04 fusion is claimed.**
