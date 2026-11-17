# Component reference cards

One card per architectural component with the fifteen items the brief requires. Cards are indexes:
each row states the decision in a line and points to the section that specifies it. Where a card
and its source document differ, the source document is right and the card is the defect.

Status words follow [01](01-product-vision.md). "Status" rows reflect the tree at commit `0be8e3e`
as recorded in [08 §1.2](08-tracking-and-sensor-fusion.md) and
[docs/13](../../13-implementation-status.md).

Component → brief responsibility: Source adapters (Capture, Device Management) · Session clock
(Time Synchronization) · Calibration · Camera pipeline (Pose Estimation) · Marker tracking ·
Solver (Sensor Fusion) · Capability report · Observables (Motion Analysis) · Recognition ·
Exercise engine (Exercise Execution, Workout Planning) · Metrics and scoring (Technique
Evaluation) · Coaching · Content store · Training runner (Session Management) · Recorders ·
Analytics store (Progress Analytics) · Local API · Desktop · VR.

## 1. Source adapters and device registry

| Item | Content |
|---|---|
| Purpose | Turn each physical or simulated input into validated `SourceEvent`s; keep the list of devices, their state and assignments |
| Responsibilities | Connect, read, timestamp, announce devices, report stream health, reconnect; hold the latest observation per (device, kind) |
| Not responsible for | Solving, calibration mathematics, anything about training |
| Inputs | Cameras, IMU feeds, tag detections, XR poses from a client, files (replay, image folders) |
| Outputs | `SourceEvent` (device announced, observation, diagnostics); `SystemState.devices` |
| Data model | `DeviceDescriptor`, `DeviceStatus`, `SensorObservation`, `ObservationData` ([docs/04](../../04-contracts.md), [08 §2.2](08-tracking-and-sensor-fusion.md)) |
| Interfaces | `Source::run(SourceContext)`; bounded ingest channel; `PUT /devices/{id}/assignment` |
| Dependencies | Session clock; validation; supervision |
| Configuration | `sources[]`, `trackables[]`, `assignments`, `quality` ([13 §4](13-configuration-schemas.md)) |
| Failure handling | Device state machine; reopen and backoff; drop newest and count on overflow ([08 §10](08-tracking-and-sensor-fusion.md)) |
| Performance | One task or OS-thread chain per source; never blocks the solver ([15 §2.1](15-performance-and-latency.md)) |
| Extension points | A new `kind` = one adapter and one match arm ([ADR-022](adr/ADR-022-sensor-ingestion.md)) |
| Validation | Adapter tests with recorded or synthetic input; fault injection; hardware bring-up |
| ADRs | 008, [020](adr/ADR-020-camera-processing-stack.md), [022](adr/ADR-022-sensor-ingestion.md), [040](adr/ADR-040-instrumented-equipment-through-sources.md) |
| Status | Simulated, replay, camera (Media Foundation and image folder): Implemented, cameras untested on hardware. SolarXR, ESP32, client ingest: Planned / Proposed. Registry per (device, kind): Proposed (T-01) |

## 2. Session clock and time synchronisation

| Item | Content |
|---|---|
| Purpose | One monotonic microsecond time base for every observation, event and stored row |
| Responsibilities | Map device clocks to session time; label `time_quality`; hold per-camera offsets |
| Not responsible for | Wall-clock time (labels only); client display timing |
| Inputs | Device timestamps, arrival times, `time_sync` profiles, configured `latency_ms` |
| Outputs | `t_us` on every observation; offset estimates and their stability |
| Data model | `TimeUs`, `TimeQuality`, `time_sync` profile (`offset_us`) |
| Interfaces | `clock.rs` offset estimator; applied inside each adapter |
| Dependencies | None |
| Configuration | `latency_ms` per source; `triangulation.max_skew_ms`, `align_exact_ms` |
| Failure handling | Unknown offset → `receive_time`, wider timing sigma, lower grades |
| Performance | Negligible |
| Extension points | Two-way exchange for custom firmware; hardware trigger (`synchronized`) |
| Validation | L-01, L-02, V-07 ([15 §4.2](15-performance-and-latency.md), [16 §3](16-accuracy-and-validation.md)) |
| ADRs | 002, 017, [023](adr/ADR-023-time-aligned-triangulation.md) |
| Status | Clock: Implemented. Offset estimator: Partial (unused, T-05). `time_sync`: Proposed |

