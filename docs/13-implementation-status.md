# 13 — Implementation and validation record

This record includes Phase 1 and Phase 0, both delivered on **17 November 2026**, followed by the
historical Stage 1–3 and camera/VR record. Phase 0 began at `b61c6b7`. Phase 1 is on `main`.

## Phase 1 — Roundhouse-kick slice on the simulator (17 November 2026)

Implements martial-arts [18 §3 Phase 1](architecture/martial-arts/18-implementation-roadmap.md)
and the slice of 18 §4. `protocol_version` remains **1**; every wire change is additive. Scope
limits of 18 §3 were kept: plan
kind `technique` only; no sequences, workouts, holds, assessments, free mode, references, DTW,
annotations, new sources, calibration procedures, smoothing changes, export, LAN or VR change.
New dependency: `rusqlite` 0.37 (bundled), building in the workspace and in
`apps/desktop/src-tauri`. Content hashes use an in-crate SHA-256 (`bansayo-training::hash`), so no
hashing crate was added. `bansayo-training` also depends on `serde_json` for event payloads and
canonical content JSON; the owner allowed this and further pure dependencies (ADR-018 amended, 14
§2 updated). The boundary stays: no Tokio, no I/O, no clock, never the engine.

### Deliverables per milestone

| Milestone | Delivered |
|---|---|
| M1 `bansayo-training` | Pure crate (no Tokio, I/O or clock). Observable catalogue for the kick definitions, metric pack and segmenter, with suppression reasons, live low-pass, backward differences and zero-phase re-filter (04 §6); evidence combination (04 §3); segmenter (07 §2); stage-1 phase machines compiled from definitions, recognition confidence, closed-set arbiter (05 §6, 07 §3); metric kinds `extremum`, `value_at`, `range`, `duration`, grade rule and comparability class (05 §8–9, 12 §5.3); scoring with `personal`/`definition` targets and null components (07 §7); cue gate and `after_rep`/`after_set` scheduler (07 §8.2); `ExerciseEngine` running a technique plan as a one-block, one-item workout (03 §1, 06 §2–9); content types, path-qualified validation and predicate compilation (13 §11); progress functions (12 §6). |
| M2 Content | `content/` with `taekwondo.dollyeo_chagi`, `taekwondo.ap_chagi`, the kick metric pack, `scoring.kick.basic` and English texts, all `status: draft`; engine `content_store` (scan, parse, resolve, compile, index, `content_rev`, snapshots by hash); configuration `schema_version: 2`, version 1 still loads. |
| M3 Runner, store, API | Training runner actor with the 14 §3 channel policies; SQLite store (WAL, foreign keys, `secure_delete`, one writer thread, numbered migrations); athletes, active athlete and the motion-data consent rule; provenance records; `training` and `observables` topics and `hello.capabilities`; content, athlete, training-session and progress routes of 14 §5.1–5.4; wire types with generated TypeScript and golden examples; `TrainingState.metric_availability`. |
| M4 Recording, re-analysis, golden | `motion.mcap` with indexed range reads for `/trace` and `/motion`; pinned re-analysis writing a linked session; `bansayo-engine regress <manifest>`; three synthetic golden sessions in `golden/synthetic/` (`bansayo.golden/v1`) checked in the normal test run. |
| M5 Desktop | Stores `trainingStore`, `trainingEvents` (ring of 256 with gap refill), `observablesStore` (outside React), `contentCache` (dropped on `content_rev` change) and `queryCache`; `MeasurementValue`, `ScoreBars`, `CapabilityBadge`, `CueBanner`, `PhaseTimeline`, SVG chart primitives, `ClipPlayer`; Train dashboard (athlete with consent, plan picker, recent sessions), live session (readiness and safety notes, counts, phase, live gauges, last repetition, cue, transport), session summary; Technique attempt browser and attempt view (clip, phase timeline, traces, metric table, scores); Progress with one line per comparability class, grade-B points secondary, break markers and per-class tables. Navigation keys follow 10 §2 and the default route is `#/train`. The UI computes no kinematics, metrics, scores or recognition. |

### Acceptance evidence (18 §3 Phase 1)

