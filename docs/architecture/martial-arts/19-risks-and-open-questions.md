# 19 — Risks and open questions

What could make this architecture wrong, what was not verified while writing it, and which
decisions wait for an experiment. Extends [docs/11](../../11-risks-and-validation.md); risk numbers
continue from its R21.

## 1. Risks introduced or sharpened by the training platform

| # | Risk | Consequence | Mitigation | Settled by |
|---|---|---|---|---|
| R22 | Detector landmarks of the striking limb are unreliable around the strike on ordinary webcams (blur, few frames) | The most interesting kick metrics (peak speed, height at apex) are often grade B or suppressed; the product looks empty to a one-camera user | Short manual exposure; honest readiness list before the session; counts, timing and phase metrics remain; IMUs are the stated tool for strike timing | V-30; Phase 2 experience |
| R23 | A Halpe-26 RTMPose model of suitable licence and speed may not be obtainable; the local `rtmpose-m.onnx` is COCO-17 | No foot points: support pivot, foot yaw, toe height and the feet-based contact estimate are unavailable | Schema maps make the detector replaceable; every foot-dependent observable already has an unavailable path; the capability report says why | [ADR-021](adr/ADR-021-pose-estimation-integration.md); O-02 |
| R24 | Free-running cameras limit accuracy at speed even after offset calibration and interpolation | Grade-A speed and apex-height metrics for fast kicks need hardware-triggered or high-rate cameras | `gated` remains available; sigma grows with speed × sync sigma so grades fall instead of values lying; camera guidance states the limit | V-07, V-08; [ADR-023](adr/ADR-023-time-aligned-triangulation.md) |
| R25 | The calibration burden (board, rig capture, floor, check) is too high for a home user | `multi_view` is rarely reached; calibrations go stale unnoticed | One wizard with live guidance; static anchor tags and reprojection monitoring detect a moved camera; `basic` stays useful without any of it | Phase 4 usability trial |
| R26 | The AprilTag binding may not build cleanly on Windows or may carry an unsuitable licence | No board calibration and no clusters | Detector behind a trait; pure-Rust port as fallback; the spike is the first task of Phase 4 | [ADR-024](adr/ADR-024-fiducial-markers-and-board-calibration.md); O-06 |
| R27 | Deterministic phase machines tuned on a few athletes miss valid executions by others (style, flexibility, speed) | Missed or `incomplete` repetitions; distrust | Thresholds normalised by body lengths; `extends` variants; tolerant optional phases; V-19 on several athletes before `reviewed`; stage-2 templates | V-19, V-20 |
| R28 | A wrong cue teaches a wrong habit | Harm to technique; loss of trust | Evidence gate and persistence; basis tag; one cue; default level `standard`; cue precision measured before `detailed` is default | V-24 |
| R29 | Comparability classes fragment history: every detector or method change starts a new series | Long-term progress looks like many short series | Continuity re-analysis with a source mask; method-difference display; classes hash only what changes the number (not the engine version) | V-34; Phase 4 |
| R30 | The architecture is large for a single developer | Layers built and never exercised | Slice-first roadmap; Phase 6 as optional packages; every component has a "not built until" condition | Roadmap discipline |
| R31 | The Godot client has never been run; GDScript JSON parsing at 90–120 Hz is unmeasured | VR frame budget missed | First task of Phase 5 is to run it; the stream rate is client-limited; existing ADR-010 names the native-extension fallback | L-07; O-10 |
| R32 | Pose inference and VR rendering share one GPU | Either the tracking rate or the headset frame rate drops | `max_inference_hz` is the first control; stage timing shows which side yields | L-07, L-08 |
| R33 | Two stores (SQLite rows, MCAP files) can disagree after a crash | Rows without clips or files without rows | Rows are authoritative; `has_*` flags are corrected at start; orphan directories are removed at start; `pending.jsonl` recovery | Crash tests in Phase 1 |
| R34 | Content without an expression language is too restrictive for instructors | Pressure to add scripting | Each unmet need is logged as a candidate catalogue entry; the boundary is revisited only on the pattern of such requests ([ADR-030](adr/ADR-030-configuration-and-content.md)) | Phase 3 authoring experience |
| R35 | Soft-tissue motion of carriers and trackers is not modelled | Tag- and IMU-derived rotations are biased during impacts and fast direction changes | Stated limitation; cross-method disagreement is exposed; slip detection | V-09, V-10 |
| R36 | Safety limits are project defaults without validation | A limit may be too lax or needlessly strict | Conservative values; instructor review per discipline; wording that never presents them as prescriptions ([17 §3](17-security-privacy-and-safety.md)) | Instructor review |
| R37 | No `expert` reference can ship without a performer, a calibrated rig and a written release | Ghost comparison starts with the athlete's own attempts and labelled synthetic clips only | Reference kinds make this explicit; `athlete_selected` works from Phase 3 | Phase 4 + a recording session |
| R38 | Scheduling jitter on Windows at a 120 Hz tick with inference threads active | Uneven tick spacing | Observables use observation times, not tick times; ticker thread already exists; jitter is measured | L-08 |
| R39 | Other people in the camera view (a class, a family member) | Subject selection jumps to another person | Largest-and-central rule at `ready`, overlap tracking, `multiple_people` notice instead of guessing | V-31 |
| R40 | The 12 Hz analysis low-pass attenuates true peak speeds of fast strikes | Speed metrics read low, consistently within a class but not across cut-offs | Cut-off is part of `tracking_config_id`; chosen by experiment | V-15, V-16 |
| R41 | Assumed body proportions distort every FK-derived position and normalised length | Normalised metrics differ before and after body-proportion calibration | `norm_basis` records `valid` or `assumed` in the comparability class | — (by design) |