## 3. Calibration store and procedures

| Item | Content |
|---|---|
| Purpose | Hold every calibration as a profile with state and dependencies; produce profiles through guided procedures or import |
| Responsibilities | Persistence, validation, invalidation, athlete scoping, calibration sets and QA |
| Not responsible for | Applying calibration (solver stages read profiles) |
| Inputs | Captured observations during a procedure; imported profiles; user-entered measurements |
| Outputs | `CalibrationProfile`s; `ProcedureState`; `calibration_set_id`; QA result |
| Data model | Kinds: `mounting`, `camera_intrinsics`, `camera_extrinsics`, `floor_plane`, `time_sync`, `body_proportions`, `heading_alignment`, `xr_alignment`, `imu_bias`, `marker_geometry`, `marker_mounting` ([09 §8](09-optical-markers-and-calibration.md)) |
| Interfaces | `POST /calibration/profiles` (import), `/calibration/procedures`, `/calibration/check` ([14 §5.5](14-runtime-and-api-contracts.md)) |
| Dependencies | Sources (for capture); `nalgebra` |
| Configuration | `calibration.*`, `markers.*` |
| Failure handling | States `valid` / `stale` / `invalid` / `assumed`; rejected captures with guidance; atomic writes |
| Performance | Solves on blocking threads |
| Extension points | New kind = enum value + invalidation rows; any external tool may import |
| Validation | V-01 – V-03, V-09, V-11; synthetic self-tests |
| ADRs | 015, [024](adr/ADR-024-fiducial-markers-and-board-calibration.md) |
| Status | Store, mounting capture, camera profile import and persistence: Implemented. ChArUco script: Implemented outside the engine, synthetic self-test only. In-engine procedures, QA, sets: Proposed |

## 4. Camera pipeline (pose estimation)

| Item | Content |
|---|---|
| Purpose | From camera frames to 2D landmarks of the one tracked athlete, and tag corners |
| Responsibilities | Capture, decode, subject selection, inference, schema labelling, preview |
| Not responsible for | 3D, smoothing of recorded landmarks, any body model |
| Inputs | Camera frames; model files |
| Outputs | `landmarks_2d` and `fiducials` observations; JPEG preview |
| Data model | `PoseLandmarks2D` with `LandmarkSchema`; `FiducialDetections` |
| Interfaces | `kind: camera` source; `GET /cameras/{id}/preview` |
| Dependencies | `nokhwa`, `image`, ONNX Runtime (`ort`) |
| Configuration | `sources[]` of kind camera; `processing_profiles`; `fiducials` |
| Failure handling | Missing model or provider → `unavailable` / `degraded`; `multiple_people` notice; one-slot mailbox drops stale frames |
| Performance | Budgets in [15 §2.1](15-performance-and-latency.md) |
| Extension points | New detector schema = one schema-map table; lifter as a labelled extension |
| Validation | Reference comparison of inference (done on one image per docs/13); V-30, V-31 |
| ADRs | 009, [020](adr/ADR-020-camera-processing-stack.md), [021](adr/ADR-021-pose-estimation-integration.md) |
| Status | Implemented with COCO-17 and One-Euro smoothing; no physical camera yet. Exposure control, unsmoothed path, Halpe-26, fiducials: Proposed |

## 5. Marker tracking

| Item | Content |
|---|---|
| Purpose | Pose of rigid tag clusters, as segment rotation evidence and IMU heading reference |
| Responsibilities | Associate detections with clusters, triangulate corners, rigid fit, single-view fallback |
| Not responsible for | Joint positions (never overridden); detection itself (camera pipeline) |
| Inputs | `fiducials` observations; camera profiles; `marker_geometry`, `marker_mounting` |
| Outputs | Cluster poses with evidence (`markers` topic); segment rotation candidates for arbitration |
| Data model | `TagDetection`, carrier model, cluster device ([09 §3](09-optical-markers-and-calibration.md), [21 §5](21-wearable-hardware.md)) |
| Interfaces | Solver stage 4 |
| Dependencies | Camera calibration; AprilTag detector |
| Configuration | `trackables[]`, `fiducials`, `markers` |
| Failure handling | Occlusion and blur → cluster `disconnected` (normal during strikes); slip → mounting `stale`; duplicates rejected |
| Performance | Per-tick fit of a few corners |
| Extension points | New carrier model = one YAML file |
| Validation | V-04 – V-06, V-09, V-10 |
| ADRs | [024](adr/ADR-024-fiducial-markers-and-board-calibration.md) |
| Status | Proposed |

