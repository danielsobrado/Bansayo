# 13 — Implementation and validation record

Resumed on **16 November 2026** from the interrupted Stage 1 work. Changes remain uncommitted;
the repository owner retains the commit decision specified in [10](10-poc-plan.md).

## Camera and VR vertical slice (17 November 2026)

Implements the camera-to-VR PoC plan: physical camera capture, 2D pose inference, camera
calibration, time alignment, a camera-only skeleton and VR training feedback. Built to accept
cameras by configuration alone; **no physical camera or headset has been connected yet.**

| Milestone | Delivered | Evidence |
|---|---|---|
| M0 Godot | Imports, parses and renders with Godot 4.5.1; desktop mode passes `--xr-mode off` so OpenXR is not started | Contract smoke; desktop render of a live simulated front kick (`--write-movie`). The Virtual Desktop OpenXR runtime was found, no headset connected |
| M1 Capture | `kind: camera` source: Media Foundation devices (nokhwa, MJPEG decoded in-process) or image folders; one capture thread per camera with a latest-frame slot; reopen every 2 s after failure; declared resolution enforced; JPEG preview endpoint; `--list-cameras` | `tests/camera_source.rs`; an image-folder camera tracked at 12.8/15 Hz in a debug build while an absent device reported `disconnected` and a mis-sized folder reported the `resolution_px` to set |
| M2 Pose | YOLOX person detection + RTMPose-m (COCO-17, no heel/toe) on ONNX Runtime 1.24 loaded at run time, DirectML on Windows; subject tracking by box overlap, detection every tenth frame; One-Euro smoothing | Matches an onnxruntime/OpenCV reference within 0.01 px on a COCO photo; 3.7 ms per tracked frame in release |
| M3 Calibration | `POST /calibration/profiles` import with validation and atomic persistence; ChArUco script (capture, intrinsics, floor-board extrinsics, quality gates); registry keeps 8 frames per camera and triangulation interpolates free-running cameras to one instant | Import test (rejections, replacement rule, restart); calibration selftest recovers three synthetic cameras within 1% of range and 0.16°; interpolation cuts the worst kick landmark error from 11.3 mm to 0.6 mm with cameras 16 ms out of phase |
| M4 Skeleton | `pipeline/skeleton_fit`: pelvis/chest frames, minimal-swing limbs with inherited twist, confidence gating; used when no trackers are configured; unseen segments stay inferred | Knee/hip angles equal the scripted squat/kick/standing angles to 1e-5 rad from simulated landmarks; Studio shows the camera-only skeleton (`sim-cameras-skeleton.yaml`) |
| M5 VR | Joint readouts with peaks, squat/kick counts, ankle/pelvis heights, camera and feature status, record/replay buttons, controller-based room alignment | Smoke-tested counters; record → replay → live round trip through the Godot client against a live engine; desktop render |

Recordings from image-folder cameras replay with their COCO-17 landmarks; recordings of the
camera-only skeleton replay with a measured root and changing angles.

Not verified: physical cameras (enumeration, MJPEG negotiation, unplug/replug), real
calibration and measurement accuracy, headset rendering, controller bindings, room alignment,
and sensor-to-headset latency. Camera timestamps are arrival times minus an optional configured
`latency_ms`; per-camera latency has not been measured. Rep counts use fixed thresholds that were
not tuned on real movement. See the root README, "Attach physical cameras".

## Implemented scope

- Shared Rust contracts, generated TypeScript and golden JSON examples.
- Configurable Rust engine with a deterministic simulator, trackers and camera projections,
  mounting capture/persistence, forward kinematics, joint measurements and feature reasons.
- Multi-camera triangulation: camera intrinsics/extrinsics as calibration profiles (declared by
  the simulator), a native triangulation solver stage with undistortion, confidence weighting,
  outlier-view rejection and a configurable sync/error/ray-angle policy (`triangulation:`),
  `landmarks_3d` stream topic with per-landmark used/rejected cameras, measured root position when
  trackers and a triangulated pelvis exist, and the Cameras workspace 3D view with frustums and a
  landmark inspector. Covered by engine tests on simulated geometry
  (`crates/bansayo-engine/tests/triangulation.rs`). Details in
  [05](05-calibration-and-fusion.md) §7.
- Versioned HTTP/WebSocket API, origin checks, bounded queues, graceful shutdown and stream rate
  limits.
- Runtime assignments, stale calibration, manual/scripted faults, MCAP input/context recording,
  session indexing and deterministic replay through the solver.
- React/three.js Studio, Overview, Devices, Cameras and Sessions; simulation, calibration,
  recording and replay controls. Geometry stays outside React state.
- Native Tauri shell that attaches to a compatible local engine or hosts the engine library.
- Godot training-room project, stream consumer and presentation buffer; **not runtime-validated**.

The authoritative requirements remain [10](10-poc-plan.md). This record describes the delivered
scope and evidence rather than redefining those requirements.

## Gate evidence

