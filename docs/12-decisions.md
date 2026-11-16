# 12 — Architecture decisions

Each record states the decision, why, what was rejected, and what would make us revisit it.
Status is **Accepted** unless noted.

## ADR-001 — Modular monolith in two crates

**Decision.** The engine is one process. Code is split into `bansayo-contracts` (data that crosses
a boundary) and `bansayo-engine` (library and binary). Module boundaries inside the engine are
enforced by review and by keeping pure modules free of Tokio and I/O.

**Why.** One process removes a whole class of failure and latency. Two crates give the one
compiler-enforced boundary that matters now: contracts cannot depend on runtime code.

**Rejected.** Microservices, a message broker, one crate per module.

**Revisit when** something other than the engine needs the domain logic without the runtime (a
Godot native extension, an offline analysis tool). Then the pure modules move to `bansayo-core`.

## ADR-002 — One coordinate, quaternion and time convention

**Decision.** World is right-handed, +Y up, +Z forward, metres, floor at `y = 0`. Quaternions are
unit Hamilton `[x, y, z, w]`, named `q_a_from_b`. Angles are radians. Time is integer microseconds
on a single session clock. Segment frames are world-aligned in a T-pose rest. Adapters convert at
the edge.

**Why.** These match three.js, Godot, glTF and OpenXR in handedness, up axis and unit, so no
client flips an axis. A world-aligned rest pose makes joint angles and retargeting simple.

**Rejected.** Z-up; per-client conventions with conversion in the protocol; degrees on the wire.

## ADR-003 — JSON over WebSocket and HTTP, versioned

**Decision.** Streaming is server-push JSON over WebSocket; control is JSON over HTTP; one
integer `protocol_version`; additive changes are compatible and receivers ignore what they do not
know.

**Why.** Browser and Godot both parse JSON natively, and it can be read in a terminal. The data
rate of one skeleton is small.

**Rejected.** gRPC (no native browser or Godot support), MQTT and brokers (infrastructure with no
benefit on one machine), a custom binary protocol (premature).

**Revisit when** a measurement shows serialisation or bandwidth is a bottleneck — likely with
many clients or with landmark streams from several cameras. The remedy is a second encoding of
the same messages, negotiated at connection.

## ADR-004 — Rust is the source of truth for contracts; TypeScript is generated

**Decision.** Contract types are defined once in `bansayo-contracts`. `ts-rs` emits TypeScript into
`contracts/ts/`. Tests emit golden JSON into `contracts/examples/` and fail when checked-in output
is stale. The Godot client, which cannot consume generated types, is checked against the golden
examples.

**Why.** It removes the hand-maintained second model, which is where protocol bugs come from.

**Rejected.** JSON Schema as the source with generators for both languages (more tooling, weaker
Rust types); hand-written TypeScript types.

## ADR-005 — Record inputs to MCAP; JSON sidecar for metadata; SQLite deferred

**Decision.** A session stores source events and the context needed to re-solve them in an MCAP
file with JSON messages, plus a `session.json`. Playback re-runs the pipeline.

**Why.** Recording inputs makes sessions re-processable and makes replay an ordinary source. MCAP
is indexed, self-describing and has standard tooling. A directory listing answers every session
query the PoC has.

**Rejected.** JSON Lines (no index, no tooling); recording only solved frames (cannot be
re-solved); SQLite now (no query needs it).

**Revisit when** sessions need searching, tagging or cross-session measurement queries: add
SQLite as an index over the same files.

## ADR-006 — The desktop shell hosts the engine library and uses only the network contract

**Decision.** The Tauri shell links `bansayo-engine` and runs it in-process, or attaches to an
engine already listening. The web UI talks to the engine over HTTP and WebSocket in both the
browser and the desktop build. The shell's Cargo package is outside the workspace.

**Why.** One UI code path and one contract. The engine stays usable without Tauri, and building
or testing it never compiles the webview stack.

**Rejected.** Tauri IPC for tracking data (a second data path the VR client could not share); a
bundled sidecar binary (packaging complexity not needed yet).

**Revisit when** process isolation matters — for example if inference instability should not take
the window down. The sidecar is then a packaging change, not an architectural one.

## ADR-007 — three.js used imperatively, outside React's render cycle

**Decision.** A plain TypeScript viewport class owns the scene and animation loop and reads pose
frames from a non-React store. React manages panels and low-rate state through
`useSyncExternalStore`. No react-three-fiber, state library, router or chart library.

**Why.** Pose frames arrive far faster than interface state should change. Keeping them out of
React is the requirement, and the imperative form makes that separation structural.

**Rejected.** react-three-fiber (adds a reconciler between the stream and the scene); a state
library (the platform hook is enough).

## ADR-008 — Real IMUs arrive through SlimeVR Server first

**Decision.** The first real-tracker adapter is a SolarXR client that reads tracker data from a
running SlimeVR Server. A native UDP adapter and custom firmware come later, for what SlimeVR
firmware does not provide.

**Why.** SolarXR has a published schema. The tracker-to-server UDP protocol is documented only by
its source code; re-implementing it would be reverse engineering for a demo. SlimeVR firmware
already runs on the boards we have.

**Rejected.** Implementing the SlimeVR tracker protocol in the engine; starting with custom
firmware.

**Revisit when** we need raw high-rate IMU data, precise clock sync, or extra sensors on the
tracker.

## ADR-009 — Camera path: simulated in the PoC; RTMPose on ONNX Runtime when real

**Decision.** The PoC ships a simulated camera producing labelled simulated landmarks. The real
path will use a capture crate chosen by a spike on Windows 11, ONNX Runtime through `ort` with
CUDA, and RTMPose with a permissively licensed person detector.