## 6. Solver (sensor fusion)

| Item | Content |
|---|---|
| Purpose | One canonical body state with evidence from whatever was observed |
| Responsibilities | Orientation from IMUs, time alignment, triangulation, gates, skeleton fit, heading correction, arbitration, root placement, anchored FK |
| Not responsible for | Prediction, smoothing for display, anything about techniques |
| Inputs | Observation snapshot; calibration profiles; skeleton |
| Outputs | `SkeletonPose` / `BodyState`, `PlanarBody`, triangulated landmarks |
| Data model | [04 §4](04-movement-domain-model.md) |
| Interfaces | Ordered pure functions in `pipeline/solve.rs`; `MotionSample` to the training runner |
| Dependencies | Calibration, skeleton, `math` |
| Configuration | `triangulation`, `gates`, `fusion`, `evidence` |
| Failure handling | A stage without input is skipped; rejected points are `null`; lost segments return to rest pose labelled inferred (ADR-013) |
| Performance | ≤ 2 ms per tick target |
| Extension points | Stage trait on the second implementation of a stage; the ladder of [ADR-025](adr/ADR-025-sensor-fusion-ladder.md) |
| Validation | Synthetic geometry tests (exist for triangulation); V-08, V-12 – V-18, V-26 – V-28 |
| ADRs | 011, 013, 014, 016, 017, [023](adr/ADR-023-time-aligned-triangulation.md), [025](adr/ADR-025-sensor-fusion-ladder.md) |
| Status | IMU orientation, triangulation, interpolation, camera-only skeleton fit, root placement: Implemented on simulated data. Gates, sigma, heading correction, arbitration, evidence: Proposed |

## 7. Capability report

| Item | Content |
|---|---|
| Purpose | State what is observable now, by which method, and why not otherwise |
| Responsibilities | Per-input and per-observable availability; derived profile label; configuration and calibration hashes |
| Not responsible for | Enabling or disabling features (consumers read it) |
| Inputs | Registry state, calibration states, solver outcomes |
| Outputs | `CapabilityReport` in `SystemState` and `GET /capability` |
| Data model | [08 §9.1](08-tracking-and-sensor-fusion.md) |
| Interfaces | Published on state change |
| Dependencies | `pipeline/features.rs` rule table |
| Configuration | `tracking.profile_override` (may only lower) |
| Failure handling | Unknown → `unavailable` with a reason, never optimistic |
| Performance | Recomputed on change only |
| Extension points | New profile rule row; new reason codes |
| Validation | Table tests per configuration; golden example |
| ADRs | [034](adr/ADR-034-capability-profiles-and-comparability.md) |
| Status | Feature table: Implemented. Report, profiles, hashes: Proposed |

## 8. Observables (motion analysis)

| Item | Content |
|---|---|
| Purpose | Named scalar time series with evidence, independent of sensors |
| Responsibilities | Angles, positions, rates, contact, proxies; suppression of ill-conditioned values; filtering and differentiation rules |
| Not responsible for | Detecting techniques; deciding grades |
| Inputs | `MotionSample` |
| Outputs | `ObservableValue[]` per tick |
| Data model | Catalogue `bansayo.observables.v1` ([04 §6](04-movement-domain-model.md), additions in [23 §3](23-biomechanics-model.md)) |
| Interfaces | Pure functions in `bansayo-training::observables`; `observables` stream topic |
| Dependencies | Contracts, `nalgebra` |
| Configuration | `analysis.filters`, `analysis.planar_camera` |
| Failure handling | Unavailable with reason codes; never a guessed value |
| Performance | Tens of scalar computations per tick |
| Extension points | One function + catalogue entry + test; revision bump |
| Validation | Unit, rigid-invariance and mirror property tests; V-12 – V-16 |
| ADRs | [019](adr/ADR-019-canonical-movement-representation.md) |
| Status | Five joint measurements: Implemented. Catalogue: Proposed |