| Gate | Evidence | Status |
|---|---|---|
| 0 | Contract round-trips, identifier validation and generated-file comparison | Passed |
| 1 | [Real-process check](../scripts/verify-gate1.mjs): hello first, inferred inputs before calibration, capture, live squat values, ordered/rate-limited stream, origin/body validation, persistent profiles, full/six/camera-only/empty configurations | Passed: eight check groups. Passed again on Windows 11 on 16 November 2026 |
| 1 UI | Chrome reference-pose flow captured twelve trackers; Studio coverage changed to 12/19 with derived/inferred styles. Playwright verified a changing knee angle | Passed |
| 2 | [Real-process check](../scripts/verify-gate2.mjs): assignment persistence/stale return, disconnect/reconnect, ten-second MCAP, replay conflicts, paused seek, two equal replay samples, mode restoration and Ctrl-C finalization | Passed: five check groups. Passed on Windows 11 on 16 November 2026 |
| 2 UI | Isolated [Playwright journey](../apps/desktop/tests/e2e/motion.spec.ts): calibration, motion, shin fault and recovery, recording/replay and all workspaces at 1440/1024/800 px | Passed; passed again on Windows 11 on 16 November 2026 |
| Triangulation | [Engine tests](../crates/bansayo-engine/tests/triangulation.rs) on simulated ground truth: noiseless 2–4 cameras, 1.5 px noise, outlier views, distortion, skew/staleness, near-parallel rays, declared extrinsics, measured root, stream topics, replay. Bounds and measured errors in [05](05-calibration-and-fusion.md) §7 | Passed on Windows 11 on 17 November 2026 (worst noiseless error 0.00026 mm; 4 cameras at 1.5 px: mean 3.66 mm) |
| 3 | On Windows 11, 16 November 2026: `cargo fmt --check`, `cargo clippy -D warnings` and `cargo test` for the workspace and the native shell crate; both gate scripts; desktop unit tests, build and the Playwright journey. The debug native shell was started, hosted the engine (`/health` and `/state` answered on port 7700 with the four-camera configuration) and exited when its window was closed. The Cameras 3D view and landmark inspector were inspected in a browser against that configuration | Passed. Not done: a packaged installer build, and looking at the UI inside the native window |

The gate scripts launch real engine processes with temporary configurations/data and clean them
up. A parent process cannot deliver Ctrl-C to a child on Windows, so the harness starts the engine
with the opt-in `--shutdown-on-stdin-close` flag and, on Windows, closes the engine's stdin to
request the same graceful shutdown; it honours `CARGO_TARGET_DIR` when locating the binary. The
gate scripts do not exercise triangulation. The browser journey uses ports 17700/15173 and temporary data, leaving a developer's normal
port-7700 engine untouched. It uses the already-installed Chrome browser; it does not download
a browser.

## Findings corrected during review

- Engine modules and the desktop entry point were incomplete after the interrupted session.
- The browser WebSocket abstraction did not type-check; malformed geometry could reach rendering.
- The domain description contradicted the pre-calibration acceptance criterion. Fresh
  uncalibrated orientations now produce an inferred rest skeleton; missing/stale orientation
  inputs or camera-only input produce a null pose.
- Paused replay seek rebuilt state without publishing a new pose. It now publishes a preview.
- Replayed timestamps can move backward on seek or mode change. Client trace history resets
  while frame sequence filtering remains monotonic.
- Joint-table controls needed actual button semantics for keyboard/native accessibility.
- The native shell's compatibility probe needed the protocol field from `/state`, not `/health`.
- Bundled native configuration must receive the application data directory before validation,
  so a read-only launch directory cannot fail the default data-directory probe.
- Recording context must preserve devices that were already disconnected at recording start.

## Findings corrected on Windows 11

- Tokio's timer on Windows wakes only about every 15.6 ms. The simulator, the solver tick and the
  stream rate limiter ran below their configured rates (100 Hz trackers delivered 64 Hz and were
  reported degraded). They now take their ticks from a plain thread
  ([ticker](../crates/bansayo-engine/src/ticker.rs)), and the simulator schedules each sample from
  the previous deadline.
- The native shell treated a timed-out probe of port 7700 as an error. Windows reports a closed
  loopback port that way, so the shell never hosted the engine. It now treats it as no engine.
- The native shell had no `icons/icon.ico`, which the Windows build requires.
- The Playwright engine launcher did not match CRLF line endings, so its origin allow-list was
  never written. The gate harness ran a stale binary when `CARGO_TARGET_DIR` was set.
- The golden examples still reported triangulation as not implemented. They now carry the
  engine's own reason codes, and a `landmarks_3d` stream example exists.

## Simulation and remaining validation

All trackers and camera observations are simulated. Camera landmarks are projected image-space
points, not physical camera capture or pose inference. Heading alignment and body proportions
are assumed; root placement is inferred by floor contact unless a triangulated pelvis exists, in
which case it is measured from simulated cameras with simulated pixel noise. Camera models are
declared by the simulator, not calibrated. Solver agreement with the simulator, triangulation
included, is a mathematical regression check, not a measurement-accuracy result. Joint definitions retain
`validated: false`.

**Godot validation is deliberately unverified.** Godot is absent and the owner explicitly chose
“Leave Godot validation unverified.” No download or installation was performed. The project and
contract smoke script received static review, but import, parsing, development-mode rendering
and OpenXR execution have not been run.

Also unverified: a packaged Windows installer, physical trackers/cameras/headset, OpenXR runtime paths,
long-running stability, many-client load, sensor-to-screen latency and real joint-angle accuracy.
See [11](11-risks-and-validation.md) for the hardware and release validation backlog.

Real device adapters, camera capture and pose inference, a camera calibration solver, calibration
of real cameras and time synchronisation between them, camera/IMU fusion beyond the measured root
(including IMU heading correction from cameras), a FreeMoCap importer or comparison mode (left out
on licence grounds, ADR-014), calibrated heading/XR alignment, avatars, exports and Analysis remain
outside this PoC.