Windows 11, Rust 1.92, Node 24, `CARGO_TARGET_DIR=F:\drusniel-cache\cargo-target`. The
evidence below was first taken with `-j 4`: with full debug info the engine library was ~460 MB,
and 24 parallel test links ran out of memory (`LNK1102`, then "can't find crate"). The dev
profile now uses line-tables-only debug info and none for third-party crates (root and Tauri
`Cargo.toml`); the library is ~120 MB and builds and tests pass at default parallelism.

| # | Criterion | Command and result |
|---|---|---|
| 1 | Scripted 3 × 10 roundhouse: exact count, phase boundaries within two sample periods, scripted metric values within tolerance | `cargo test -p bansayo-training three_by_ten_roundhouse_session_counts_exactly phase_boundaries_within_two_sample_periods_and_metric_values_within_tolerance`: **pass** (pure session on the simulator's scripted kinematics and ground-truth phase times). |
| 2 | Two re-analyses give identical event logs | `cargo test -p bansayo-engine --test training recorded_session_is_reviewable_and_reanalyses_are_identical`: **pass** (two pinned re-analyses of a recorded session, compared event by event; linked `reanalysis_of`, `analysis_rev` 2). |
| 3 | One camera disabled mid-set lowers grades or suppresses metrics per repetition; session completes | **Partly met.** Golden session `20261117T090905Z-ni9h` (SIM-CAM2 disconnected after repetition 1) completes with three counted repetitions and each repetition's evidence records the loss (sources drop SIM-CAM2), but no grade changes and no metric is newly suppressed. See "Contradictions" below. |
| 4 | A second session appears beside the first in Progress; a different pose-model id is a separate series | `cargo test -p bansayo-engine --test training a_second_session_and_a_different_pose_model_appear_in_progress`: **pass** (two sessions → two points in one class; restart with `detector_id: simulated-projection-b` → a second class with one point). Pure: `progress::tests::classes_stay_separate_and_b_points_are_secondary`. |
| 5 | At most one cue per repetition (scheduler replay test) | `regress` check `cues.max_per_rep ≤ 1` on all golden sessions and masks; `coaching::tests::at_most_one_cue_ordered_by_category_priority_then_deviation`; engine journey asserts it on live state: **pass**. |
| 6 | Playwright journey pick → set → summary → attempt → progress | `npx playwright test` (from `apps/desktop`): **4 pass** — the new `training` project on `config/sim-training.yaml` (create athlete with consent, pick roundhouse 1 × 3, acknowledge safety notes, start, kicks counted, summary with three attempts, attempt view with phase timeline, metric table and playing clip, Progress with one simulated series class, 1440/1024/800 px without horizontal scroll) and the unchanged tracking journey and primitives tests. |
| 7 | Session/item machine tables; any command sequence leaves a defined state | `exercise::tests::{session_machine_table, item_machine_table, property_any_command_sequence_leaves_a_defined_state}`: **pass**. |
| 8 | Observable tests incl. every suppression rule; rigid-transform and mirror properties | `observables::tests::{every_suppression_rule_reports_its_reason, property_rigid_transform_invariance, property_mirror_swaps_sides_and_negates_signed_observables}` and the Phase 0 angle tests: **pass**. |
| 9 | Store: migration round trip; invariants I1, I3, I4, I6, I9; deleted session leaves no row | `store::tests::{migration_round_trip_and_settings, invariants_and_deletion}` and the engine journey's delete check (session, re-analyses and files removed): **pass**. |
| 10 | Three synthetic golden sessions pass `regress` in the normal run | `cargo test -p bansayo-engine --test golden`: **pass** (≥ 20 checks over three sessions and two masks). |
| 11 | fmt, clippy, tests (workspace and Tauri), gate scripts, desktop tests and build | See "Final checks" below. |
| 12 | Generated TypeScript and golden examples current | `cargo test -p bansayo-contracts generated_contracts_are_current`: **pass** after regenerating the eight review and re-analysis types the WIP commit had added without TypeScript. |

### Final checks (criterion 11)

| Check | Result |
|---|---|
| `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings` | **pass** |
| `cargo test --workspace --no-fail-fast` (default parallelism, three consecutive runs) | **pass**: 30 test binaries, 178 passed, 0 failed, 2 ignored (the existing photo-detector test without local models; the on-purpose golden generator) |
| Tauri crate: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` | **pass** (1 test); `rusqlite` resolves and builds there |
| `node scripts/verify-gate1.mjs`; `node scripts/verify-gate2.mjs` | **pass**: 8 and 5 real-process checks; scripts unchanged |
| Desktop `npx vitest run`; `npm run build`; `npx playwright test` | **pass**: 54 unit tests; build (existing large-chunk advisory); 4 Playwright tests |
| `crates/bansayo-engine/tests/training.rs` repeated 8 times, partly under extra load | **pass** 8/8 after the re-analysis fix |

### Fixes made while completing the phase

- Generated TypeScript was stale for `RepTrace`, `MotionClip`, `ReanalyseRequest` and related types.
- `GET /training/reps/{s}/{i}/motion` filled `positions_m` with keypoints indexed by segment; it now
  carries segment origins from the engine's forward kinematics, so clients draw without any.
- Re-analysis of a session whose stored row was still `active` could replay a partial event log
  (missing the `start` command) and produce an empty aborted session under load; the route now
  answers `conflict` until the row is closed, and tests wait for the stored status.
- Configuration validation's write probe collided between parallel validations on Windows
  ("Access is denied" for a name being deleted); probes now have unique names.
- English texts were missing for `cue.positive.improved`, `component.*` and three safety notices.

### Contradictions found in the documents

- **18 §3 criterion 3 vs 05 §8 and 04 §8.** On the simulator, origin `simulated` caps every grade
  at B (grade A requires origin `live`); grade B depends only on method and minimum confidence;
  camera-fit skeleton values have `null` sigma while `body_proportions` is `assumed` (04 §8 "FK
  position"). Removing one of three cameras leaves two good views, so method, confidence and
  sigma are unchanged and the specified rule yields the same grades. Removing a camera from a
  two-camera rig instead leaves no triangulation, so tracking is lost and the session pauses
  rather than completing. Criterion 3 therefore cannot be met as written on a simulated rig
  without a new grading input. Not redesigned; candidates for the owner: propagate keypoint sigma
  through the skeleton fit (04 §8 "segment long axis") and let grade B use it, or restate the
  criterion as "records the loss in each repetition's evidence".
- 18 §4 shows `/content/index` and `/capability`; the capability report is in `SystemState`
  (Phase 0), so the desktop reads it there.

### Defaults chosen

- Desktop navigation keys follow 10 §2 (Train 1, Technique 2, Progress 4, Overview 5, Studio 6,
  Devices 7, Cameras 8, Sessions 0); the default route is `#/train`.
- The Progress chart's x axis is session time (UTC); grade-B points are drawn dashed and hollow.
- Live gauges infer a display unit from the observable's id, because `ObservableValue` carries
  no unit (marked as a shortcut in the code).
- The attempt clip uses the current skeleton definition for forward kinematics (shortcut until
  skeleton definitions are stored per session).
- The training journey runs a second engine and dev server (ports 17701 and 15174) on
  `config/sim-training.yaml`, leaving the existing tracking journey unchanged.

### Notes for Phase 2

- On the camera-only engine path the chamber, pivot and extension phases of the golden sessions
  start at the same instant (zero-length chamber and pivot) and recovery is long (~0.9 s); the pure
  3 × 10 test on scripted kinematics does meet the two-sample bound. Real-camera tuning should
  start from these recordings.
- Grade lowering on view loss needs propagated sigma through the skeleton fit (see above).
- `ObservableValue` should carry its unit (or the content index should list observable units).
- Store the skeleton definition with each session so clips survive skeleton changes.
- Re-analysis with `content: current` answers `400` (not in Phase 1 scope).

## Phase 0 — Engine fit (17 November 2026)

Phase 0 is complete. It adds no training feature, HTTP route, WebSocket topic, workspace or
screen. `protocol_version` remains **1**. The two wire additions are optional:
`TriangulatedLandmark3D.sigma_m` and `SystemState.capability_report`.
The martial-arts architecture documents were not modified.

### Delivered A–E

| Deliverable | Implementation |
|---|---|
| A — Engine fit | T-01: storage, ordering and loss per `(device, observation kind)`; existing eight-frame camera history retained; primary-stream rate excludes multiplexed streams. T-02: common context contains events, clock, cancellation and drops; simulation controls live in an extension; static source-spawning match, no `dyn Source`. T-03: all nine declared calibration kinds survive migration, manifest loading, saving and replay contexts; uninterpretable payloads remain intact with state `invalid`. T-05: device times pass through the offset estimator; camera receive timestamps subtract configured latency. T-06: propagated triangulated sigma uses accepted-view RMS, distance, focal length, ray angle and view count, with validated `evidence.detector_noise_floor_px` (default 2.0). T-07: existing reference-frame and rate validation pinned without changing validation behaviour. T-04: unconsumed capabilities remain unavailable with `not_implemented`; no solver stage added. |
| B — Pure crate | `bansayo-training::observables::angles` is the single joint-angle implementation called by the engine. Direct dependencies are contracts and nalgebra only. Existing frame joint ids and non-trunk arithmetic remain. Trunk lean now uses world up projected into the pelvis sagittal plane, positive forward, suppressing an undefined projection. No further observable catalogue. |
| C — Contracts and seams | Rust movement/evidence and capability types; normative 26-point `bansayo.body.v1.points`; pose-to-motion conversion with FK endpoints; camera-fit twist inferred and body gyro unavailable. Internal `Streams.motion` watch matches live/replayed frame sequences. Reports extend the feature rules with conservative overrides and SHA-256 identities. TypeScript and all 44 golden JSON examples are current, including ten new movement/schema/report examples. Generated TypeScript whitespace is normalised by its generator. |
| D — Simulator | Front-kick trajectory reused, with phase/peak metadata. Added scripted roundhouse, sensor-frame gyro streams, and opt-in toe/heel projections via simulated-source `landmark_schema: bansayo.body.v1.points`. Default remains the 19-point schema. Gyro history resets at motion/link discontinuities. |
| E — Desktop | Button, Field, Select, Dialog, Drawer and Readout use CSS modules and existing tokens. Native controls, accessible labels/help/errors, disabled semantics, modal focus wrapping, Escape and focus restoration. `parseHash` returns `{ workspace, rest }`; existing workspace navigation remains. No screen or navigation entry. |

### Acceptance evidence

Validated in an isolated worktree on **macOS arm64**, Rust **1.92.0**, Node **24.20.0**, installed
Chrome. `CARGO_TARGET_DIR` was unset: fresh worktree and native targets were built. The supplied
Windows drive paths and PowerShell workaround did not apply to this host.

| # | Criterion | Command and result |
|---|---|---|
| 1 | Workspace/native checks | `cargo fmt --all --check`; `cargo clippy --workspace --all-targets -- -D warnings`; `cargo test --workspace`: **pass, 119 passed, 0 failed, 1 pre-existing ignored**. `cargo fmt --manifest-path apps/desktop/src-tauri/Cargo.toml --check`; `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings`; `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml`: **pass, 1 test**. |
| 2 | Unchanged process gates | `node scripts/verify-gate1.mjs`: **8 groups pass**. `node scripts/verify-gate2.mjs`: **5 groups pass**. Both scripts are byte-for-byte unchanged from `b61c6b7`. |
| 3 | Desktop | From `apps/desktop`: `npm test`: **41 pass**; `npm run build`: **pass**; `npm run test:e2e`: **3 pass**, including the byte-for-byte unchanged original tracking journey. Two old unit expectations were necessarily adapted from a string to the explicitly requested `parseHash` object; their route semantics remain the same. |
| 4 | Registry regression | `cargo test -p bansayo-engine --test phase0_registry`: **2 pass**. Independent diagnostics sequence does not replace orientation; duplicates/reordering are checked per kind. Includes T-07 validation boundaries. Existing interpolation/triangulation regressions pass. |
| 5 | Angle properties | `cargo test -p bansayo-training`: **4 pass**. Rotation grids check relative-angle rigid-transform invariance and left/right reflection. World-up trunk lean is checked under heading changes and reflection, with positive-forward and degenerate-plane assertions. LLVM coverage: **100% of the moved angle module's lines, regions and functions**, not a whole-repository coverage claim. |
| 6 | Recorded frame bits | `cargo test -p bansayo-engine --test phase0_replay`: **pass**. Frozen **7.5 s MCAP, 451 frames**, captured before production edits, lives in `crates/bansayo-engine/tests/fixtures/phase0/`. All eight non-trunk joint channels compare with `f32::to_bits`; frame times and joint counts match. Frozen forward leans change from negative to positive. Reference motions: squat, front kick, standing. Initial capture is preserved in commit `50ed5ab`, never regenerated by normal tests. |
| 7 | Pure dependency boundary | `cargo build -p bansayo-training`; `cargo tree -p bansayo-training`: **pass**, no Tokio or bansayo-engine in the tree. No clock, filesystem or network operations in the library. No registry dependency added; lockfiles add the local crate. |
| 8 | Generated output | `cargo test -p bansayo-contracts generated_contracts_are_current`: **pass without blessing**. Full contract tests additionally pass round trips, deterministic generation and old JSON without optional additions. Re-run after generator whitespace normalisation: **all pass**; desktop rebuild also passes. |
| 9 | Capability state | `node scripts/verify-phase0.mjs`: **4 groups pass**, reports observed in the existing state stream: full-body **hybrid**, six trackers **inertial**, camera-only **basic**, empty **none**. Trackers checked after mounting capture. `cargo test -p bansayo-engine --test phase0_motion`: **5 pass**, also covering multi-camera, camera-fit, stale inputs, overrides, identities and evidence boundaries. |

Additional checks: `npm audit`: **0 vulnerabilities**; `git diff --check b61c6b7..HEAD`: **pass**.
The existing ignored test is `sources::camera::pose::tests::detects_a_person_in_a_photo`: local
models and `BANSAYO_POSE_IMAGE` are absent, so it was **not run**. Hardware, accuracy and packaged
installer checks remain outside Phase 0. The successful desktop build retains its existing
large-chunk advisory. No debug log, runtime data directory, models or node_modules were committed.

Intermediate failures were investigated, not hidden. Regressions cover diagnostics clobbering
orientation, calibration loss (including replay contexts), unrelated triangulation sigma leaking
into an inferred root, gyro spikes on script changes, and modal focus/strict-effect failures.
Gate 2 intermittently rejected mounting capture with "Movement detected": queued observations
from the previous simulation motion entered the window. Capture now accepts only samples
captured inside its window, and the simulator reads current controls before emission. The
unchanged Gate 2 passed twice consecutively after that fix, including the final complete run.
Native select tests use the selection API to avoid host popup keyboard differences; modal Tab,
Escape and focus restoration are tested directly.

### Code/document discrepancies and resolutions

- The requested `Store::load` does not exist. Current code already persists mounting and camera
  profiles through authoritative `state/runtime.yaml` plus legacy migration. The manifest rejected
  uninterpretable profiles; legacy loading retained only summaries. Both paths and replay contexts
  now preserve complete invalid profiles, while consumers refuse to use them.
- Background docs/03 and ADR-016 still say camera-only poses are null. Source already fits a
  world skeleton from calibrated triangulated landmarks without trackers, as martial-arts 08 §1.2
  records. This implementation was retained with inferred twist evidence; no replacement solver.
- ADR-017's original non-interpolation decision no longer describes current code. Existing
  eight-frame interpolation was retained and regression tested; no new alignment mode/procedure.
- O-01 was confirmed by execution: the old trunk formula returned a negative forward lean and
  ignored the pelvis plane. Only that joint was reconciled; recorded non-trunk values retain bits.
- Front kick already existed; roundhouse, feet and gyro did not. Front kick was reused.
- O-20/O-22 inspection confirms capture/inference, camera imports, interpolation and camera-fit
  code exist. One-Euro smoothing is always active with fixed constants and no config switch.
  Its behaviour remains unchanged; its actual constants enter the processing identity. Moving
  smoothing off recorded inputs remains Phase 2 work.
- The existing parser is `serde-saphyr`, unlike the older ADR-012 `serde_norway` choice. Retained.
- Literal unchanged string-return assertions conflict with the requested object-returning
  `parseHash`. Those two assertions were adapted; gate scripts and the original browser journey
  remain unchanged. No martial-arts architecture document was edited.

### Phase 1 handoff

- Consume `Streams.motion`, or replace its latest-sample delivery with ADR-018's bounded training
  channel. No training runner, training API, database, motion recording format or training topic was added.
- `planar` is deliberately empty. A one-camera report can be `basic` while world joints remain
  unavailable. Planar normalisation and observable realisations are deferred.
- Camera-fit axis evidence must not be promoted to observed twist. FK positions still depend on
  assumed geometry and have null sigma. Root sigma exists only for a contributing triangulated
  measurement, not an inferred floor-contact root.
- `input.<device>.<capability>` report entries are reason-only declarations of unconsumed streams,
  not implementations of the future observable catalogue. Gyro is recorded but does not yet
  provide available body angular velocity.
- Opt into the feet schema explicitly. `Motion::kick_truth()` supplies later recognition tests
  with phase times and peaks. Gyro is a sensor-frame finite difference of noise-free truth, with
  a zero first sample after discontinuities.
- Triangulation sigma is propagated, not validated accuracy. Sync-motion uncertainty, empirical
  tables, new fusion stages and filter changes remain deferred.
- Profiles follow currently usable input facts; future consumers must retain that rule. Overrides
  cannot raise a profile. No new source adapters, solver stages, VR changes or Phase 1 features
  were implemented.

### Local commit ledger

RED and GREEN checkpoints are retained. The documentation commit follows this ledger. The work
is brought onto `main` by fast-forward, with no push.

```text
50ed5ab engine: capture pre-phase0 simulator frames and failing registry regression
cbeefa6 engine: retain independent observation streams and pin IMU validation
1f75b93 engine: specify common source context and simulator extension
13a064a engine: separate simulator context and statically dispatch source spawning
35f60cf engine: reproduce unsupported calibration loss on both load paths
07e97ec engine: retain uninterpreted calibration profiles with invalid state
1c503a5 engine: specify device and receive timestamp mapping
c1a1282 engine: map device timestamps and preserve receive-time latency semantics
a2322ea engine: specify triangulated uncertainty and detector noise defaults
4177d8c contracts: add propagated triangulated uncertainty and detector noise floor
db1269d engine: specify reasons for validated but unconsumed inputs
5776f65 engine: report unconsumed inputs as not implemented
11b2631 engine: specify pure angle crate with invariance mirror and sign tests
65a8c8d engine: share angle mathematics through the pure training crate
235f92d contracts: specify canonical movement schema and additive capability state
db7efdb contracts: define canonical movement evidence and optional capability report
7d23cd5 engine: specify movement evidence profiles and configuration identities
a584c40 engine: build canonical motion and publish evidence-based capability reports
efa364e simulator: specify feet gyro and kick ground-truth seams
8ad620d simulator: add feet projections gyro streams and scripted roundhouse metadata
1b753f4 desktop: specify shared controls and hash subsegments
f36561d desktop: add shared UI primitives and preserve hash subsegments
976fca1 desktop: reproduce modal focus wrap and strict effect lifecycle failures
31e35b4 engine: complete lint checks and lock the native training dependency
d109ad0 engine: reproduce opaque calibration loss in recording context
4194df2 engine: preserve invalid calibration payloads through replay contexts
696f6c6 desktop: contain modal focus and handle strict effect replay
45194dd engine: strengthen joint bit and degenerate-axis regression evidence
361b816 simulator: reproduce gyro spike across scripted motion changes
8d48488 simulator: reset gyro history at discontinuities and fingerprint filter constants
89d6224 engine: reproduce unrelated root uncertainty in movement evidence
e14f9c5 engine: keep root uncertainty tied to its contributing method
7cc41e7 engine: specify mounting capture timestamp boundaries
8496a16 engine: exclude queued pre-capture samples and apply current simulator controls
d2907b5 contracts: normalize generated TypeScript trailing whitespace
```

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