## 9. Recognition

| Item | Content |
|---|---|
| Purpose | Repetitions, phases, step tokens, stances; and honest unknowns |
| Responsibilities | Activity segmentation, phase machines, arbiter, template check, form alignment |
| Not responsible for | Counting towards targets, scoring, any device knowledge |
| Inputs | Observable frames; recognition context from the exercise engine |
| Outputs | `MovementEvent`, `Repetition` with phases and confidence |
| Data model | [04 §7](04-movement-domain-model.md), [07 §1–5](07-recognition-and-coaching.md) |
| Interfaces | Stepped by the training runner |
| Dependencies | Compiled definitions |
| Configuration | `recognition.*`; per-definition blocks |
| Failure handling | Unknown conditions never true; `not_detectable` per capability; `rejected_quality` on gaps |
| Performance | Machines × a few comparisons per tick; DTW once per segment |
| Extension points | New definitions (data); stage 3 behind the arbiter interface |
| Validation | V-19 – V-22, V-25 |
| ADRs | [026](adr/ADR-026-recognition-engine.md), [037](adr/ADR-037-technique-authoring-and-review.md) |
| Status | Proposed. (A fixed-threshold counter exists in the VR client as PoC debt.) |

## 10. Exercise engine

| Item | Content |
|---|---|
| Purpose | Execute plans: lifecycle, sequences, sets, timers, limits, goals |
| Responsibilities, non-responsibilities, inputs, outputs | [06 §2](06-exercise-and-workout-engine.md) |
| Data model | `TrainingState`, `TrainingEvent`, plans ([14 §7](14-runtime-and-api-contracts.md)) |
| Interfaces | `ExerciseEngine::step`; commands |
| Dependencies | Contracts; compiled content |
| Configuration | `training.*` |
| Failure handling | Invalid command rejected; tracking loss sub-state and auto-pause |
| Performance | No allocation on ticks without events |
| Extension points | New plan kind = new compiled-plan variant (for example `reaction`, [27](27-reaction-and-decision-drills.md)) |
| Validation | Table and property tests; golden event logs |
| ADRs | [027](adr/ADR-027-exercise-engine.md) |
| Status | Proposed |

## 11. Metrics and scoring (technique evaluation)

| Item | Content |
|---|---|
| Purpose | One graded measurement per metric per repetition, hold or set; component scores against stated targets |
| Responsibilities | Metric kinds, zero-phase re-filtering, grade rule, comparability class, targets, composite with coverage |
| Not responsible for | Choosing what to say (coaching); trends (progress) |
| Inputs | Repetition windows of observables; references; baselines |
| Outputs | `MetricMeasurement`, `ScoreResult` |
| Data model | [05 §8–10](05-technique-library.md), [07 §7](07-recognition-and-coaching.md) |
| Interfaces | Pure functions called by the runner |
| Dependencies | Observables; content |
| Configuration | Metric packs and scoring models (content) |
| Failure handling | Grade X with reasons; `null` components; composite withheld below coverage |
| Performance | Once per repetition |
| Extension points | New metric kind or anchor (code); new metrics and models (data) |
| Validation | Property tests; V-12 – V-16, V-35 |
| ADRs | [028](adr/ADR-028-scoring-and-coaching.md), [036](adr/ADR-036-measurement-status-and-immutable-provenance.md) |
| Status | Proposed |

## 12. Coaching

| Item | Content |
|---|---|
| Purpose | At most a few, evidence-backed cues at the right moments; cross-session proposals |
| Responsibilities | Candidate evaluation, evidence gate, prioritisation, channel limits, fading; recommendation rules |
| Not responsible for | Measuring; wording (i18n keys, clients format) |
| Inputs | Repetition results, recent history, athlete level; stored history for recommendations |
| Outputs | `CueInstance`; `Recommendation` |
| Data model | [07 §8](07-recognition-and-coaching.md), [24 §3](24-adaptive-training.md) |
| Interfaces | Runner (cues); `GET /progress/recommendations` |
| Dependencies | Scoring; analytics store |
| Configuration | `coaching.*`; cue definitions (content) |
| Failure handling | No evidence → no cue |
| Performance | Once per repetition and per set |
| Extension points | New cue condition form or rule kind (code); cues (data) |
| Validation | Scheduler table tests; V-24 |
| ADRs | [028](adr/ADR-028-scoring-and-coaching.md), [039](adr/ADR-039-adaptive-training-rules-and-language-models.md) |
| Status | Proposed |

