# Architecture decision records — training platform

ADR-001 – ADR-017 are in [docs/12](../../../12-decisions.md) and remain in force. This folder
continues the numbering.

A record is **Accepted** only when nothing in it waits for an experiment. **Pending Validation**
records state their decision criteria; **Proposed** records describe work not yet scheduled.

| ADR | Decision | Status |
|---|---|---|
| [018](ADR-018-training-crate-and-runtime-boundaries.md) | Training logic in a pure crate inside the one engine process; no Go, no services | Accepted |
| [019](ADR-019-canonical-movement-representation.md) | Body state with evidence; closed observable catalogue | Accepted |
| [020](ADR-020-camera-processing-stack.md) | Native capture threads, MJPEG, no OpenCV in the engine | Pending Validation (hardware) |
| [021](ADR-021-pose-estimation-integration.md) | Top-down RTMPose on ONNX Runtime; feet-capable schema recommended | Pending Validation |
| [022](ADR-022-sensor-ingestion.md) | Every input is a source adapter; no plugin loader | Accepted (structure) |
| [023](ADR-023-time-aligned-triangulation.md) | Interpolate free-running cameras to a common instant | Pending Validation (V-08) |
| [024](ADR-024-fiducial-markers-and-board-calibration.md) | AprilTag clusters on rigid carriers; ChArUco script kept for calibration for now | Pending Validation |
| [025](ADR-025-sensor-fusion-ladder.md) | Closed-form arbitration first; filters only on recorded evidence | Accepted (policy) |
| [026](ADR-026-recognition-engine.md) | Declarative phase machines, then templates, then learning | Accepted (stage 1) |
| [027](ADR-027-exercise-engine.md) | One pure exercise engine; every mode is a plan | Accepted |
| [028](ADR-028-scoring-and-coaching.md) | Component scores against stated targets; limited cue scheduler | Accepted (structure) |
| [029](ADR-029-session-and-analytics-storage.md) | SQLite for analytics, MCAP for streams, JSON for profiles | Accepted |
| [030](ADR-030-configuration-and-content.md) | Versioned YAML over closed catalogues; no expression language | Accepted |
| [031](ADR-031-local-first-execution.md) | Local-first; optional token-protected LAN mode | Accepted; LAN mode Proposed |
| [032](ADR-032-desktop-integration.md) | New workspaces in the existing shell; the UI computes nothing | Accepted |
| [033](ADR-033-vr-runtime.md) | Godot 4 + OpenXR on PC, presentation only; WebXR rejected | Pending Validation (headset) |
| [034](ADR-034-capability-profiles-and-comparability.md) | Observability decides features; comparability classes protect history | Accepted |
| [035](ADR-035-communications.md) | Bounded channels inside; additive JSON topics outside | Accepted |
| [036](ADR-036-measurement-status-and-immutable-provenance.md) | One status vocabulary; immutable results with provenance | Accepted |
| [037](ADR-037-technique-authoring-and-review.md) | Definitions authored from annotated examples, reviewed by a second person | Accepted (data model) |
| [038](ADR-038-golden-sessions.md) | Golden sessions as the regression gate | Accepted |
| [039](ADR-039-adaptive-training-rules-and-language-models.md) | Adaptive training by closed rules; language models only explain | Proposed / Experimental |
| [040](ADR-040-instrumented-equipment-through-sources.md) | Equipment through the source contract; nothing built ahead of a device | Proposed |
| [041](ADR-041-coaching-exchange-by-bundles.md) | Sharing between people is file bundles; no hosted service | Proposed |

## The fourteen mandatory technology decisions

| Decision in the brief | Record |
|---|---|
| 1 Core tracking runtime architecture | ADR-001, ADR-011, [018](ADR-018-training-crate-and-runtime-boundaries.md) |
| 2 Camera-processing stack | [020](ADR-020-camera-processing-stack.md) |
| 3 Pose-estimation integration | ADR-009, [021](ADR-021-pose-estimation-integration.md) |
| 4 Sensor ingestion | ADR-008, [022](ADR-022-sensor-ingestion.md) |
| 5 Sensor fusion | [023](ADR-023-time-aligned-triangulation.md), [025](ADR-025-sensor-fusion-ladder.md) |
| 6 Shared movement representation | ADR-002, [019](ADR-019-canonical-movement-representation.md) |
| 7 Recognition engine | [026](ADR-026-recognition-engine.md) |
| 8 Exercise execution engine | [027](ADR-027-exercise-engine.md) |
| 9 Coaching engine | [028](ADR-028-scoring-and-coaching.md), [039](ADR-039-adaptive-training-rules-and-language-models.md) |
| 10 Session storage | ADR-005, [029](ADR-029-session-and-analytics-storage.md) |
| 11 Desktop application integration | ADR-006, ADR-007, [032](ADR-032-desktop-integration.md) |
| 12 VR runtime and rendering | ADR-010, [033](ADR-033-vr-runtime.md) |
| 13 Configuration and content management | ADR-012, [030](ADR-030-configuration-and-content.md), [037](ADR-037-technique-authoring-and-review.md) |
| 14 Local-first versus networked execution | [031](ADR-031-local-first-execution.md), [041](ADR-041-coaching-exchange-by-bundles.md) |