## 2. Data, models and licences

| Asset | Position | Open point |
|---|---|---|
| Pose and detector weights (RTMPose, YOLOX) | User-supplied local files; not distributed with the application until reviewed (existing ADR-009) | Terms of the training datasets behind each pretrained weight file |
| Halpe-26 keypoint definition | Used as a schema (names and order) | Licence of any Halpe-trained weights |
| Public martial-arts or general motion datasets | **Not used.** Their licences were not reviewed in this task, their marker sets and skeletons differ from `bansayo.body.v1`, and none matches this sensor mix | Reviewed only if stage-3 recognition is ever started ([07 §3.4](07-recognition-and-coaching.md)) |
| FreeMoCap | Ideas only, no code, no importer (existing ADR-014) | — |
| AprilTag library and tag families | Planned dependency behind a trait | Licence and maintenance of the chosen binding (O-06); family facts not re-verified (O-05) |
| ONNX Runtime, capture crate, SQLite binding, MCAP | Planned or existing dependencies | Licence and version of each confirmed when it is added to `Cargo.toml`; none was added by this task |
| Segment mass fractions for the balance proxy | Published anthropometric table (Winter) | Exact table and edition cited in the code that embeds it |
| Reference clips shipped with the project | Recorded for the project, written performer release, CC BY 4.0 | None exist yet |
| User content | `licence` field per definition and reference; private by default | — |
| Kata and poomsae definitions | Step lists are factual descriptions authored for the project, attributed to a named style | An instructor of that style reviews each before `reviewed` |

## 3. Open items

Things assumed, read but not run, or not checked while producing this set.