**Why.** A capture and inference path cannot be verified in this iteration, and an unverified one
would be a false claim. RTMPose is Apache-2.0 and exports to ONNX.

**Rejected.** Ultralytics YOLO-pose (AGPL-3.0); requiring Python at runtime; presenting simulated
landmarks as inference.

**Open.** The terms of the datasets behind pretrained weights must be reviewed before any model is
distributed with the application.

## ADR-010 — VR client in GDScript on the shared stream

**Decision.** The Godot client is GDScript, consumes the same WebSocket stream as the desktop,
and performs only presentation and display interpolation.

**Why.** It keeps one body model and one biomechanics implementation. JSON parsing and transform
assignment do not need native code.

**Rejected.** godot-rust/gdext now (a native build per platform for no functional gain); a
separate VR protocol.

**Revisit when** the client needs shared Rust code or measured parsing cost matters at headset
frame rates.

## ADR-011 — Fusion as ordered stages; no estimator framework yet

**Decision.** The solver is a short sequence of plain functions. A stage trait and a configurable
list arrive with the second implementation of any stage. No EKF or factor graph in the PoC.

**Why.** The current stages are closed-form. A filter tuned on simulated data demonstrates
nothing.

**Revisit when** camera and IMU observations must be reconciled under occlusion with real data.

## ADR-012 — YAML through `serde_norway`

**Decision.** Configuration is YAML parsed with `serde_norway`.

**Why.** `serde_yaml` is archived and `serde_yml` is deprecated. `serde_norway` is the maintained
fork with the same API.

**Rejected.** TOML (the brief specifies YAML); the deprecated crates.

**Check at implementation.** Agent 1 confirms the crate's current maintenance status before
adding it, and reports back if it has changed; `serde-saphyr` is the fallback.

## ADR-013 — Loss of tracking returns a segment to its rest pose

**Decision.** A segment without a usable observation takes its rest rotation relative to its
parent and is marked inferred. It does not hold its last pose.

**Why.** A frozen limb looks like data. A limb that snaps to rest and changes style cannot be
mistaken for tracking.

**Rejected.** Hold-last-value; extrapolation.

**Revisit when** stage 5 constraints can produce a better inferred pose. The label stays
`inferred` either way.

## ADR-014 — Triangulation is written independently; no FreeMoCap code or importer

**Decision.** Multi-camera triangulation is native Rust written from standard multi-view geometry
(`pipeline/triangulate.rs`). No code from FreeMoCap is used or ported, and a FreeMoCap recording
importer or comparison mode is deliberately not built.

**Why.** FreeMoCap is AGPL-3.0 and this project is MIT. Copying or translating its code would
bring AGPL obligations into the project. The method itself (linear triangulation, reprojection
check) is textbook and small enough to write and test directly.

**Rejected.** Porting FreeMoCap's triangulation; calling it as a Python subprocess; an importer
that links its code.

**Revisit when** licensing is reviewed and settled for the whole project. Until then the same
rule as ADR-009 applies: copyleft code stays out. At most, FreeMoCap may later serve as an offline
comparison reference: its exported results compared against ours outside the product, with no code
linked or copied.

## ADR-015 — Camera models are calibration profiles

**Decision.** A camera's intrinsics and extrinsics are `camera_intrinsics` and `camera_extrinsics`
profiles in the existing calibration store, with typed `data` (`CameraIntrinsicsData`,
`CameraExtrinsicsData`). Simulated cameras declare theirs from configuration, in memory
(`declared_by_simulator`). Their data is also published in `CalibrationProfileSummary.data`. There
is no calibration solver.

**Why.** It reuses the store's states, invalidation, persistence and recording instead of adding a
second place that holds calibration, and clients draw cameras from state they already receive.
Solvers keep reading profiles and never compute calibration (see [05](05-calibration-and-fusion.md)).

**Rejected.** Camera parameters in the YAML read directly by the solver; a separate camera-rig
file; a solver written without real boards or detections to test it against.

**Revisit when** real cameras exist and a calibration procedure is built; it writes the same
profiles.

## ADR-016 — Triangulation is a solver stage; its results are derived, not recorded

**Decision.** Triangulation runs inside `pipeline::solve`, before root placement, so live mode and
replay share one code path. Triangulated landmarks are computed from the recorded 2D landmarks and
the camera profiles and are not themselves recorded. When trackers and a triangulated pelvis both
exist, the skeleton root uses the measured position (provenance `measured`) instead of the
floor-contact guess. Camera-only setups still publish no skeleton pose.

**Why.** It follows ADR-005: recording inputs keeps sessions re-solvable, so a change to the
triangulation re-processes old recordings. A single code path means replay cannot differ from live
by construction.

**Rejected.** A separate triangulation task with its own output channel and recording; recording
the 3D landmarks; filtering or blending the camera root with the IMU estimate before real cameras
exist to tune it against.

**Revisit when** real cameras need temporal filtering or fusion stage 4's correction (ADR-011).

## ADR-017 — Camera views are combined only within a configured skew

**Decision.** A landmark is triangulated only from views that are fresh (within
`engine.stale_after_ms`) and whose frame time is within `triangulation.max_skew_ms` (default 20 ms)
of the newest camera frame. Views outside that window are left out, not interpolated. The skew,
the reprojection limit, the minimum view count and the minimum ray angle are configuration under
`triangulation:`.

**Why.** At a few metres from a moving subject, unsynchronised views place a landmark between
where the body was at two moments. Excluding them is visible and honest; interpolating hides it.
The default is a starting value, not a measured one.

**Rejected.** Combining the latest view of every camera regardless of age; interpolating 2D
landmarks to a common time.

**Revisit when** real cameras give measured skew and noise: set the defaults from data, and add a
`time_sync` profile if offsets between camera clocks are systematic.