## 13. Content store

| Item | Content |
|---|---|
| Purpose | Load, validate, compile, index and serve definitions; write user content |
| Responsibilities | Directory scan, schema and cross-reference checks, snapshots, atomic writes, evaluate and suggest for authoring |
| Not responsible for | Executing content |
| Inputs | `content/`, user content directory, `PUT /content/...` |
| Outputs | Content index, compiled definitions, snapshots |
| Data model | [13](13-configuration-schemas.md); example sets and review blocks ([20 §4](20-technique-authoring.md)) |
| Interfaces | `/content/*` routes |
| Dependencies | `bansayo-training::content` |
| Configuration | `content.dirs` |
| Failure handling | Invalid file reported and skipped; id conflicts refused |
| Performance | Scan at start and after writes |
| Extension points | New kind of file = schema + validator + compiler |
| Validation | The example files as first corpus; fuzzed malformed input |
| ADRs | 012, [030](adr/ADR-030-configuration-and-content.md), [037](adr/ADR-037-technique-authoring-and-review.md) |
| Status | Proposed |

## 14. Training runner (session management)

| Item | Content |
|---|---|
| Purpose | Host the training library inside the engine; own the one training session |
| Responsibilities | Build `MotionSample`s into the library, route commands, publish state and events, feed store and recorder |
| Not responsible for | Any training logic itself |
| Inputs | `MotionSample`, `CapabilityReport`, commands |
| Outputs | `training_state`, `training_event`, `observables`; rows; motion recording |
| Data model | [14 §3, §6–7](14-runtime-and-api-contracts.md) |
| Interfaces | Actor with bounded channels |
| Dependencies | `bansayo-training`; content store; analytics writer |
| Configuration | `training.*` |
| Failure handling | Gap flag on overflow; session `interrupted` after a crash; never blocks tracking |
| Performance | ≤ 1 ms per tick target |
| Extension points | Re-analysis and the regression runner reuse it unpaced |
| Validation | Replay determinism |
| ADRs | [018](adr/ADR-018-training-crate-and-runtime-boundaries.md), [035](adr/ADR-035-communications.md) |
| Status | Proposed |

## 15. Recorders and replay

| Item | Content |
|---|---|
| Purpose | Keep inputs for re-solving, solved motion for review, optional video |
| Responsibilities | MCAP writing, indexed reads, engine replay mode, re-analysis jobs, golden runner |
| Not responsible for | Analytics rows |
| Inputs | Validated source events and context; frames; training events |
| Outputs | `recording.mcap`, `motion.mcap`, `video.mcap`; clips and traces |
| Data model | [12 §7](12-data-and-progress.md) |
| Interfaces | Recording routes (existing); `/training/reps/.../{trace,motion}`; `regress` subcommand |
| Dependencies | `mcap` |
| Configuration | `storage.record_defaults`, retention |
| Failure handling | Drops counted; damaged artefact reported; disk full stops recording, not the session |
| Performance | Writer threads; re-analysis ≥ 10 × real time target |
| Extension points | New channels in `motion.mcap` |
| Validation | Existing replay tests; golden sessions |
| ADRs | 005, 016, [029](adr/ADR-029-session-and-analytics-storage.md), [038](adr/ADR-038-golden-sessions.md) |
| Status | Input recording and replay: Implemented. Motion, video, indexed reads, re-analysis, runner: Proposed |

## 16. Analytics store and progress