| Id | Item | How it is settled | Blocks |
|---|---|---|---|
| O-01 | `trunk_forward_lean` in `pipeline/joints.rs` appears, by reading, to return a negative value for a forward lean and to ignore the pelvis frame, contradicting docs/03. Not executed | Unit test asserting the documented sign during the move to `bansayo-training` | Phase 0 |
| O-02 | The local `rtmpose-m.onnx` is taken to be COCO-17 from its source configuration; its output shape was not inspected. Availability, licence and speed of a Halpe-26 export are unconfirmed | Inspect the model; export or obtain a Halpe-26 model; measure inference time | Phase 2 |
| O-03 | `nokhwa` with Media Foundation: manual exposure control, MJPEG at high frame rates and several cameras on one controller are unverified | The capture spike of ADR-020 | Phase 2 |
| O-04 | `ort` version, CUDA/cuDNN pairing and DirectML fallback on the target machine (existing R7) | Pin and run | Phase 2 |
| O-05 | AprilTag family facts (cell counts, identifier counts, relative false-positive behaviour) are quoted from memory of the published families, not re-read from the sources | Read the library's family definitions; correct [09 §2](09-optical-markers-and-calibration.md) and the range table if needed | Phase 4 |
| O-06 | Which AprilTag binding crate: licence, maintenance, Windows build | Spike | Phase 4 |
| O-07 | Measurement ranges of the LSM6DSV80X / LSM6DSV320X high-g variants were not read from datasheets; whether ±16 g saturation at the shin matters is unmeasured | Datasheets; V-17 | Package 6b |
| O-08 | Whether SlimeVR Server's SolarXR feed exposes raw angular rates (existing R3) | Read the schema; test | Package 6b |
| O-09 | Kick and punch speed figures used for blur and skew arithmetic (13–18 m/s, 15 m/s worked example) are literature-order values quoted from abstracts | V-30 measures the test athletes' own speeds | Camera guidance |
| O-10 | The Godot client has never been imported or run | Install Godot; run the smoke test and a session | Phase 5 |
| O-11 | `serde_norway` maintenance status (existing ADR-012 check) | Check when the content loader is written | Phase 1 |
| O-12 | `rusqlite` with bundled SQLite inside both the workspace engine and the out-of-workspace Tauri shell build | Build both | Phase 1 |
| O-13 | Windows per-user data-directory permissions as created by the engine | Inspect ACLs on a fresh install | Phase 1 |
| O-14 | Passthrough availability and quality per OpenXR runtime | Headset trial | Phase 5 |
| O-15 | The `time_sync` cross-correlation procedure has no implementation and no evidence | V-07 | Phase 4 |
| O-16 | Size of `motion.mcap` per minute with every observable at 120 Hz | L-08 | Retention defaults |
| O-17 | Whether the desktop's session-clock estimator (needed for local timers and the metronome) is accurate enough for audible tempo | Measure beat error against the engine clock | Phase 3 |
| O-18 | Style attributions ("Kukkiwon/WT", "Shotokan") of the initial definitions were written by the project, not by instructors of those styles | Instructor review | `reviewed` status |
| O-19 | This document set was produced over two sessions; documents 01–14 and the examples were re-read, link-checked and parse-checked in the second, but no schema validator exists to check the YAML examples against [13](13-configuration-schemas.md) mechanically | The content loader's validator, run over `examples/` as its first test corpus | Phase 1 |

## 4. Decisions pending validation

| Decision | Status | Criterion | Experiment |
|---|---|---|---|
| [ADR-020](adr/ADR-020-camera-processing-stack.md) capture crate and decoder | Pending Validation | Manual exposure works; target frame rates reached with the planned camera count; decode ≤ 4 ms | Capture spike; L-01 |
| [ADR-021](adr/ADR-021-pose-estimation-integration.md) pose model and runtime | Pending Validation | Halpe-26 model obtained under acceptable terms; inference within budget | O-02; [15 §2.1](15-performance-and-latency.md) |
| [ADR-023](adr/ADR-023-time-aligned-triangulation.md) `aligned` as default | Pending Validation | Stated in [16 §3](16-accuracy-and-validation.md) V-08 | V-07, V-08 |
| [ADR-024](adr/ADR-024-fiducial-markers-and-board-calibration.md) binding; size rules | Pending Validation | Builds on Windows under a compatible licence; V-05 confirms the envelope | Spike; V-04 – V-06 |
| [ADR-033](adr/ADR-033-vr-runtime.md) GDScript client at headset rates | Pending Validation | Frame and parse budgets of [15 §2.3](15-performance-and-latency.md) | L-07 |
| `time_sync` by cross-correlation ([09 §8](09-optical-markers-and-calibration.md)) | Pending Validation | Mean absolute error ≤ 2 ms against LED truth | V-07 |
| Passthrough as the default view for kicking drills ([11 §7.3](11-vr-architecture.md)) | Pending Validation | Available and comfortable on the target runtime | Headset trial |
| Every measurement-affecting default | Starting value | [16 §8](16-accuracy-and-validation.md) | As listed there |

## 5. Design self-review

The twenty questions of the brief, answered against the documents.

