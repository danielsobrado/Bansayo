# Bansayo Phase 07 — End-to-end hardware validation, accuracy and release gates

**Status:** Prescriptive implementation and physical experiment plan, NOT completed hardware validation.  
**Date:** 2026-11-21.  
**Target:** [danielsobrado/Bansayo](https://github.com/danielsobrado/Bansayo); all approved changes committed directly to `main`, never force-push.  
**Verified current Bansayo source baseline:** [c86ebd6d0277badcb9116c275add5359023a4b6b](https://github.com/danielsobrado/Bansayo/commit/c86ebd6d0277badcb9116c275add5359023a4b6b).  
**Reference baseline:** [maceq687/FullBodyPoseEstimation@be895709](https://github.com/maceq687/FullBodyPoseEstimation/commit/be895709c0bf86d35f64eb2cd5d2ffb4c6838593).  
**Prior phases:** [01 XR observations](phase-01-openxr-observation-pipeline-implementation.md) · [02 XR stage/world and clocks](phase-02-openxr-room-alignment-and-time-synchronization-implementation.md) · [03 athlete calibration](phase-03-body-proportions-t-pose-and-controller-wrist-calibration-implementation.md) · [04 camera/XR/IMU arm fusion](phase-04-confidence-aware-xr-camera-imu-fusion-and-elbow-ik-implementation.md) · [05 skinned avatar](phase-05-godot-humanoid-avatar-retargeting-and-vr-presentation-implementation.md) · [06 foot-model evaluation](phase-06-foot-landmark-model-benchmark-and-conditional-integration-implementation.md).  
**Canonical experiment references:** [16 — Accuracy and validation](architecture/martial-arts/16-accuracy-and-validation.md), [15 — Performance and latency](architecture/martial-arts/15-performance-and-latency.md), [29 — Tracking hardware laboratory](architecture/martial-arts/29-tracking-hardware-lab.md), and [validation/README.md](validation/README.md).  
**Deliverable:** Automated offline verification and an operator-reproducible physical test package, measured reports and evidence-based **GO / LIMITED / NO-GO / BLOCKED** decision for actual body tracking with 2–4 cameras, OpenXR HMD/controllers, SlimeVR and optionally direct ESP32 IMUs.

> **Critical fact at this verified HEAD:** Phases 01–06 are implementation **documents**. The corresponding XR observation, room-alignment, athlete-measurement, XR elbow fusion and skinned-avatar code has not been demonstrated by their docs. Existing real-world results cover **one 30 FPS UGREEN webcam** and partial device/camera diagnostics, **not** a validated 2–4 camera + XR + SlimeVR system. It is prohibited to declare Phase 07 passing until the relevant earlier phases exist as tested code and all required physical experiments are actually performed. A blocked or limited hardware result is an acceptable, truthful outcome.

## 0 — Definition of the product claim, not of a simulator demo

The goal of Phase 07 is to answer, with independent evidence, these questions:

1. Can two, three and four physically installed cameras keep stable identities, negotiated rates, correct calibration and metric world positions **throughout martial-arts movement**?
2. Does a common floor/world frame accurately transform measured camera poses, the headset stage and both controller grips without recenter or left/right errors?
3. How much time uncertainty, age and display lag is introduced by camera exposure, capture, GPU inference, session timestamps, the SlimeVR SolarXR feed, the engine tick, WebSocket, Godot's pose buffer and headset compositor?
4. Do real athlete body measurements, controller-local wrist offsets and the confidence-aware arm solver remain geometrically plausible with occlusion and rapid punching?
5. Do SlimeVR bands improve **specific validated outputs**, including knee/hip angles, event timing and occlusion resilience, rather than just making a rendered avatar move?
6. Can the engine explain and suppress **unsupported** positions, timing, speeds and coaching scores, without fabricating confidence or world position?
7. Can recordings reproduce measured/live decisions under deterministic replay, after reconnect, athlete switch, camera/room changes and VR recentering?
8. Can an actual Windows 11 + GPU + USB/Wi-Fi/OpenXR deployment run uninterrupted, within measured frame and memory budgets, with no unsafe VR presentation or corrupted session?
9. Which configurations can be **recommended and claimed** for karate, taekwondo and Muay Thai technique training, and which are only visual/simulated?

### 0.1 No false equivalence

- **Synthetic ground truth** proves correct mathematical implementation, not real motion-capture accuracy.
- **A single 30 FPS camera** is useful for basic 2D drills but cannot produce calibrated multi-camera metric 3D by itself.
- **SlimeVR SolarXR orientations** use receive-time with configured latency, not hardware sample time, raw acceleration or true absolute global position. Do not call them clock-synchronized measurements.
- **Direct ESP32 `bansayo.imu.v1`** carries a wrapped device timestamp, gyro and acceleration (when firmware exists); this must be validated separately, not averaged into SolarXR latency statistics.
- **OpenXR tracked grip positions** are not anatomical wrists without Phase 03 calibration, and runtime pose time may represent a predicted display instant, not camera exposure.
- **Godot's 50ms interpolation buffer** is a presentation delay. It is not a hardware latency measurement.
- **Multi-camera reprojection error** is not ground-truth spatial error. A calibrated wand/floor/physical reference provides independent error.
- **Camera/IMU agreement** is not ground-truth dynamic accuracy. Quantify the available reference and label agreement when no calibrated external measurement exists.
- **A higher camera FPS request** is not delivered FPS. Capture may drop below rate due to exposure, USB bandwidth, CPU and shared inference worker.
- **A good rendered avatar** cannot compensate for incorrect world frames, old observations, stale calibration or an invalid model-rights decision.
- **A GO decision is scoped** to actually tested hardware, movement, lighting, reference, athlete population, posture and sensor configuration.

## 1 — Exact upstream-to-target source map, pinned to commits

### 1.1 Upstream concepts (independent evaluation only)

| Upstream reference at `be895709` | Verified code behavior | Why Phase 07 must independently test Bansayo |
|---|---|---|
| [VNectModel.cs L122–132, L169–183](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L169-L183) | Reads HMD and two controller device poses per Unity update | Validate Bansayo OpenXR tracking presence, role identity, timestamps, handedness and device disconnection; do not assume Unity conventions translate to Godot. |
| [VNectModel.cs L189–214](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L189-L214) | Uses HMD-relative controller positions, hard-coded controller Euler offsets and a “phantom elbow” | Use independent controller-tip and wrist-reference fixtures to show that Bansayo's calibrated transform and IK do **not** share those systematic errors. |
| [VNectModel.cs L217–236](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L217-L236) | Device initialization/calibration triggered by UI | Test lifecycle, recenter, session resets, untracked controller, recovery; not just initial successful pose. |
| [VNectModel.cs L603–670](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L603-L670) | Guided T-pose/VR scale procedure based partly on controller span/HMD height | Validate actual measured left/right arm lengths and offsets against independent physical landmarks; do not treat eye height or controller span as human bone measurements. |
| [PoseVisualizer3D.cs L51–78](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/PoseVisualizer3D.cs#L51-L78) | Maps BlazePose-style model-relative landmarks into Unity display coordinates with scaling | Test 3D world landmark metres separately from model-relative points; no model-relative pose may be relabeled camera world. |
| [VmtReceiver.cs L70–100, L105–168](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VmtReceiver.cs#L70-L168) | Parses OSC/VMT messages including tracker positions and quaternions | Do not assume VMT time/frame/calibration semantics. Bansayo source events and replay, not upstream OSC messages, are the test authority. |
| [LICENSE](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/LICENSE) | GPL-3.0 reference project | No copied C#/Unity assets, code or model binaries. |

### 1.2 Existing Bansayo implementation and hard validation hooks

**Every code link in this table refers to the verified `c86ebd6` tree.** Re-map on main before implementing. Documentation and planned files from Phases 01–06 are distinguished from existing production code.

| Target file and exact lines | Current actual behavior | Phase 07 harness/validation task |
|---|---|---|
| [sources/camera/capture.rs L251–269, L443–480](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/sources/camera/capture.rs#L251-L480) | Physical camera, RTSP, network and image-folder capture adapters; devices/modes | Inspect each physical camera's exact USB identity, supported resolution/FPS and actual negotiated mode; compare USB/RTSP modes honestly. |
| [sources/camera/identity.rs](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/sources/camera/identity.rs) | Serial/interface/hardware fingerprint matching | Test two identical devices, USB-port reorder, unplug/replug and calibration identity invalidation. |
| [sources/camera/lifecycle.rs L112–210, L228–258](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/sources/camera/lifecycle.rs#L112-L258) | Mode negotiation; publishes Active/Reconnecting/Degraded, frame rate and capture faults | Assert actual FPS and health reasons, not nominal `rate_hz`. Capture reconnect time and dropped frame count. |
| [sources/camera/timing.rs L1–55](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/sources/camera/timing.rs#L1-L55) | Arrival intervals measured over 300 samples; actual FPS/jitter/p95/p99; **not exposure time** | Compare delivered rate/interval histograms to external LED reference. Never interpret arrival jitter as exposure-time uncertainty directly. |
| [sources/camera/pose.rs L1–24, L141–300](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/sources/camera/pose.rs#L1-L300) | YOLOX+RTMPose COCO17, fixed single-person tracking, one ROI, raw keypoints | Measure inference tracked/redetect separately, occlusion, side swap, fast strike blur and selected subject errors. Phase 06 model stays optional/not assumed. |
| [sources/camera/mod.rs L174–291, L319–478, L479–608](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/sources/camera/mod.rs#L174-L608) | One latest-frame slot per camera; one inference thread services sources; old frames dropped if overloaded; raw `landmarks_2d` emitted | Validate 2/3/4 physical simultaneous feeds, per-camera fairness, missed-frame counts and bounded latency, not just a single camera. |
| [pipeline/triangulate.rs L89–181, L202–315](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/pipeline/triangulate.rs#L89-L315) | Native multi-view DLT with min views, reprojection and ray-angle gates; matching source schemas | Use measured wand position/length over the entire exercise volume, not only mean image reprojection. |
| [config/schema.rs L292–311](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/config/schema.rs#L292-L311) | Default `triangulation.max_skew_ms=20`, `min_views=2`, ray angle 3°, reprojection limit 15px | Compare gated versus aligned with measured camera time offset and mark defaults provisional; do not silently loosen without an experiment. |
| [scripts/calibrate_cameras.py L1–65, L66–125](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/scripts/calibrate_cameras.py#L1-L125) | Python **offline-only** ChArUco board, capture, solve and engine import; 1px intrinsics/2px extrinsics RMS constants | Run intrinsics/rig/floor calibration with measured square size and holdout board views; capture uses OpenCV outside running engine. |
| [calibration/store.rs](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/calibration/store.rs) | Profile persistence, active-room filtering, invalidation | Prove camera/room/athlete/stage invalidation; snapshot and restore profile/quality states. |
| [clock.rs L10–40, L45–156, L180–212](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/clock.rs#L10-L212) | SessionClock; rolling offset estimator, drift, uncertainty, reset; SourceTime arrival-minus-latency fallback | Validate what **each** source time quality actually means against LED reference. Do not call receive-time synchronized. |
| [sources/slimevr/solarxr.rs L24–58, L183–268](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/sources/slimevr/solarxr.rs#L24-L268) | Decodes SolarXR tracker IDs, raw/reference-adjusted quaternions, status, battery and poll rate | Validate actual announced tracker keys, raw vs reference-adjusted path and axis conventions, stable IDs. |
| [sources/slimevr/mod.rs L136–235](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/sources/slimevr/mod.rs#L136-L235) | Accepts usable tracker statuses, emits receive-time orientations; configurable latency; no native sample timestamps/gyro | Measure network/feed delay empirically, periodic keepalive, disconnection and heading drift, not imaginary sample timestamps. |
| [sources/esp32.rs L1–115](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/sources/esp32.rs#L1-L115) | Independent optional 64-byte `bansayo.imu.v1` UDP with device clock, gyro and accel; no firmware shipped | Run only when custom hardware/firmware exists; measure wrap/reboot, time uncertainty and 6-axis range. |
| [calibration/mounting.rs L1–40](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/calibration/mounting.rs#L1-L40) | One-second mounting reference capture; max spread 0.035 rad | Measure repeatability, strap slip and stable IMU-to-segment mounting before fast kicks. |
| [pipeline/align.rs L1–82](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/pipeline/align.rs#L1-L82) | No-extrapolation SLERP, chooses reference time for fresh streams | Validate stale data never extrapolated; source gaps and sync quality show reasons rather than pretending to measure. |
| [pipeline/fusion.rs L1–94](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/pipeline/fusion.rs#L1-L94) | Rung-1 IMU heading correction and held camera root, with deterministic state | Validate slow correction, drift, conflict reasons and no body-root jump on slow-to-fast transitions. Phase 04 arm fusion, if implemented, must pass its own tests too. |
| [pipeline/runtime.rs L216–245, L390–488](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/pipeline/runtime.rs#L216-L488) | One solver loop producing `MotionSample`, `ServerMessage::Frame`, source events, training and recording | Measure actual publish rate, time/quality metadata, dropped analysis messages; no hidden second solver tick. |
| [diagnostics.rs L1–68, L93–193](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/diagnostics.rs#L1-L193) | Seven timing stages; 10s percentile windows; per-stage budgets; opt-in `diagnostics.timing` | Preserve existing stage metrics, add **separate optional** hardware test markers when a missing quantity needs instrumentation. |
| [recording/mod.rs L17–76](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/recording/mod.rs#L17-L76) | Context includes skeleton/config/devices/links/profiles/room, used for input replay | Snapshot exact calibration/athlete/algorithm identity before every take; compare recorded versus reconstructed results. |
| [recording/replay.rs L27–69, L118–155](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/recording/replay.rs#L27-L155) | Replay restores context; seek resets and applies events, solves at a selected time | Validate **exact** sequential versus seek output after stateful Phase 04 additions; do not assume equivalent when history not reconstructed. |
| [pipeline/capability.rs L256–299](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/pipeline/capability.rs#L256-L299) | Computes `tracking_config_id` and `calibration_set_id` | Log both for every take, verify profile/hash/algorithm changes break comparability series. |
| [api/mod.rs L45–85, L150–191](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/src/api/mod.rs#L45-L191) | State/stream/skeleton/recording/calibration endpoints and source status | Prefer existing HTTP/WS APIs; don't introduce a remote authenticated test backend or bypass safety/permissions. |
| [apps/vr/scripts/engine_client.gd L1–110](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/apps/vr/scripts/engine_client.gd#L1-L110) | Protocol-v1 WebSocket and bounded packet processing; skeleton/frame/state/training topics | Verify real headset connection, rate, disconnect and reconnection; no fake frame from old skeleton rev. |
| [apps/vr/scripts/pose_buffer.gd L1–72](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/apps/vr/scripts/pose_buffer.gd#L1-L72) | 50ms rendering buffer, 16 frames max, 500ms stale timeout, interpolation only | Measure actual camera→VR *optical* latency independently; ensure no extrapolation or stale opaque limbs. |
| [apps/vr/scripts/training_room.gd L241–268, L331–407](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/apps/vr/scripts/training_room.gd#L241-L407) | Existing **manual local controller-on-board** XR alignment; XR stage and navigation | Validate Phase 02 engine-owned alignment replaces the local approximation, handles recenter, and HMD/camera remain independent. |
| [apps/vr/tests/contract_smoke.gd](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/apps/vr/tests/contract_smoke.gd) | Existing headless VR protocol checks | Add physical tests without dropping deterministic smoke coverage. |
| [tests/hybrid_fusion.rs](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/crates/bansayo-engine/tests/hybrid_fusion.rs) | Synthetic multi-camera/IMU fusion regression, **not physical accuracy** | Retain exact synthetic golden checks, then compare against independent physical real-world evidence. |
| [scripts/engine-test-harness.mjs L1–125](https://github.com/danielsobrado/Bansayo/blob/c86ebd6d0277badcb9116c275add5359023a4b6b/scripts/engine-test-harness.mjs#L1-L125) | Isolated spawned engine/API/stream test harness with temporary directories | Reuse for headless identity/time/replay/observability tests. Do not automate live webcam unplug through synthetic mocks and claim hardware passed. |

### 1.3 Existing physical evidence, not to be overwritten

| Current record | Verified result and limitation | Phase 07 implication |
|---|---|---|
| [2026-11-18 phase2 camera bring-up](validation/2026-11-18-phase2-camera-bringup.md) | One physical UGREEN 1280×720 MJPEG **30 FPS** webcam; one-camera release decode p95 **4.3ms**, inference p95 **22.0ms** with a full person detector on every frame | Historical baseline only. Hardware is not a 60/120 FPS source; do not claim the legacy 12ms inference budget was met. |
| [2026-11-19 hybrid owner run 2](validation/2026-11-19-hybrid-owner-run-2.md) | One webcam opened by identity, manual exposure readback, unplug/replug, degraded-rate feedback; second camera/room triangulation and SlimeVR **not run** | Reuse device, don't repeat synthetic proof; prioritize second camera, calibration, release inference and real sensors. |
| [2026-11-19 hybrid synthetic](validation/2026-11-19-hybrid-synthetic.md) | Camera/IMU hybrid shows good simulated relative behavior under prescribed noise and latency | Useful regression baselines, **never** a statement of real-world gyro/optical precision. |
| [HYBRID-OPEN-ITEMS](validation/HYBRID-OPEN-ITEMS.md) | Second identical webcam, moved camera, IMU+camera tracking and ESP32 firmware remain open | Phase 07 checklist must close these with real observations or mark BLOCKED. |
| [HYBRID-OWNER-CHECKLIST](validation/HYBRID-OWNER-CHECKLIST.md) | C-01…C-09, S-01…S-06, H-01…H-07, E-01…E-02 physical tests | Preserve names and attach new Phase 07 outcomes to old IDs; no duplicate contradictory checklist. |

### 1.4 External accuracy and runtime authorities

- [Khronos OpenXR specification](https://registry.khronos.org/OpenXR/specs/1.1/html/xrspec.html): stage/local reference space, grip versus aim pose, tracking validity and predicted display time. The user's actual XR runtime is the authority for reported frame pacing; camera exposure remains separate.
- [SlimeVR Server source](https://github.com/SlimeVR/SlimeVR-Server): official SolarXR tracker protocol and tracker lifecycle; Bansayo adapter must not infer absent sensor timestamps.
- [OpenCV ChArUco calibration reference](https://docs.opencv.org/4.x/df/d4a/tutorial_charuco_detection.html): use real measured board dimensions, distortion coefficients and held-out views.
- [AprilTag reference repository](https://github.com/AprilRobotics/apriltag): fiducial family/tag semantics and detector flags where they are actually deployed.
- Existing Bansayo calibration and validation code remains the **source of truth** for current accepted data and metrics.

## 2 — Hardware topology and inventory: exact required tiers

### 2.1 Tiered, incremental physical deployment

| Tier | Hardware | What is objectively testable | What is NOT claimable |
|---|---|---|---|
| H0 — existing | One 30 FPS webcam + Windows PC | Camera identity, manual exposure, real FPS, single-camera 2D, latency upper bound, device recovery | Metric multi-camera 3D, limb-length spatial accuracy, real XR fusion |
| H1 — minimum metric 3D | **Two simultaneously calibrated** cameras, rigid mounts, measured board, floor grid, 240 FPS phone/LED | DLT geometry, floor/volume truth, cross-camera skew, camera-only keypoints | XR wrists, actual SlimeVR improvements |
| H2 — hybrid | H1 + physically mounted, powered, assigned SlimeVR bands with SlimeVR Server | Camera/IMU agreement, heading drift, mounting, occlusion, recovery, input age | Direct gyro timing through SolarXR, or XR controller anatomy |
| H3 — PCVR | H1 or H2 + OpenXR HMD and both controllers + implemented Phases 01–05 | Real stage/world, grip/wrist, body alignment, VR display/frame/latency, elbow IK | Unvalidated high-speed dynamic ground-truth accuracy |
| H4 — accuracy lab | H3 + third/fourth cameras, ≥100/120 FPS independent reference, wand, goniometer/LED | Cross-layout quality, strike timing and dynamic body accuracy within reference limits | Lab-grade MoCap accuracy unless such a reference is actually used |
| H5 — optional direct IMU | H2/H3 + ESP32 boards running actual `bansayo.imu.v1` firmware | Device-time mapping, gyro/accel events, reboot/wrap/saturation | Those streams if only SolarXR devices were tested |

**Release gates** follow the tier achieved. Full Bansayo **PCVR multi-sensor** claim requires H3 minimum plus independent physical accuracy/latency verification. H4 is required for claims about **fast martial-arts accuracy**, not merely for video demonstration.

### 2.2 Equipment to record, not assume

Create a per-run inventory, never hardcode IDs into Rust:
- Windows version, CPU model, GPU model/driver, VR runtime/headset refresh, Godot build and engine commit.
- Camera count 1/2/3/4, verified unique serial or interface IDs, USB controller/topology, bus speed, power, resolution, negotiated format and actual FPS, firmware/driver.
- Lens FOV/intrinsics, focus, distortion, shutter type (rolling/global if known), manual exposure/gain, lighting/lux and mains flicker.
- ChArUco printed board row/column size, actual measured square and marker dimension, flatness, fixed stage origin, floor height and orientation.
- Wand: two rigid endpoint targets separated by caliper-measured length (about 500mm), with uncertainty and photograph of fixture (private).
- Foot grid/wall rail marked at known measured positions; measured reference floor origin and independent check points.
- SlimeVR Server version/branch, tracker stable keys, sensor model where known, actual polling/packet delivery rate, battery, RSSI, mounting location/strap/rotation setting, known "raw/reference_adjusted" selection.
- Two OpenXR controller role/physical identity, grip/aim pose availability, room-scale stage support, boundary mode, headset compositor frame rate and known mixed-reality restrictions.
- Optional standalone ESP32 firmware repository/commit, board MAC/sensor mapping, OTA/reboot and device clock unwrap.
- Optional phone ≥240 FPS/reference LED or external motion capture, device time mapping and annotator/reference resolution.

If a field is unmeasured or unknowable, record `unknown`, not an inferred value. This inventory itself must be versioned as a private dataset manifest, with sanitized public metadata.

### 2.3 Proposed physical room layout

**Reference volume:** A **2.5m × 2.5m** unobstructed floor area, camera-visible floor edges, at least **2.2m safe vertical clearance**, subject to actual play area and headset guardian. These are **test-design starting dimensions**, not universal safe clearance for high kicks; perform personal boundary inspection first.

- Camera A: near frontal view at approx 2.0–2.5m from center, 1.1–1.5m high, firmly mounted.
- Camera B: near lateral view at 60–100° relative to A, 2.0–2.5m from center, 1.1–1.5m high, avoiding cross-camera obstructions.
- Camera C optional: ~120° total separation for rear/oblique coverage; never behind a strike line or directly facing mirrors.
- Camera D optional: remaining quadrant to improve rear/occluded limbs, provided USB bandwidth and inference capacity actually permit.
- All cameras must see at least 70% of a fixed reference wand trajectory, including low-to-floor positions, before being included in the 3D evaluation.
- Never move/adjust cameras **within** one calibration block. Move intentionally in failure test, then recalibrate. Capture the exact 6-DoF camera placement only if independently measured; guessed nominal positions don't become `CameraExtrinsicsData`.
- Controller-target calibration held slowly within the safe play zone; do not conduct maximum-power kicks with headset on if the physical guardian/spotter cannot see the feet.

**Minimal 2-camera result** is the first gating milestone. 3/4 cameras are expansions measured independently, not proof by assertion that adding cameras always improves quality.

## 3 — Preflight gates and strictly ordered operator procedure

### 3.1 Software preflight

Before connecting actual sensors:

1. Read `git status`, latest SHA, `Cargo.lock`, `apps/vr/project.godot`, current config and all Phase 01–06 implementation commit links. Do **not** assume those phases exist because their files in `docs/` exist.
2. Build the release engine on target Windows 11, run contracts/golden/renderer smoke tests, confirm diagnostics can be enabled and all API endpoints are local/authenticated according to existing policy.
3. Verify model files and user-supplied ONNX Runtime checksums using `python scripts/setup.py check`; don't invoke an unapproved model download.
4. Create a **gitignored** test data directory with sufficient free space; keep personal video, faces, trainer notes, hardware serials/IDs and credentials out of the public repo.
5. Verify all recorded client/server protocol versions and `SkeletonDefinition.rev` ordering. Run baseline without connected sensors to prove the no-sensor mode doesn't report measured body tracking.
6. Confirm current safety boundary/VR play area for movement; have an observer outside strike area. Stop on dizziness, tracking loss, guardian malfunction or unstable footing.
7. Identify the **authoritative time** for each sensor: actual exposure start/midpoint, source device sample time, receive time, OpenXR predicted display time or render time. If source only supports receive time, mark this clearly.
8. Check screen capture/reference phone video can observe both an external event LED and the application display without exposing personal sessions.
9. Never "fix" missing calibration by temporarily disabling stale/profile identity validation; a measurement with that bypass is invalid for release.
10. Log preflight status and exact blocked prerequisites in `phase07-run-manifest.json` (private path), with a sanitized summary checked into `docs/validation/`.

**Presence gates:** `H0_GO`, `H1_GO`, `H2_GO`, `H3_GO` are **separate**. If H1 isn't met, H3 metric world validation cannot pass; only independent XR transport/desktop checks may still be run.

### 3.2 Physical camera setup and calibration

Use the existing exact `scripts/calibrate_cameras.py` commands (when the engine isn't using the cameras):

```powershell
# Run from the repository root on the actual Windows host.
cargo build --release -p bansayo-engine
./target/release/bansayo-engine.exe --list-cameras
python scripts/calibrate_cameras.py board --out data/private/charuco.png
python scripts/calibrate_cameras.py capture --camera CAM-FRONT=0 --camera CAM-SIDE=1 --out data/private/calib
python scripts/calibrate_cameras.py solve --dir data/private/calib --square-m 0.04 --engine http://127.0.0.1:7700
```

**Important:** Board generation/solver options are the current code's examples; inspect `python scripts/calibrate_cameras.py --help` and its subcommand help before running. The `--square-m 0.04` is only correct **if the actual printed square measures 40mm**. The camera indices are for the **offline calibration capture only**; the live engine must use unique hardware identities. Stop the engine before camera capture, restart for importing camera profiles only as the script requires. Do not hold the physical webcam open in two applications. The solve command may import over the engine API when it is running **after capture**; preserve that separation.

**V-01** five independent intrinsics calibrations/camera with 15–30 diverse board views, held-out board frames never used in solve. Record RMS reprojection per calibration, focal/spread, image resolution, distortion and actual print scale; accept only when current solver thresholds plus held-out geometric checks pass.

**V-02** calibrate room; use rigid 500mm wand at **3×3×3** positions over training volume, at slow/still and motion. Record DLT reconstructed endpoint distance minus physical distance, n, mean absolute, p95, maximum per cell. Require **absolute length error <=1%** at the intended operating volume per existing hardware-lab entry gate, with failure details; 1% of 0.5m is 5mm. If reference target localization cannot achieve this, record failure and consider changing the valid operating volume rather than claiming 3D accuracy anyway.

**V-03** measure floor points at 9 horizontal locations and 3 heights; report residuals in mm and tilt in degrees. For initial **release threshold** use p95 height error <=20mm across intended foot-raise region and max <=40mm, **provisional**, to be tightened/relaxed only with held-out measured evidence and recorded rationale. Misaligned floor invalidates height metrics.

**Hard identity test C-07/C-09:** move a camera intentionally by 20cm or yaw by 1–3°. Current code can reject bad reprojections, **but may not mark extrinsics stale automatically**. Never claim automatic stale detection is implemented until there is an actual code/test. For now the operator must deliberately invalidate/recalibrate camera profile, and the plan requires a persisted manual reason `camera_rig_moved`. The Phase 07 validation may add a safe diagnostic/wizard, but not bypass the need for physical recalibration.

### 3.3 Camera-count matrix and reference order

For **one unchanged physical recording** under a 4-camera rig, derive comparison using source masks wherever raw data supports them. Source masking/re-analysis must use identical hardware exposure timing. Do not compare unrelated athlete motion takes as if only camera count changed.

| Case | Cameras | Measurement | Expected evidence |
|---|---|---|---|
| C1 | A | Raw 2D only | No qualified metric 3D from a single camera |
| C2 | A+B | Initial calibrated volume and athlete test | 3D world reconstruction in overlapped area; V-02/V-03 results |
| C3 | A+B+C | Compare to C2 on same frames | Valid 3D yield, p95 length error, occlusion and latency |
| C4 | A+B+C+D | Compare to C2/C3 on same frames | All four stable IDs, additional view rejection and resource cost |
| C4-minus-1 | 4 then drop one camera each | V-26 | Quality degrades only in relevant joints; no total crash |
| C4-minus-2 | 4 then drop two | V-26 | Two valid cameras still yield measured metrics where visible |
| C4-minus-3 | 4 then drop three | V-26 | Falls back to 2D and/or IMU evidence; **no metric world claims** |
| C4-oblique | 4 source subsets by angle | V-27 | Pose/gait/kick occlusion and geometric sigma by layout |

**At every case** log actual delivered FPS, inference Hz, dropped latest frames, capture interval jitter, used_views per landmark, triangulation acceptance, reprojection p95, processing stage p95 and model ID. Count eligible frames against **sensor capture time**, not render frames.

## 4 — Camera timing and synchronized observation QA

### 4.1 Physical LED reference method (L-01 / L-02 / V-07 / V-08)

Do not use visual Wi-Fi ping or WebSocket RTT as direct camera exposure timestamp evidence.

**Reference setup:** ESP32 with LED visible by all cameras; a separately recorded microcontroller event timestamp plus **two-way** clock exchange to engine; reference 240 FPS phone films the LED and optionally desktop monitor/headset lens.

**Per-camera procedure:**
1. Set explicit manual exposure/gain; record device's **readback** after application.
2. Trigger 100 LED state edges, each separated by pseudorandom interval >=250ms (deterministic seed 20261121) to avoid periodic aliasing; discard insufficiently resolved transitions, report excluded count.
3. For each edge, map microcontroller event clock to engine session clock with uncertainty. Identify first frame whose exposure integrated LED state; account for exposure integration ambiguity and rolling shutter. The first bright frame is an **interval bound**, not necessarily exposure start.
4. Estimate offset/distribution and uncertainty with an exposure interval model. Report p50/p95/p99 observed event→arrival latency, mean/SD and the **reference-clock uncertainty** separately.
5. Compute pairwise camera offset differences from same flashes, not adjacent frame arrival alone; report synchronization residual by camera.
6. Repeat at 30, 60, and >=100FPS **only with actual hardware modes**, and after changing exposure. If UGREEN max is 30FPS, mark 60/100 **BLOCKED_UNSUPPORTED_CAMERA_MODE**, do not synthesize a different rate as real.
7. Apply measured offsets/QA through existing `latency_ms` or Phase 02 calibrated time configuration with proper source quality. Do not relabel receive-time as synchronized due to offset subtraction.
8. Run ten independent cross-camera time sync estimates (V-07). Existing architecture target **mean absolute error <=2ms and SD <=2ms** versus LED-derived offsets. If a receive-time-only camera cannot reach this target, report failure and bound usable movement speed.
9. Compare `gated` vs `aligned` triangulation (V-08) using a measured wand swung at increasing speeds, and **same raw frame sets**. Accept aligned as the default only if >5m/s wand-length error <=50% gated, static not worse and yield not lower, as existing ADR-023; otherwise preserve existing default/mode.
10. For any estimator that settles and later resets, verify identity/uncertainty reset on device clock reboot and no interpolation across reset.

**Never pretend** the hardware LED protocol is implemented if no firmware or host event capture exists. The Phase 07 script must either provide a minimal isolated firmware+host reference **as a new tested module** or declare L-01/L-02/L-05 BLOCKED.

### 4.2 Source-level timing quality matrix

| Source | Current exact time semantics | Valid claim | Invalid claim |
|---|---|---|---|
| Existing UVC/MJPEG camera | Session arrival minus configured latency; true exposure time unknown | Arrival time + explicitly bounded/external-calibrated exposure lag | Device-hardware timestamp accurate to 1ms |
| RTSP camera | Network decode/arrival time, uncertain buffering | Slow visual/2D context; metric only with validated sync profile | Equal to USB capture time without proof |
| Phone/WebSocket camera | Network arrival with variable delay; Phase 01 phone ingestion separate from XR | Measured arrival latency and source profile | Exposure-synchronized without paired source timestamp |
| SlimeVR SolarXR | `receive_time` with configured latency; no sample stamp/gyro in current adapter | Relative orientations with empirically measured lag bound | Raw gyro timestamps or synchrony with camera at sensor-sample instant |
| ESP32 direct UDP | Device microsecond clock (32-bit wrap), SourceTime estimates offset | Estimated sample time after settlement, with observed uncertainty | Perfect absolute time from a board at boot |
| Phase 01 OpenXR HMD/grips | Tracked device pose and client monotonic time; Phase 02 maps clock/stage | Aligned pose at declared runtime time, with clock uncertainty | Guaranteed sensor exposure pose at camera shutter instant |
| Godot streamed pose | Engine `t_us` plus 50ms display buffer and compositor | Rendering delay and frame age measured at client | Same as native XR motion-to-photon |
| 240 FPS reference phone | Video frame interval ≈4.17ms plus clock/exposure/readout error | Finite time interval reference | Exact 4.17ms absolute capture accuracy without clock/exposure calibration |

Retain `TimeQuality` in each SourceEvent. Never use packet arrival timestamp and then label a pose `Measured` at a physical instant without uncertainty. For fast kicks, age×speed matters: if movement is 5m/s, 20ms skew can produce ~0.10m position mismatch. This arithmetic illustrates risk, not observed rig error.

## 5 — OpenXR room/stage alignment physical validation (requires Phases 01–02)

### 5.1 Prerequisite artifact/identity gate

Before XR tests, prove the following **actual implemented code** (names may change from the plans):
- Three independently tracked streams `XR-HMD`, `XR-LEFT-GRIP`, `XR-RIGHT-GRIP` present, valid and recorded; **grip pose**, not Godot UI aim.
- HMD/left/right tracking flags, device descriptor/source IDs and sequence/timestamps stable across 60s, with meaningful disconnect/reconnect.
- Phase 02 `T_xr_stage_from_world` exists and contains valid room, camera set, board reference, stage session generation and controller-tip pivot calibration.
- Four-timestamp clock sync is settled and quality has a measured/estimated uncertainty; **Phase 01 alone is receive-time**, insufficient for measured elbow fusion.
- The physical controller tip/stylus fixture has independently measured `r_grip_from_tip`, not guessed by reaching a board corner with an uncalibrated controller housing.
- Rig/world right-handed +Y up, floor normal up, +X board row, +Z matching canonical body forward.
- Active StageInfo, boundary/guardian and current OpenXR runtime reference-space mode are in the report.
- If any unmet, **BLOCKED_XR_PREREQUISITE**; camera-only or IMU-only experimental blocks can proceed independently.

### 5.2 Measured physical alignment procedure

**Board landmarks:** Define four non-collinear, measured reference points (floor corners; at least one elevated test point afterward) in camera world. Use a rigid, consistently calibrated pointer attached to a controller grip.

1. Hold physical pointer at each marked point **without moving/rotating the board**, for 2 seconds per point, collect ≥60 valid tracked samples across the session.
2. Fit rigid 6-DoF \(T_{xr\_stage \leftarrow world}\) using Phase 02 implementation, with explicit matched camera-world point and XR stage pointer positions.
3. Keep at least **three independent holdout points**, including 1 point above floor, out of the solve set; never evaluate fit-only points as heldout accuracy.
4. Compute per-axis residual and Euclidean 3D error, median/p95/max, board-to-world floor error and yaw rotation error where an independent orientation fixture exists.
5. **Provisional gates** on holdout: median <=0.030m, p95 <=0.050m, max <=0.080m; Stage/world yaw error <=3° if independently referenced. These are targets, **not verified performance**. If no independent orientation reference, report yaw calibration error unverified.
6. Have operator turn 0°, 90°, 180° and walk within guardian while controllers remain tracked; stage-world coordinates must stay fixed, not rotate with HMD heading.
7. Test left controller and right controller independently; if two grip fixtures or tip offsets differ, calibration must not silently share a pivot.
8. Deliberately **recenter** OpenXR reference space. Alignment must become stale/blocked at once until recalibrated; no old, convincingly aligned VR body.
9. Suspend/resume headset, leave/rejoin OpenXR session, disconnect controller, change physical room. Each must trigger appropriate stage/calibration invalidation or clearly limit portability.
10. Record both source timestamps, clock quality, stage generation and transform matrix for each run. No random residual fitting in Godot user code.

**Separate test:** HMD eye centre is *not* anatomical skull centre. Neither alignment nor headset tracking proves head-pose accuracy for full-body metrics.

### 5.3 Additional 6-DoF and reference-frame QA

- Synthetic identity and 90° yaw transform remain unit tests; physical test checks floor up direction by raising reference pointer +0.500m measured on vertical rail and comparing all axes.
- Verify left↔right sign/role labels with real movement one controller at a time and with one controller occluded.
- Controller grip↔aim difference is measured and logged but **never** used as a replacement calibrated wrist offset.
- Moving XR Origin in Godot to satisfy aesthetics does not change persisted physical camera world; BodyRoot applies engine transform once.
- With head motion but otherwise stationary body, headset camera is independent; never translate the avatar root/head because HMD moved.

## 6 — Athlete calibration and arm-fusion physical QA (requires Phases 03–05)

### 6.1 Athlete-specific T-pose, repeatability

- Use pseudonymous athlete ID, entered height independently measured, current camera set and room.
- Three independent **5-second** T-pose captures per athlete; arms level, both sides clear, observed COCO17 shoulder/elbow/wrist. Independent tape/marker measurement of bony landmarks by second operator where practical.
- Compare measured shoulder→elbow and elbow→wrist length per side to reference, raw error (signed), median absolute error and repeated MAD. Target repeated median deviation **<=20mm** (Phase 03 provisional gate).
- Failure: hide right arm but not left; only left measured, right **Assumed**. No mirrored phantom body profile.
- Athlete A↔B switch, entered height edited, room change and reclip reference: per-attribute staleness must follow Phase 03 validity matrix. Calibrations never migrate across athlete IDs by display name.
- Confirm skeleton rev changes before first pose of new proportions. In Godot 19-bone rig must rebuild without one frame of wrong limb length; ghost uses recorded skeleton.

### 6.2 Controller grip→wrist local translation

Use independent camera wrist landmarks; slow calibration motion, no high-force punches until reference metrics pass.

1. Require settled Phase 02 map/time, same current body profile and controller identity.
2. Capture Phase 03 4-second wrist calibration with at least 30 time-corresponding camera+XR samples and >=20° controller rotation span.
3. Compute held-out predicted world wrist using \(p_{world,grip}+R_{world,grip}r_{grip\_from\_wrist}\), compare with **independent qualified camera wrist** at matched t_ref, not the same calibration samples.
4. Target held-out median wrist residual <=0.030m, p95 <=0.050m, **provisional** and only meaningful with camera wrist reference uncertainty stated.
5. Compare left/right separately; change grip/strap, controller identity, room/stage and confirm correct stale/blocked behavior.
6. In a static T-pose, twist the controller in the hand without shifting the forearm; wrist position should not jump by a fixed Euler guess. A failed position residual blocks XR elbow IK.
7. Preserve grip pose orientation as controller pose; do not publish anatomical forearm pronation/twist from a grip whose anatomical rotation calibration does not exist.

### 6.3 Controlled elbow IK and full sensor arbitration

Run each movement under **the same recorded source events**, then source-masked replay where allowed:

| Scenario | Expected engine decision | Reference check |
|---|---|---|
| Camera elbow fully visible + tracked XR wrist + valid arm lengths | Two-link constrained arm may be **Derived** | Elbow 3D reference or qualified independent camera marker |
| Camera elbow blocked for 100ms, wrist valid | Prior bend plane **Inferred**, no 180° jump | Visual/video + elbow continuity |
| Camera elbow blocked >150ms, wrist valid | Anatomical fallback explicitly **Inferred** | No measured elbow score or false known joint |
| Two valid mounted IMUs disagree with XR wrist | Preserve real IMU orientation; report conflict | IMU/ruler/goniometer reference |
| Only upper-arm IMU present | Preserve upper IMU, solve forearm only when geometrically reachable | Forearm angle reference |
| Only forearm IMU present | Preserve forearm orientation; constrained upper only if compatible | Shoulder/forearm reference |
| XR wrist unreachable by >4cm | No IK, reason `arm_target_unreachable` | Stable root, no pose explosion |
| XR stage recentered mid-sequence | Immediate XR constraint invalidation | No world-frame drift/disguised pose |
| Camera root stale beyond allowed 150ms | No measured world-root claim | Explicit inferred/unavailable state |
| One camera remains in 2-camera rig | World metric result suppressed, IMU-only orientation may remain | No invalid zeroed body |
| SlimeVR feed stops 10s | Device disconnected then recovers; camera fallback where valid | No stale frozen segment |
| Full body model/profile/athlete revision changes | Recompute current skeleton and clear state | No wrong-athlete wrist offsets |

**Independent static angle reference:** elbow, knee and hip at approximately 0°, 45°, 90°, 120°, measured with calibrated goniometer/inclinometer, **two human raters** and recorded reference disagreement (V-12). For dynamic kicks/punches without calibrated external reference, report **agreement** only; do not claim centimetre or degree motion-capture accuracy.

### 6.4 Avatar/VR external-figure validation

- Verify 19 world-pose-to-local bones with the Phase 05 exact FK residual threshold <=2mm for matched deterministic fixtures.
- Confirm real physical avatar aligns with floor/target marks (L-04/Phase 02) to measured tolerance, no double transform.
- Verify clinic ghost, live avatar and camera-image mirror display distinct synchronized/replay states, without HMD following body head.
- During tracking dropout, correct provenance materials (measured/derived/inferred/unavailable), no opaque old hand/leg.
- Record headset actual refresh/FPS, compositor dropped/reprojected frames and VR client parse/apply times. Native XR HMD motion-to-photon cannot be inferred from engine frame latency.
- Keep first-person collidable body **off**; the app is a coaching/training visualization, not a contact-proof guardian.

## 7 — SlimeVR and optional ESP32 independent experimental blocks

### 7.1 SlimeVR original protocol / mounting / rate

Run the existing [HYBRID-OWNER-CHECKLIST.md](validation/HYBRID-OWNER-CHECKLIST.md) **S-01…S-06** first. Exact expectations:

- SlimeVR Server reachable at explicitly configured endpoint (typically loopback `ws://127.0.0.1:21110`), actual version recorded. No remote insecure port exposure for this test.
- Stable tracker key→DeviceId mapping across cold restart, tracker order and USB/LAN reconnect. ID collisions rejected.
- For each tracker: actual data **update** Hz, battery, RSSI, `TimeQuality::ReceiveTime`, repeated-value keepalive distinct from new motion samples.
- Proper assignments `pelvis`, `left_thigh`, `right_thigh`, `left_shin`, `right_shin` as available; missing tracker case must not pretend 5 trackers were tested.
- One-second mounting calibration facing canonical +Z. Expected spread threshold `MAX_SPREAD_RAD = 0.035` rad, no movement during reference capture.
- 10-minute drift test at rest, followed by kicking. Heading rate <=15°/s correction by current algorithm; measure actual drift degrees/min and correction convergence.
- Record and compare SolarXR **Raw** vs **ReferenceAdjusted** rotations in **separate sessions** with identical externally referenced poses; never apply SlimeVR mounting twice silently.
- Disconnect SlimeVR Server for 10 seconds. Confirm source statuses, no stale silent orientation, rejoin without restarting Bansayo.
- If \(p95\) input latency cannot be bounded because source lacks a device timestamp, mark **timing accuracy UNVERIFIED** even if animation visually follows.

**SlimeVR is not absolute position tracking in this Bansayo adapter.** Inferred root/foot during camera outages must be labeled as such. Body-pose T-pose and IMU mounting are different calibrations.

### 7.2 Hybrid tests: same motion, comparable inputs

Start with existing H-01…H-07 and expand:
1. Slow knee-flexion: camera-only vs hybrid within **10°** for the checklist's known slow reference, if both methods valid; difference is agreement, not anatomical truth.
2. Fast front kick and roundhouse: cameras 30 FPS + valid IMUs, record 10 repetitions each side at **safe controlled** speed, report IMU method for thigh/shin and measured root percentage.
3. Occlusion: block one calibrated camera 300ms, then 1s; foot and root position claims must disappear/degrade according to quality; orientations may remain tracker-derived.
4. Compare 5%/20%/50% injected packet loss, 200ms/1s network gap on same recorded source events. Verify no stale-source interpolation and recovery behavior (V-28).
5. Heading drift under 10 minutes mixed movement, plus 90-minute sustained run if thermal/safety permit; report degrees/min and fraction of frames with `imu_camera_disagree`.
6. Mounting slip: deliberately **loosen/reposition** one band during a controlled static test, then recapture mounting. Correct result is degraded/stale/invalidated or detected disagreement; **if no automatic slip detector exists, report a known gap**, not a PASS.
7. Compare segmented training events with camera-only vs IMU-only vs hybrid_auto re-analysis on identical recording. Track `tracking_config_id` and `calibration_set_id` changes and per-metric grade availability.
8. Independent high-speed phone camera provides timing/event annotation; any 10° dynamic angle target from H-05 needs an actual reference measurement, not phone video alone unless accurate joint markers/goniometer protocol support it.
9. Show cases where hybrid is **worse** than camera only, including IMU mounting mistakes and high gyro drift; do not report only successful takes.
10. Do not merge results from different athletes/rigs/methods in progress trends without comparability evidence.

### 7.3 Direct ESP32 packet stream (H5 optional)

The current `sources/esp32.rs` 64-byte packet format exposes device time, gyro, accel. When firmware is actually available:
- Verify 100/200Hz real sample output, UDP packet loss, sequence gaps, 32-bit microsecond wrap (~71.6min), reset/reboot and estimator settlement/uncertainty.
- Reference gyro ±2000°/s nominal scale against fixture/inclinometer; test high-g kick saturations and rejected nonfinite packets.
- Compare direct raw accel/gyro events to SolarXR orientation-derived timing with **same physical movement**, but label reference clock uncertainty.
- Reboot one board while other sensors operate; registry must not interpolate across epoch.
- No firmware in current repo: report **BLOCKED_NO_FIRMWARE**, do not invent sample-time performance from SolarXR.
- Do not integrate linear acceleration to absolute body position without a validated drift-correction model; out of scope.

## 8 — Movement, training and measurement study

### 8.1 Exercise matrix with controlled conditions

Use a spotter/second operator and conservative speeds first. Stop on user fatigue, dizzy headset feedback, straps sliding or safety-boundary issues. No combat contact or near-wall strikes.

| Trial | Motion | Primary metrics | Sensors/edge cases |
|---|---|---|---|
| M-00 | Quiet 20s stand, arms at rest, T-pose | Root stability, floor height, drift and clock | A+B, HMD stationary, bands static |
| M-01 | Slow straight arm extension/flexion | Shoulder/elbow/wrist agreement, bend-plane continuity | Camera+XR, left/right, intentional occlusion |
| M-02 | Guard→jab→cross→guard | Punch event timing, wrist path/arm IK | Controllers, fast/slow, side swaps |
| M-03 | Slow front kick 5 each side | Ankle height, knee/hip angle, support root | Camera-only, hybrid, heel visibility |
| M-04 | Faster front kick 10 each side | Peak extension timing, dropout, IMU advantage | 30 vs 60/≥100 FPS where actual |
| M-05 | Roundhouse 10 each side | Hip/shin pose, guard, rotational source confidence | Occlusions, IMU headings, camera layout |
| M-06 | Side kick 10 each side | Peak foot height/line, pelvis orientation, frame skew | Different support foot camera visibility |
| M-07 | Knee strike 10 each side | Hip/knee timing, support foot stability | Camera vs hybrid |
| M-08 | Elbow strike slow+moderate | Elbow bend/plane and source switching | Camera elbow partially occluded |
| M-09 | Kata/short guided combination | Rep counts, sequence/phase errors, event delivery | VR clinic, training events, repeat |
| M-10 | High kick hold 5–10s, then lower | Height uncertainty, angle stability, endpoint drift | Static reference rail, no overextension |
| M-11 | Deliberate bad pose/incorrect side | Wrong-side recognition and rejected metrics | Confidence/fallback labels |
| M-12 | Sensor-loss demonstration (non-striking) | Recovery, no false scores or unsafe VR | Unplug camera, drop SlimeVR, XR recenter |

**Sample sizes and labels:**
- Initial integration check: 1 adult volunteer, 3 repetitions/side and static reference tests; enough to prove pipeline connectivity, **not** model generalization.
- First release claim for rep detection: follow existing V-19: **≥30 repetitions × ≥3 participants per technique**, per relevant capability profile; independent annotations; precision and recall targets **>=0.95**, count error <=1 per 20. If cohort unavailable, **BLOCKED_GENERALIZATION**.
- V-20 event/phase timing against independent ≥240 FPS video with synchronization marker and annotator agreement.
- Calibration repeatability V-33: five sessions over two weeks for selected method/athlete; cannot claim long-term personal improvement/noise threshold before run.
- For martial arts at speed, **do not** declare foot or speed Grade A if the actual camera is 30 FPS and metric requires 60 or 100+ Hz; degrade according to current metric rate gates.

### 8.2 Quality and decision discipline

- Per metric report actual `Provenance`, `MethodId`, `TimeQuality`, `SigmaBasis`, confidence, source age, calibration set, number of usable views and rejection count.
- V-35 compare empirical errors against confidence bins and propagated sigma; `SigmaBasis::Validated` only after relevant independent experiments and versioned validated uncertainty table.
- At least one complete deliberate occlusion/one-camera drop must **suppress** metric grade rather than silently keep last valid value.
- A false "measured" label for camera-originated body during missing camera/root is an automatic NO-GO, even if rendered motion looks smooth.
- A physically inaccurate but high-confidence output should be reported as detector/calibration reliability issue, not averaged away with IMU.
- Record failed repetitions/ambiguous subject switches; do not throw them out from precision/recall or availability denominators.

## 9 — Latency, performance and sustained-load gates

### 9.1 Existing architecture targets, still unmeasured end-to-end

The following are **targets from architecture/15**, not actual achieved values:

| Path / stage | Reference target p95 | Independent measurement required |
|---|---:|---|
| MJPEG 1280×720 decode | <=4ms | Existing diagnostics p95; current one-camera historical **4.3ms** did not meet |
| Pose inference per camera | <=12ms | Separate tracked, redetect and mixed 4-camera load |
| Engine solver | <=2ms | diagnostics Stage::Solve |
| Publish/serialize local stream | <=2ms | diagnostics Stage::Publish |
| Camera exposure→engine frame (gated) | <=55ms | LED/exposure reference + engine timestamps |
| Camera exposure→engine frame (aligned) | <=72ms | Same with aligned camera skew |
| IMU sample→engine frame | <=20ms | **Not provable with SolarXR alone** unless external reference bounds device time |
| Camera exposure→desktop pixels | <=90ms | 240 FPS reference video of event and screen |
| Camera exposure→VR mirror | <=125ms | High-speed lens-view reference + LED |
| Hazard→notice | <=100ms | Independent trigger→publish clock where relevant |
| VR application frame budget 90Hz | <=11.1ms | Headset runtime/profiler, not desktop Godot editor |
| VR application frame budget 72Hz | <=13.9ms | Same |
| GDScript frame parse+apply | <=1ms | Instrument actual client parse/update |
| Engine CPU on four 720p60 + twelve trackers | <=4 cores p95 | Process CPU and thread stats, **if hardware exists** |
| GPU inference on same GPU as VR | <=50% GPU | GPU utilization plus VR compositor frame pacing |

Architecture also includes 120Hz solver cadence for training, 200Hz target IMU, ≥60FPS kick timing and ≥100FPS speed metrics; actual camera modes decide what can be claimed. **Targets must never be copied into the result column without timestamps and method.**

### 9.2 Which clocks are compared

Define **five distinct clocks/intervals** in report:

1. `t_physical`: independently measured LED/fixture event with uncertainty.
2. `t_capture`: true camera exposure centre if measured; otherwise **unknown**, not automatically `SourceEvent.t_us`.
3. `t_session`: engine's session clock used for observations and frame calculations.
4. `t_client_receive`: actual Godot/desktop monotonic receive instant.
5. `t_display`: actual physical display of avatar in 240 FPS phone/lens reference, with compositor uncertainty.

Equations:

```text
camera_to_engine = t_engine_frame_publish - t_physical
engine_to_client = t_client_receive - clock_map(t_engine_publish)
client_to_display = t_display - t_client_receive
physical_to_display = t_display - t_physical

camera_pair_skew_i_j =
  mapped_capture_i - mapped_capture_j     # only when both known/estimated
xr_paired_time_error = t_xr_pose_mapped - t_camera_capture
imu_input_age = t_engine_tick - t_imu_sample_mapped
```

**Do not** sum percentiles of independent stages and call that an end-to-end p95: instead measure actual matched events across stages, then calculate p50/p95/p99 over whole-event distributions. Treat uncertain offsets as intervals/bounds.

**VR lens recording:** high-speed reference filming through headset lens is a measurement technique with occlusion/reflection limitations; publish uncertainty and excluded frames. Native headset tracking prediction is outside Bansayo's camera event lag.

### 9.3 Sustained-load and recovery

Run all four camera modes (if present) + enabled IMUs + headset:
- 10-minute warmup and short trial to find obvious instability.
- 30-minute full exercise/clinic/recording trial if hardware and safe participant rotation allow it.
- **90-minute L-08** endurance run with live or physically reproducible stand-in feeds, source buffers and recording on; note whether source is physically live or simulated.
- Record stage timing windows every 10s, per-camera dropped input counts, requested/delivered rate, inference Hz, browser/VR message drops, CPU/GPU utilization, GPU memory, engine RSS, peak queue depths, recorder writes/sec, disk MB/min and output file size/corruption.
- Provisional reliability gates: no crash; no stale measured pose after >500ms; no unbounded queue/memory; 0 corrupted MCAP recordings; each camera delivers **>=85% negotiated rate** where normal light and confirmed mode; no >2s unexplained I/O stall; no unintended athlete/room mapping.
- Evaluate source load shedding: latest-frame overwrite vs increasing lag. A faster GPU cannot fix missed exposure timestamps or camera USB mode; identify physical bottleneck.
- In source-loss tests, no spurious `recording_failed`, duplicate DeviceId or unbounded reconnection attempts.
- A device 30FPS limitation should be recorded as capability constraint, not a failure of a nonexistent 120FPS hardware mode.

## 10 — Deterministic recording, replay and source masking

### 10.1 Recording is the experimental truth

For each accepted take save:
- `recording.mcap`, session metadata, input SourceEvents, recorded camera-specific raw landmarks / device time / client XR events (when implemented), calibrations/config context, active athlete profile and start/end time.
- `session.json`, anonymized dataset/take manifest, baseline source quality and event timestamps.
- `tracking_config_id` and `calibration_set_id`; compare those IDs for every replay.
- Live derived frame and MotionSample **hashes** if capture protocol currently supports them; otherwise produce offline validated hashes from captured output. Do not confuse output hashes with the raw sensor-event checksums.
- Camera source, model and origin; no mixing `Origin::Simulated` with live results claimed as physical validation.
- If video record is disabled or unavailable, mark footage reference **missing** rather than claiming independent optical accuracy.

### 10.2 Exact replay equality

**Baseline:** same event stream, same source order, same `Context`, same solver tick times and same profile/method version should reproduce all canonical output fields within numerical tolerance and same quality/provenance reasons.

The current `recording/replay.rs::seek` restores and applies events, then computes a pose at a chosen time. **Phase 04 adds stateful bend-plane history**; replay seek must reconstruct intermediate solver tick state to claim determinism. This is a **specific test/fix target**, not an assertion current replay already handles that state.

Test:
- Live frame sequence vs recorded sequential replay.
- Sequential replay to t vs random seek to same t at x1, x0.25, x2 and loop.
- Re-run after changing today's active athlete; recorded body/skeleton must remain that of recorded context.
- Re-run with camera-only/IMU-only/hybrid source masks, **explicit new analysis context/identity**, not pretending those reanalyses equal the original mixed-sensor output.
- Camera unplug/replug and XR clock reset events must remain represented in replay in proper accepted order.
- Clip end/restart/switch mode must reset historical pose-buffer/IK state and suppress old skeleton until appropriate revision.
- Ensure motion-joint null/inferred semantics match original when an optical elbow becomes missing or when IMU network gaps are present.
- Record hashes for full outputs after normalization of floating point (quantize positions/angles at a documented <1e-5m/deg precision for deterministic comparison), and separately exact event ordering/profile IDs.

**No silent assumption** that a new skinned avatar's visual frame count equals engine solver tick count. Replay matches engine state and recording, not desktop draw frames.

### 10.3 Regression promotion

Only verified, privacy-approved test corpus summaries (not raw identifiable face video) enter a GitHub regression manifest. The actual raw MCAP remains in the private data directory unless consent/rights explicitly allow redistribution.

Use current `bansayo-engine promote` / `regress` workflows **only after verifying actual CLI flags**. Existing `crates/bansayo-engine/tests/hybrid_fusion.rs` and Phase 0/2 regressions run every CI. A new real golden test must include:
- Input recording SHA-256 and metadata manifest with explicit no-public-video note.
- Reference calibration/model/algorithm identity.
- Allowed tolerance and why (e.g. measured static reference spread); no widened tolerance simply to pass new code.
- Actual reproducibility on clean machine **without** private data when the fixture is synthetically reconstructed; real private corpus tests run on the operator's machine and report BLOCKED in public CI.

## 11 — Diagnostic additions: minimal, not a new observability platform

Most instrumentation already exists in `diagnostics.rs`, `CameraRuntimeStatus`, `TimeSyncStatus`, `Registry`, `SystemState`, `TrackingQuality` and client stream. Add only missing test-time outputs needed to quantify physical gaps, enabled by a dedicated `diagnostics.hardware_validation` opt-in flag.

**New counters / measured fields (no new network protocol when avoidable):**
- Per-source `accepted`, `rejected`, `dropped_capture`, `dropped_ingest`, `late`, and reconnect count; reuse existing per-camera counters where present.
- `latest_source_age_us`, current `time_quality`, mapped uncertainty and drift/reset count; don't collapse receive-time and estimated.
- Current camera `intrinsics_profile_id`, `extrinsics_profile_id`, confirmed identity, active room and source timestamp class; status summary only, no raw proprietary serial in public report.
- For camera multi-view: accepted frame-set count, used views per landmark, p95 skew, p95 reprojection, room/rig fingerprint.
- For XR alignment: stage session/generation, source/room fingerprint, profile valid/stale state, holdout reference error only in experiment report.
- For Phase 03/04: per-side measured-vs-assumed segment lengths, wrist offset profile valid, 3D wrist/IK residual and source selection reason; no false "accuracy" field.
- For SlimeVR: receive-time source age, actual rate, battery/RSSI if provided, mounting status, heading residual/correction state.
- For Godot: parse/apply time, most recent frame arrival age, buffer depth, skeleton revision applied, actual application frame time, graphics/backend mode.

Use existing typed `StageTiming` and status structures if compatible. If a new diagnostics payload field is added, regenerate TypeScript contract fixtures, maintain optional backward compatibility, and never change frame protocol version just for validation-only counters.

**Logging policy:** normal production behavior logs per episode, status transitions and summary windows, **not** raw body pose at INFO each frame. Test detail saved to bounded CSV/JSONL in private directory with explicit retention. Avoid leaking addresses, subject names and headset serials.

## 12 — Prescriptive validation artifacts and scripts

### 12.1 New files (small, single-purpose)

| Path | Operation | Responsibility |
|---|---|---|
| `docs/validation/phase07/HARDWARE-MATRIX.md` | New | Canonical H0…H5 tiers, C/S/H/V/L crosswalk, operator prerequisites and status |
| `docs/validation/phase07/TEST-PROCEDURES.md` | New | Exact LED/wand/floor/IMU/XR/VR movement procedures and stop rules |
| `docs/validation/phase07/RELEASE-GATES.md` | New | Thresholds, GO/LIMITED/NO-GO/BLOCKED decision logic, supported claims |
| `docs/validation/phase07/report.template.md` | New | Per-run machine/hardware/consent/source-status/sampling/uncertainty table |
| `benchmarks/hardware/rig.example.yaml` | New | Sanitized hardware inventory/test block definitions, no IDs or secrets |
| `benchmarks/hardware/thresholds.yaml` | New | Typed test-only numeric thresholds; architecture reference targets vs release gates separate |
| `benchmarks/hardware/manifest_schema.json` | New | Machine-checkable JSON schema of per-run manifest |
| `benchmarks/hardware/validate_manifest.py` | New | Validate required identities, units, exposure flags, timestamps, references, labels and status |
| `benchmarks/hardware/score_geometry.py` | New | Pure wand/floor/XR/wrist error, percentiles and independent-reference validation |
| `benchmarks/hardware/score_timing.py` | New | Pair event IDs/time uncertainty, compute camera skew, LED latency bounds and full-path measured p95 |
| `benchmarks/hardware/score_tracking.py` | New | Measured/derived/inferred/unavailable yield, source reliability, IMU conflicts and drop reasons |
| `benchmarks/hardware/score_replay.py` | New | Compare sequential/seek/live output summaries and normalized hashes |
| `benchmarks/hardware/report.py` | New | Generate sanitized Markdown/JSON outcomes and precise BLOCKED reasons |
| `benchmarks/hardware/tests/test_geometry.py` | New | Synthetic rigid transform/wand 3D/floor sanity and error polarity tests |
| `benchmarks/hardware/tests/test_timing.py` | New | Synthetic clock offsets, skew, latency, rollover and uncertainty tests |
| `benchmarks/hardware/tests/test_replay.py` | New | Synthetic deterministic event sequence and correct inability to compare different contexts |
| `benchmarks/hardware/tests/test_release_gate.py` | New | Test no GO without measured external reference and active required tiers |
| `scripts/hardware-validation.ps1` | New | Windows wrapper to capture toolchain, run preflight and invoke existing engine/test commands |
| `crates/bansayo-engine/tests/phase07_validation.rs` | New | Pure source/status/time/multicamera/recording invariants; CI without hardware |
| `apps/vr/tests/phase07_telemetry_smoke.gd` | New | Headless source-quality display, no XR headset dependency |
| `crates/bansayo-engine/src/diagnostics.rs` | Edit only if required | Minimal optional timing/counters with typed bounded memory |
| `crates/bansayo-engine/src/config/schema.rs` | Edit only if required | `diagnostics.hardware_validation` opt-in, default false |
| `crates/bansayo-engine/src/config/validate.rs` | Edit only if required | Strict valid report/file path/rate ranges, no loose fields |
| `crates/bansayo-engine/src/pipeline/capability.rs` | Edit only if required | Include actual profile/version where newly needed, never test-only fake sources |
| `crates/bansayo-engine/src/recording/replay.rs` | Edit **only if test proves bug** | Fix seek vs sequential state-history discrepancy with deterministic tick reconstruction |
| `apps/vr/scripts/status_panel.gd` | Edit only if necessary | Current XR/camera/IMU readiness and blocked reason, no new VR controls layer |
| `docs/13-implementation-status.md` | Edit after actual implementation | Real status and report links; no unrun hardware accuracy claims |
| `docs/validation/README.md` | Edit | Index Phase07 matrix/templates and completed/blocked reports |
| `.github/workflows/ci.yml` | Edit | Run pure tests; no hardware false green/red and no private model/data download |

**Do not create** a second streaming server, new database, Python production backend, camera SDK abstraction, unlicensed model loader, redundant body solver, graphics renderer or extra sensor-fusion layer for validation.

### 12.2 Private per-run data schema

`benchmarks/hardware/rig.example.yaml` should use a **test-only YAML manifest**, parsed offline. Keep actual serials/paths out of repository:

```yaml
schema: bansayo.hardware_validation/v1
run_id: phase07-2026-11-21-room-a
engine_commit: REPLACE_WITH_REAL_40_CHAR_SHA
mode: hardware
tier: H1
room_id: room-a
references:
  board_measured_square_m: 0.040
  wand_measured_length_m: 0.500
  wand_length_uncertainty_m: 0.001
  clock_reference: led_two_way_exchange
  external_3d_reference: wand
cameras:
  - device_id: CAM-FRONT
    identity_confirmed: true
    format: mjpeg
    requested_fps: 30
    measured_fps: null
    exposure_s: 0.00390625
    timestamp_class: receive_time
  - device_id: CAM-SIDE
    identity_confirmed: true
    format: mjpeg
    requested_fps: 30
    measured_fps: null
    exposure_s: 0.00390625
    timestamp_class: receive_time
slimevr:
  enabled: false
xr:
  enabled: false
tests:
  - id: V-02
    status: blocked
    reason: reference_data_not_recorded
    reference_file: null
  - id: L-02
    status: blocked
    reason: led_firmware_missing
    reference_file: null
```

This is **an example with placeholders**, not a genuine experiment. The checker must reject it as a completed hardware pass. All measured fields must include units, source/revision, sample count and reference uncertainty. Do not hardcode sample measurements into source.

### 12.3 Offline report schema

The per-experiment report stores:

```json
{
  "schema_version": 1,
  "run_id": "sample-synthetic-only",
  "mode": "synthetic",
  "engine_commit": "<real-commit>",
  "tracking_config_id": "<real-id>",
  "calibration_set_id": "<real-id>",
  "hardware_tier": "H0",
  "test_id": "V-02",
  "status": "blocked",
  "reason": "reference_not_available",
  "measurement": null,
  "reference": {
    "method": "none",
    "uncertainty": null
  },
  "evidence_files": []
}
```

**Mandatory `mode` enum:** `synthetic`, `physical`, `replay_of_physical`, `offline_reference`. Synthetic must be displayed as such in all summaries. File paths in public report must be relative sanitized manifests or digests only.

### 12.4 YAML thresholds with explicit levels

Use separate categories, **not one magical accuracy number**:

```yaml
schema: bansayo.hardware_validation_thresholds/v1
preflight:
  requires_real_camera_identity: true
  requires_independent_reference: true
geometry:
  wand_max_relative_error: 0.01
  floor_p95_error_m: 0.020     # provisional Phase 07 operating-volume gate
  floor_max_error_m: 0.040
  xr_holdout_median_error_m: 0.030
  xr_holdout_p95_error_m: 0.050
  xr_holdout_max_error_m: 0.080
  grip_wrist_holdout_p95_error_m: 0.050
timing:
  camera_time_offset_mean_abs_ms: 2.0
  camera_time_offset_sd_ms: 2.0
  camera_to_engine_p95_ms: 55.0  # gated path; adjust label for aligned
  camera_to_vr_p95_ms: 125.0
  vr_max_frame_ms_at_90hz: 11.1
quality:
  camera_min_delivered_ratio: 0.85
  no_measured_pose_on_one_camera: true
  no_stale_measured_pose_after_ms: 500
  require_real_sensor_sources: true
reliability:
  corrupted_recordings_allowed: 0
  accidental_cross_athlete_profiles_allowed: 0
training:
  rep_precision_min: 0.95
  rep_recall_min: 0.95
  rep_count_abs_error_per_20_max: 1
```

Mark which thresholds come from **existing architecture** (e.g. V-07, V-19) and which are **new provisional release limits** (e.g. XR holdout), and include the report/provenance for any change. Production calibration thresholds continue to live in engine YAML; benchmark thresholds do not override them at runtime.

## 13 — Clear release decision matrix

### 13.1 Hardware tier outcomes

| Decision | Requirements | What may be claimed |
|---|---|---|
| **GO H1 CAMERA_3D** | Two physical calibrated cameras + independent wand/floor accuracy, known camera time limits, correct quality suppression | Metric 3D within tested volume/pose/lighting; no headset/SlimeVR claim |
| **GO H2 HYBRID** | GO H1 + stable physical SlimeVR mounting, measured drift/rate/occlusion/recovery, known SolarXR timing limits | Camera+IMU pose augmentation for tested outputs, with explicit measurement/receive-time limitations |
| **GO H3 PCVR** | GO H1/H2 as applicable + real Phase 01–05 code, valid XR world/stage+clock, controller/wrist heldout residual, headset frame/latency and correct fallback | Correctly aligned external avatar/VR coaching for tested hardware and movements |
| **GO H4 MARTIAL_ARTS_ACCURACY** | GO H3 + independent dynamic reference and V-12/13/16/19/20/30 measured across athletes and repeated conditions | Specific technique/metric accuracy ranges only, with report IDs and exact sensor requirements |
| **LIMITED** | Working physical device path but incomplete independent measurements, throughput or operating-range limits | Explicit *demonstration/basic tracking* only; suppress accuracy and unavailable metric claims |
| **NO-GO** | Physical evidence violates safety/accuracy/replay/profile/identity gate | Stop release of affected configuration; preserve older validated tiers |
| **BLOCKED** | Required camera/headset/sensors, model rights, consent or reference hardware absent | No test or accuracy assertion; synthetic regression allowed as preparation only |

No “Phase07 complete = everything works” field. Each sensor/metric capability has its own validated operating envelope. In particular a 30 FPS camera configuration is not promoted to high-grade kick speed because it completed a VR demo.

### 13.2 Automatic hard failures (always NO-GO for affected claim)

- Cross-athlete calibration/profile reused without explicit valid identity.
- XR stage moved/recentered but old world alignment still reported valid and shown convincingly aligned.
- World/metre coordinate inversion, left/right swap, nonfinite output or camera model relative data falsely used as metric world.
- Single-camera world position marked measured with no qualified second calibrated view.
- Old IMU or XR data used across disconnect/reset/clock epoch without freshness and correct provenance.
- Inferred elbow/foot/heading marked measured; unknown sigma marked validated.
- Silent dropped sensor messages producing fake confidence or incorrect rep counts without quality suppression.
- Crash, corrupted recording or loss of reproducibility in a safety/assessment-critical sequence.
- Unsafe headset UI that obstructs guardian or encourages high-speed strikes beyond a verified physical play boundary.

### 13.3 Reporting example of correct limited claim

```text
Configuration: one UGREEN 1280x720 at 30 FPS.
Validated: camera identity/controls/rate and 2D body pose on this PC.
Not validated: metric 3D, high-speed kick accuracy, OpenXR body alignment,
controller anatomy, SlimeVR timing/precision, 2-4 camera throughput.
Status: LIMITED / H0 only.
```

This is consistent with the current real-camera evidence and does not imply nonexistent physical XR validation.

## 14 — Exact deterministic test suite and physical test IDs

### 14.1 CI / synthetic invariant tests

- [ ] `hardware_manifest_rejects_synthetic_mode_with_physical_go`.
- [ ] `hardware_manifest_rejects_missing_reference_uncertainty_and_unit`.
- [ ] `hardware_manifest_rejects_unverified_camera_identity_for_metric_3d`.
- [ ] `hardware_manifest_rejects_duplicate_device_ids_and_ambiguous_usb_selector`.
- [ ] `hardware_manifest_report_never_contains_private_serial_or_video_path`.
- [ ] `hardware_time_wrap_and_reset_starts_new_epoch`.
- [ ] `hardware_time_receive_time_cannot_claim_sensor_sync`.
- [ ] `hardware_time_led_edges_out_of_order_detected_not_sorted_silently`.
- [ ] `hardware_time_estimated_offset_preserves_reference_uncertainty`.
- [ ] `hardware_time_camera_pair_skew_is_signed_and_symmetric`.
- [ ] `hardware_time_end_to_end_percentiles_from_matched_events_not_sum_of_p95`.
- [ ] `hardware_geometry_identity_and_90deg_yaw_stage_world_transform`.
- [ ] `hardware_geometry_measured_wand_0p5m_known_endpoints`.
- [ ] `hardware_geometry_length_error_increases_with_biased_camera`.
- [ ] `hardware_geometry_floor_axis_y_positive_and_heel_height`.
- [ ] `hardware_geometry_reprojection_only_does_not_generate_ground_truth_error`.
- [ ] `hardware_geometry_one_camera_3d_has_no_qualified_metric_result`.
- [ ] `hardware_source_mask_two_of_four_keeps_world_when_valid`.
- [ ] `hardware_source_mask_one_of_four_suppresses_world_and_foot_metrics`.
- [ ] `hardware_source_occlusion_does_not_sustain_measured_pose_after_gap`.
- [ ] `hardware_slimevr_receiver_time_gyro_missing_is_distinct_from_esp32`.
- [ ] `hardware_slimevr_raw_reference_adjusted_not_double_mounting`.
- [ ] `hardware_identity_camera_move_invalidates_or_blocks_3d`.
- [ ] `hardware_identity_controller_change_invalidates_wrist_offset`.
- [ ] `hardware_identity_athlete_switch_blocks_foreign_body_geometry`.
- [ ] `hardware_xr_recenter_disables_world_aligned_avatar`.
- [ ] `hardware_ik_unreachable_preserves_imu_and_reports_reason`.
- [ ] `hardware_ik_camera_elbow_missing_marks_fallback_inferred`.
- [ ] `hardware_recording_replay_sequential_equals_seek_with_stateful_ik`.
- [ ] `hardware_recording_new_session_calibration_does_not_leak_into_old_replay`.
- [ ] `hardware_quality_missing_external_reference_is_blocked_not_green`.
- [ ] `hardware_quality_unmeasured_grade_a_metric_is_suppressed`.
- [ ] `hardware_no_go_on_stale_marked_measured_even_if_latency_passes`.
- [ ] `hardware_existing_camera_only_and_imu_only_goldens_unchanged`.

### 14.2 Physical operator tests

- [ ] **C-01…C-09** cameras: identity/mode, rate, exposure, USB replug, duplicate units, moved camera.
- [ ] **V-01** repeat intrinsic calibration, **V-02** wand volume, **V-03** floor grid.
- [ ] **L-01** exposure latency, **L-02** cross-camera skew, **V-07** independent LED offset.
- [ ] **V-08** gated/aligned at static and >5m/s wand speed.
- [ ] **S-01…S-06** SlimeVR protocol/mounting/heading/reconnect.
- [ ] **V-12** independently measured static angles, **V-13** floor-relative limb heights.
- [ ] **H-01…H-07** hybrid slow/fast/front/roundhouse, occlusion and repeatable re-analysis.
- [ ] **V-15/V-16/V-20** peak speed and event/phase timing with reference.
- [ ] **V-17/V-18** IMU drift/heading, camera correction and stress.
- [ ] **V-26/V-27/V-28** camera subset/occlusion and IMU dropout.
- [ ] **V-29/V-30/V-31** lighting, fastest safe kicks/punches and clothing/person ambiguity.
- [ ] **V-19** 3 participants×30 reps per technique where claimed.
- [ ] **V-32/L-08** long run, frame rate, dropped events and memory.
- [ ] **V-33/V-34/V-35** repeatability, sensor comparability and confidence-to-error calibration.
- [ ] **L-03/L-04/L-07** physical desktop/lens display latency and VR frame timing.
- [ ] **XR-01…XR-08** actual device/role/clock, stage transform holdout, controller wrist, recenter and headset independence.
- [ ] **A-01…A-06** T-pose, athlete isolation, arm IK, unavailable/inferred and Godot rev.
- [ ] **R-01…R-08** live recording, source masking, stage reset, partial loss, sequential/seek replay, old MCAP.
- [ ] **E-01/E-02**, if firmware present: ESP32 direct sample timing/reboot; else BLOCKED_NO_FIRMWARE.

### 14.3 Verified/expected command family

**Run actual checked-in commands** after inspecting `--help` / test fixture files:

```powershell
# From repo root, engine/contract baseline:
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p bansayo-engine --test camera_source
cargo test -p bansayo-engine --test phase2_camera
cargo test -p bansayo-engine --test hybrid_fusion
cargo test -p bansayo-engine --test slimevr_source
cargo test -p bansayo-engine --test replay
cargo test -p bansayo-engine --test recording
cargo test -p bansayo-engine --test phase07_validation

# Existing real-engine and API test suites:
node scripts/verify-gate1.mjs
node scripts/verify-gate2.mjs
python scripts/calibrate_cameras.py selftest

# VR headless (does NOT validate an attached headset):
godot --headless --path apps/vr --editor --quit
godot --headless --path apps/vr --xr-mode off --script tests/contract_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/spatial_ui_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/clinic_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/phase07_telemetry_smoke.gd

# New offline score/unit checks:
python -m unittest discover -s benchmarks/hardware/tests
python benchmarks/hardware/validate_manifest.py --manifest benchmarks/hardware/rig.example.yaml --check-shape-only

# Actual physical testing: operator must use a private, real manifest and explicit command;
# NEVER claim the example YAML carries measurements.
powershell -ExecutionPolicy Bypass -File scripts/hardware-validation.ps1 -Manifest data/private/phase07/rig.yaml -Mode Physical
```

**Command caveats:** A new file/test path does not exist until implemented; missing command is **BLOCKED_NOT_IMPLEMENTED**. `clinic_smoke.gd` may require an isolated running engine; verify and start it with the documented simulation config before treating a headless failure as product regression. `cargo test` cannot prove real hardware, nor may a private camera/video reference be downloaded by public CI.

## 15 — Seven direct-main implementation packages, ordered dependencies

These are **implementation work packages**, not already completed commits. Each new commit must re-check latest main first, avoid overwriting newer work, preserve previous code and run relevant tests.

### Commit 01 — Establish true hardware readiness and ownership

- [ ] Fetch latest main and identify actual implemented **Phase 01–06** code SHA per prerequisite; mark missing ones BLOCKED.
- [ ] Audit existing physical evidence and open device list, reference fixtures and actual OS/GPU/camera hardware.
- [ ] Add `HARDWARE-MATRIX.md`, `TEST-PROCEDURES.md`, `RELEASE-GATES.md` and sanitized `report.template.md`.
- [ ] Write explicit BLOCKED fields for missing XR/SlimeVR/second camera/LED/high-speed reference; no fictional measurements.
- [ ] Preserve existing V/L/C/S/H owner checklists and link rather than replace them.
- **Commit message:** `docs: define Phase 07 hardware readiness tiers and release gates`.
- **Gate:** exact hardware availability and source-code status known before proceeding.

### Commit 02 — Add pure offline benchmark schemas and reference scorers

- [ ] Add versioned rig/threshold YAML and strict manifest JSON schema, private path handling.
- [ ] Build independent geometry/timing/tracking scorers with deterministic reference fixtures.
- [ ] Enforce matched-event p95, sample counts and reference uncertainty; missing evidence must block GO.
- [ ] Add Python stdlib unittest cases, with optional YAML dependency pinned only where used by offline scripts.
- **Commit message:** `bench: create reproducible geometry and latency reference scoring harness`.
- **Gate:** synthetic tests pass and NEVER produce a “physical measurement” status.

### Commit 03 — Instrument bounded diagnostic gaps only

- [ ] Use current diagnostics counters/status rather than a new stack.
- [ ] Add a test-only diagnostic opt-in for per-source age/drop, XR Stage/clock and profile validity where runtime lacks a field.
- [ ] Keep protobuf/JSON protocol v1 backward compatible (regenerate contracts on actual changes), no INFO per-frame spam.
- [ ] Add `phase07_validation.rs` and reuse existing Node engine-test harness.
- **Commit message:** `engine: expose bounded hardware validation quality and timing diagnostics`.
- **Gate:** default-off behavior and existing golden recordings identical.

### Commit 04 — Automated test of the real-source invariants

- [ ] Implement camera identity, 2-camera loss, stale-profile, SlimeVR network, direct ESP32 epoch and optional XR constraints in deterministic integration tests.
- [ ] Validate exact room/athlete/stage reset, no left-right swap and no single-camera world metric claim.
- [ ] Fix observed engine bugs **only with failing regression first**, commit minimal per responsibility.
- [ ] In replay verify Phase 04 bend continuity state reconstructed under seek; fix if test shows discrepancy.
- **Commit message:** `test: cover real-source identity loss calibration and deterministic replay`.
- **Gate:** tests pass, no release claim that camera or headset was physically used.

### Commit 05 — Physical camera rig and time reference evaluation

- [ ] With actual second/third/fourth camera, run C/V/L camera blocks in order, external wand/floor/LED reference.
- [ ] Record measured geometry, time offsets, FPS/dropouts and reference uncertainty, versus provisional gates.
- [ ] Run real movement slow first, contrast 30/60/100 modes **only where supported**, and record false failures/blocked tests.
- [ ] Put only sanitized reports/manifests/digests in GitHub; retain private sessions/videos locally.
- **Commit message:** `validation: document physical camera rig accuracy and synchronization`.
- **Gate:** GO H1 only with independent reference; otherwise LIMITED or BLOCKED.

### Commit 06 — Physical SlimeVR, OpenXR and martial arts trials

- [ ] Only after required Phase 01–05 app code and successful H1 geometry, run H2/H3 tests, stage/wrist/athlete calibrations, SlimeVR mounted tracking, 240 FPS reference, 1s occlusions and safe strikes.
- [ ] Measure p50/p95 latency and actual rates; source time classes and held-out residuals; preserve camera/IMU-only fallback.
- [ ] Test headset recenter, controller grip changes, IMU strap slip, device reconnect, two-client stream and ghost/clinic.
- [ ] Populate actual completed V/H/L/XR/A/R report IDs or explicit BLOCKED when headset/tracker/reference missing.
- **Commit message:** `validation: report physical XR SlimeVR and martial arts tracking results`.
- **Gate:** no full-PCVR result without physical headset, stage and wrist calibration; safety failures are NO-GO.

### Commit 07 — Replay, sustained load, CI and final decision

- [ ] Run V-32/L-08 and 10/30/90-minute tests according to hardware, timing and privacy restrictions.
- [ ] Complete real-source replay/seek and source-mask comparisons with exact hashes/quality metadata.
- [ ] Link validated V/L reports and actual hardware contexts to release decision; update `docs/13-implementation-status.md` and `docs/validation/README.md`.
- [ ] CI runs only offline fixtures, with hardware tests marked clearly manual. Record actual tool versions and commands run.
- [ ] Review latest main, open TODOs and all failed/blocked cases; no silent broad "passed".
- **Commit message:** `docs,ci: publish Phase 07 scoped release decision and reproducibility evidence`.
- **Gate:** GO/LIMITED/NO-GO/BLOCKED supported by measured, independently referenced results with bounded uncertainty.

**Do not create empty commits** merely to reach a count of seven. If physical hardware isn't available, finish the safe infrastructure/docs and **stop** with specific dependency blockers, not a fabricated Phase07 completion.

## 16 — Mandatory exact completion report for the implementing agent

Provide **all** fields:

1. Source baseline SHA and latest `main` SHA before and after; actual implemented commits for Phases 01–06, not their planning document SHAs.
2. Machine/OS/GPU/driver/Godot/OpenXR runtime, delivered vs requested camera modes and SlimeVR Server version, with real/pseudonymous identifiers appropriately redacted.
3. Hardware tier and complete physical inventory, reference instruments, measured uncertainty, room/board/camera XR calibration profile IDs.
4. C/S/H/V/L/XR/A/R/E test matrix **PASS / FAIL / BLOCKED / SKIP**, sample size, real/synthetic status and evidence file links/digests.
5. For 2/3/4 cameras: actual FPS, inference Hz, p95 camera arrival intervals/jitter, used_view fraction, DLT reprojection, wand length and floor height **independent** errors.
6. Timing: LED reference quality, camera offset/skew/uncertainty, mapped XR stage clock, SolarXR receive-time limitation, optional ESP32 sample clock, p50/p95/p99 physical→desktop/VR.
7. Spatial: XR Stage↔camera world holdout points, transform, controller side/pivot, wrist held-out 3D residual per arm, athlete lengths, correct invalidations.
8. Body: static goniometer agreement, dynamic reference where available, 30/60/100 FPS real sample rates, heading drift, IMU contribution and blocked/suppressed rates.
9. Workout: per technique/side/person rep counts, precision, recall, phase/strike timing and independent annotation uncertainty.
10. Replay: input SHA, live sequential/seek comparison, exact calibration set and tracking identity, cross-athlete/room reset tests.
11. Reliability: 10/30/90-minute session resource histograms, crash count, dropped-frame count, disk/MCAP corruption, pose buffer recovery and headset frame budget.
12. All scripts/tests actually run with PASS/FAIL/BLOCKED; do not mark tool absent or unrun as PASS.
13. Safety verification and headset no-first-person-body/guardian behavior; any unsafe behavior explicitly NO-GO.
14. A **scoped** GO/LIMITED/NO-GO/BLOCKED recommendation with exact supported configuration and metrics; no marketing summary.
15. Any remaining work as `TODO` with owner, exact path, dependency/blocker, acceptance test and whether it blocks public release.

## 17 — Definition of done and strict separation of evidence

**The Phase 07 documentation/infrastructure milestone** can complete when the operator procedure, typed private manifest, independent score tools, synthetic tests, traceability matrices and honest hardware prerequisite statuses are committed and CI-verified.

**The Phase 07 full physical validation milestone** can complete only when the actual 2–4 camera rig, SlimeVR, HMD/controllers and independent references needed for the intended product claim have been used; the measured spatial/temporal error, reliability and workout quality are reported with physical source integrity and privacy; and the relevant release gates pass.

Until then:
- Existing one-camera proof **does not** become four-camera proof.
- A simulated kick **does not** become an actual athlete accuracy trial.
- An XR session initialized without calibrated stage/world timestamp data **does not** become 6-DoF body validation.
- A SlimeVR band transmitting orientations without device timestamps **does not** establish sub-millisecond motion timing.
- A rig with no independent dynamic reference can report **agreement, repeatability and error bounds**, not proven absolute high-speed joint accuracy.
- A schema/model with unreviewed weights rights (Phase 06) remains disabled regardless of appearance.

**Outcome:** A reproducible, testable, critical decision about what Bansayo measures **in the real world** under defined hardware, lighting and movements. The expected output is **evidence**, not simply another functioning demo.
