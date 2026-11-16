# 13 — Implementation and validation record

Resumed on **16 November 2026** from the interrupted Stage 1 work. Changes remain uncommitted;
the repository owner retains the commit decision specified in [10](10-poc-plan.md).

## Implemented scope

- Shared Rust contracts, generated TypeScript and golden JSON examples.
- Configurable Rust engine with a deterministic simulator, trackers and camera projections,
  mounting capture/persistence, forward kinematics, joint measurements and feature reasons.
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
| 1 | [Real-process check](../scripts/verify-gate1.mjs): hello first, inferred inputs before calibration, capture, live squat values, ordered/rate-limited stream, origin/body validation, persistent profiles, full/six/camera-only/empty configurations | Passed: eight check groups |
| 1 UI | Chrome reference-pose flow captured twelve trackers; Studio coverage changed to 12/19 with derived/inferred styles. Playwright verified a changing knee angle | Passed |
| 2 | [Real-process check](../scripts/verify-gate2.mjs): assignment persistence/stale return, disconnect/reconnect, ten-second MCAP, replay conflicts, paused seek, two equal replay samples, mode restoration and Ctrl-C finalization | Passed: five check groups; final rerun pending |
| 2 UI | Isolated [Playwright journey](../apps/desktop/tests/e2e/motion.spec.ts): calibration, motion, shin fault and recovery, recording/replay and all workspaces at 1440/1024/800 px | Passed |
| 3 | Final Rust, native-shell and application checks | In progress |

The gate scripts launch real engine processes with temporary configurations/data and clean them
up. The browser journey uses ports 17700/15173 and temporary data, leaving a developer's normal
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

## Simulation and remaining validation

All trackers and camera observations are simulated. Camera landmarks are projected image-space
points, not physical camera capture or pose inference. Heading alignment and body proportions
are assumed; floor-contact root placement is inferred. Solver agreement with the simulator is a
mathematical regression check, not a measurement-accuracy result. Joint definitions retain
`validated: false`.

**Godot validation is deliberately unverified.** Godot is absent and the owner explicitly chose
“Leave Godot validation unverified.” No download or installation was performed. The project and
contract smoke script received static review, but import, parsing, development-mode rendering
and OpenXR execution have not been run.

Also unverified: Windows 11 builds/runs, physical trackers/cameras/headset, OpenXR runtime paths,
long-running stability, many-client load, sensor-to-screen latency and real joint-angle accuracy.
See [11](11-risks-and-validation.md) for the hardware and release validation backlog.

Real device adapters, camera capture/inference, triangulation, camera/IMU fusion, measured root
position, calibrated heading/XR alignment, avatars, exports and Analysis remain outside this PoC.