| # | Question | Answer | Where |
|---|---|---|---|
| 1 | Can the system operate with only one camera? | Yes, at profile `basic`: counting, timing and image-plane angles; no metres, no speeds | [08 §4.2](08-tracking-and-sensor-fusion.md), [05 §10.6](05-technique-library.md) |
| 2 | Can another camera be added without modifying exercise logic? | Yes. `bansayo-training` cannot name a device; it reads observables | [14 §2.1](14-runtime-and-api-contracts.md) |
| 3 | Can printed tags be introduced without replacing the pose model? | Yes. Clusters constrain rotations in arbitration; joint positions stay with landmarks | [09 §11](09-optical-markers-and-calibration.md) |
| 4 | Can an IMU improve a kick measurement without becoming mandatory? | Yes. It adds `imu_gyro` timing and bridges occlusion; without it the same metric is realised by another method or graded lower | [08 §5](08-tracking-and-sensor-fusion.md), [08 §7.3](08-tracking-and-sensor-fusion.md) |
| 5 | Can a missing camera be handled without a complete session failure? | Yes. Views drop, sigma grows, the profile may fall; the session pauses only when no required observable remains | [08 §10](08-tracking-and-sensor-fusion.md) |
| 6 | Can sensors operate at different sample rates? | Yes. Latest value per (device, kind) with its own observation time; derivatives across observation times | [04 §3](04-movement-domain-model.md), [15 §3](15-performance-and-latency.md) |
| 7 | Can a user change from desktop to VR without rewriting workouts? | Yes. One session, one content store; presentation profiles are per client | [14 §6](14-runtime-and-api-contracts.md), [11 §5](11-vr-architecture.md) |
| 8 | Can one technique have different Karate, Taekwondo and Muay Thai variants? | Yes. Each is its own definition in a shared family; `extends` for school variants | [05 §2](05-technique-library.md), [05 §4](05-technique-library.md) |
| 9 | Can new exercises be defined mostly through configuration? | Yes, within closed catalogues of observables, metric kinds and rule kinds; a need outside them is a code change with a test | [13 §1](13-configuration-schemas.md) |
| 10 | Can kata and poomsae use the same foundation as single techniques? | Yes. Forms consume the tokens of technique machines through the shared sequence automaton | [06 §4](06-exercise-and-workout-engine.md), [07 §4.2](07-recognition-and-coaching.md) |
| 11 | Can flexibility be measured without inventing unavailable observations? | Yes. Active/supported is a property of the chosen definition; joint range needs the pelvis orientation and is otherwise replaced by the separate limb-elevation metric | [03 §8](03-training-modes.md), [05 §10.5](05-technique-library.md) |
| 12 | Can measurements be compared fairly across sessions? | Only inside a comparability class, with break markers at calibration changes | [12 §5](12-data-and-progress.md) |
| 13 | Can the system distinguish improved movement from improved sensing? | Operationally: classes, continuity re-analysis under a source mask, method-difference display, and a noise term that includes the validated minimum detectable change | [12 §5.5](12-data-and-progress.md), [16 §7](16-accuracy-and-validation.md) |
| 14 | Can recorded data be replayed for future algorithm evaluation? | Yes. Inputs are recorded; re-analysis produces a new linked session | [12 §7](12-data-and-progress.md) |
| 15 | Can unreliable measurements be excluded from scoring? | Yes. Grades gate scoring; components are `null`; the composite needs coverage | [05 §8](05-technique-library.md), [07 §7](07-recognition-and-coaching.md) |
| 16 | Are sensor fusion and movement recognition separate concerns? | Yes, by crate boundary: fusion ends at `BodyState` in the engine; recognition lives in `bansayo-training` | [ADR-018](adr/ADR-018-training-crate-and-runtime-boundaries.md) |
| 17 | Are runtime processes and module boundaries justified? | Three process kinds, no services, three crates, each with its reason | [14 §1](14-runtime-and-api-contracts.md) |
| 18 | Can the system run locally without mandatory cloud services? | Yes. There are none | [ADR-031](adr/ADR-031-local-first-execution.md) |
| 19 | Are safety and privacy addressed appropriately? | Addressed by design rules and defaults; the limits themselves are unvalidated (R36) | [17](17-security-privacy-and-safety.md) |
| 20 | Can the first useful implementation be delivered without the whole platform? | Yes. Phases 0–2 need no calibration solver, no tags, no IMUs, no VR | [18 §3](18-implementation-roadmap.md) |

Weaknesses this review exposed, kept visible rather than argued away:

- Question 13 has no purely algorithmic answer. The architecture prevents silent mixing and
  quantifies method differences; it cannot prove that a change inside one class is not a slow
  drift of calibration. The calibration check and anchor tags bound that, they do not eliminate it.
- Question 1 is true but thin for striking: with one ordinary webcam the metrics that survive are
  counts, timing and normalised planar values (R22).
- Question 9 depends on the closed catalogues being rich enough; R34 tracks the pressure.

## 6. What this architecture task did not do

- No code was written, compiled or run; no test was executed.
- No hardware was used; every number is a design value or a target.
- No dependency was added; crate choices marked pending are untested on this machine.
- No licence was reviewed beyond what docs/12 already records.
- No instructor reviewed the technique, form or safety content.