| Item | Content |
|---|---|
| Purpose | Durable sessions, repetitions, measurements and their provenance; progress queries |
| Responsibilities | Schema, migrations, single writer, series per comparability class, baselines, records, trends, retention, export, deletion |
| Not responsible for | Streams (MCAP); interpretation beyond the stated classification rule |
| Inputs | Training events; queries |
| Outputs | `/progress/*`, `/training/sessions/*` results |
| Data model | [12 §3–6](12-data-and-progress.md), [32 §3](32-measurement-provenance.md) |
| Interfaces | Writer channel; query functions |
| Dependencies | `rusqlite`; `bansayo-training::progress` |
| Configuration | `progress.*`, `storage.retention` |
| Failure handling | Never blocks tracking; spill and recover; migration failure leaves the file untouched |
| Performance | Row durable ≤ 1 s target |
| Extension points | Migrations; new derived queries |
| Validation | Golden queries; deletion property test; invariants I1 – I9 |
| ADRs | [029](adr/ADR-029-session-and-analytics-storage.md), [034](adr/ADR-034-capability-profiles-and-comparability.md), [036](adr/ADR-036-measurement-status-and-immutable-provenance.md) |
| Status | Proposed |

## 17. Local API

| Item | Content |
|---|---|
| Purpose | The only contract between engine and clients |
| Responsibilities | HTTP control and queries, WebSocket topics, ingest socket, preview stream, origin and (LAN) token checks |
| Not responsible for | Any logic; it routes to owning actors |
| Inputs / outputs | [14 §4–5](14-runtime-and-api-contracts.md) |
| Data model | `bansayo-contracts`, generated TypeScript, golden examples |
| Interfaces | `/api/v1`, `protocol_version` 1, `hello.capabilities` |
| Dependencies | Axum stack (existing) |
| Configuration | `api.*` |
| Failure handling | Envelope with error codes; lagging clients lose oldest and refill |
| Performance | Frames serialised once; rate limits per client |
| Extension points | Additive topics, routes, optional fields; reserved binary encoding |
| Validation | Gate scripts (existing); golden examples; contract smoke in Godot |
| ADRs | 003, 004, [031](adr/ADR-031-local-first-execution.md), [035](adr/ADR-035-communications.md) |
| Status | Tracking API: Implemented. Training, content, athletes, progress, ingest, LAN mode: Proposed |

## 18. Desktop application

| Item | Content |
|---|---|
| Purpose | Training, analysis, authoring, calibration and device management on the PC |
| Responsibilities | Present engine state; hold view state and drafts; send commands |
| Not responsible for | Kinematics, metrics, scoring, recognition, alignment |
| Inputs | Stream topics and HTTP results |
| Outputs | Commands; content writes through the engine |
| Data model | Stores of [10 §4](10-desktop-architecture.md) |
| Interfaces | Network contract only |
| Dependencies | React, three.js, Tauri shell |
| Configuration | Presentation profile per athlete; local preferences |
| Failure handling | Empty states with instructions; reasons instead of zeros; reconnect |
| Performance | Per-frame data outside React; charts decimated |
| Extension points | New workspace = entry in the navigation table |
| Validation | Unit, component and Playwright journeys |
| ADRs | 006, 007, [032](adr/ADR-032-desktop-integration.md) |
| Status | Studio, Overview, Devices, Cameras (with live image), Sessions: Implemented. Train, Technique, Workouts, Progress, Calibration: Proposed |

## 19. VR application

| Item | Content |
|---|---|
| Purpose | Immersive and mixed-reality presentation of the same session, with space safety |
| Responsibilities | Render, guide, enforce headset-side safety, optionally forward XR poses |
| Not responsible for | Any analysis; first-person body |
| Inputs | Stream topics, HTTP results, OpenXR |
| Outputs | Commands; XR pose observations (P2) |
| Data model | Scripts of [11 §3](11-vr-architecture.md); avatar map ([25 §4](25-mixed-reality-and-avatars.md)) |
| Interfaces | Network contract only |
| Dependencies | Godot 4, OpenXR runtime |
| Configuration | `bansayo.presentation.vr/v1` |
| Failure handling | Fade on stream gap; pause on boundary or tracking loss; module fallbacks |
| Performance | [15 §2.3](15-performance-and-latency.md) |
| Extension points | New scene module mapped by the session director |
| Validation | Headless smoke, desktop mode, headset checklist |
| ADRs | 010, [033](adr/ADR-033-vr-runtime.md) |
| Status | Stream client, pose buffer, debug skeleton, status panel: Implemented, run in desktop mode only. Training scenes: Proposed |
