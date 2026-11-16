# 04 — Contracts

This is the approved contract between the engine and every client. Changing anything here needs
the architect's approval and a notice to both implementation agents.

Type shapes use TypeScript notation. After Stage 0 of the plan, `crates/kinexa-contracts` is the
implementation source of truth, `contracts/ts/` is generated from it, and `contracts/examples/`
holds golden messages that the Rust tests round-trip. If the crate and this document disagree,
that is a defect to raise, not a choice to make locally.

## 1. Boundaries and transports

| Boundary | Transport | Encoding | Status |
|---|---|---|---|
| Engine → desktop UI, VR client (streaming) | WebSocket, server push | JSON text frames | PoC |
| Client → engine (control, queries) | HTTP | JSON | PoC |
| Engine internal stages | Rust types over bounded channels, `Arc` for fan-out | None — no serialisation between stages | PoC |
| Recorded sessions | MCAP file | JSON messages | PoC |
| SlimeVR Server → engine | WebSocket, SolarXR | FlatBuffers | Designed |
| ESP32 tracker → engine | UDP | Binary, §7 | Designed |
| Engine → SteamVR | To be decided with hardware | — | Extension point |

No gRPC, MQTT or broker. JSON is used first because it is inspectable and both the browser and
Godot parse it natively. A binary encoding of the same messages is an additive option to be taken
only when a measurement shows JSON is the bottleneck (ADR-003).

## 2. Versioning and compatibility

- `protocol_version` is an integer, currently `1`, announced in `hello` and present in the URL
  (`/api/v1`).
- **Compatible changes** do not bump the version: adding a field, adding a message type, adding an
  enum value.
- **Clients must** ignore unknown fields, ignore unknown message types, and treat an unknown enum
  value as "other" rather than failing.
- **The engine** ignores unknown fields but is strict about values: an unknown enum value in a
  request is `invalid_request`, and a persisted file containing one is reported as `invalid`. The
  engine only ever reads requests, and files written by the same or an older build.
- Every identifier that can reach the filesystem (`SessionId`, `ProfileId`, `DeviceId`) is a
  validated type. Calibration profile ids, wherever they appear, are `ProfileId`.
- **Breaking changes** (removing or renaming a field, changing a unit, frame or meaning) bump the
  version and the URL prefix.
- A client that sees a `protocol_version` it does not support shows an explicit incompatibility
  error and does not render data.
- Persisted artefacts (config, calibration profiles, session metadata) each carry their own
  `schema_version`.

## 3. Shared types

```ts
type DeviceId  = string;   // ^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$   (no colon: ids become file names)
type SourceId  = string;   // same pattern
type SegmentId = string;   // ^[a-z][a-z0-9_]{0,63}$, defined by the active skeleton
type SessionId = string;   // ^[0-9]{8}T[0-9]{6}Z-[a-z0-9]{4}$
type ProfileId = string;   // ^[A-Za-z0-9][A-Za-z0-9_.-]{0,159}$, e.g. "mounting-SIM-T01-pelvis"
type TimeUs    = number;   // integer microseconds on the session clock
type Vec3      = [number, number, number];
type Quat      = [number, number, number, number];   // x, y, z, w

type Origin        = "live" | "simulated" | "fixture";
type EngineMode    = "live" | "replay";
type Provenance    = "measured" | "derived" | "inferred" | "unavailable";
type TrackingState = "disconnected" | "acquiring" | "tracking" | "degraded" | "unavailable";
type TimeQuality   = "synchronized" | "estimated" | "receive_time";

type FrameRef =
  | { type: "world" }
  | { type: "xr_stage" }
  | { type: "sensor"; id: DeviceId }
  | { type: "sensor_reference"; id: DeviceId }
  | { type: "camera"; id: DeviceId }
  | { type: "image"; id: DeviceId }
  | { type: "model"; id: DeviceId }
  | { type: "segment"; id: SegmentId };

interface Reason { code: string; subject: string | null; message: string }
```

Meanings are defined in [03](03-domain-model.md).

### Devices

```ts
type DeviceKind = "imu_tracker" | "camera";          // more kinds are added as adapters arrive
type Transport  = "simulated" | "file" | "udp" | "usb" | "websocket";

type SensorCapability =
  | "orientation" | "angular_velocity" | "acceleration" | "position" | "distance" | "contact"
  | "image" | "landmarks_2d" | "landmarks_3d_model"
  | "battery" | "signal_strength" | "quality";

interface LandmarkSchema { id: string; names: string[]; edges: [number, number][] }

interface CameraInfo {
  resolution_px: [number, number];
  landmark_schema: LandmarkSchema | null;
}

interface DeviceDescriptor {
  id: DeviceId;
  source_id: SourceId;
  kind: DeviceKind;
  origin: Origin;
  model: string | null;            // e.g. "ESP32-C3 + ICM-45686"
  firmware: string | null;
  transport: Transport;
  capabilities: SensorCapability[];
  nominal_rate_hz: number | null;
  camera: CameraInfo | null;       // present for cameras
}

interface StreamStatus {
  measured_rate_hz: number;
  last_seen_t_us: TimeUs | null;
  received: number;                // observations accepted
  dropped: number;                 // lost in transport (seq gaps) or dropped at a full queue
  time_quality: TimeQuality;
}

type CalibrationKind =
  | "imu_bias" | "mounting" | "heading_alignment" | "body_proportions"
  | "camera_intrinsics" | "camera_extrinsics" | "floor_plane"
  | "xr_alignment" | "time_sync";

type CalibrationState = "missing" | "valid" | "stale" | "assumed" | "invalid";

interface CalibrationRef { kind: CalibrationKind; state: CalibrationState; profile_id: string | null }

interface DeviceStatus {
  descriptor: DeviceDescriptor;
  state: TrackingState;
  reasons: Reason[];
  assignment: SegmentId | null;
  stream: StreamStatus;
  battery: { level: number; charging: boolean | null } | null;   // level 0..1
  signal: { rssi_dbm: number } | null;
  quality: number | null;                                        // device-reported, 0..1
  calibration: CalibrationRef[];                                 // the kinds relevant to this device
}
```

### Observations

Observations are the engine's input currency. They are recorded, and they are the unit of replay.
They are not streamed to clients in the PoC.

```ts
interface SensorObservation {
  device_id: DeviceId;
  source_id: SourceId;
  origin: Origin;
  seq: number;
  t_us: TimeUs;                    // capture instant, session clock
  device_time_us: number | null;   // device clock, if the device has one
  time_quality: TimeQuality;
  calibration_ref: string | null;  // profile already applied by the adapter, if any
  confidence: number | null;
  data: ObservationData;
}

type ObservationData =
  | { kind: "orientation";      frame: FrameRef; q: Quat }          // q_frame_from_sensor
  | { kind: "angular_velocity"; frame: FrameRef; rad_s: Vec3 }
  | { kind: "acceleration";     frame: FrameRef; m_s2: Vec3 }
  | { kind: "position";         frame: FrameRef; m: Vec3 }
  | { kind: "distance";         m: number }
  | { kind: "contact";          in_contact: boolean; force_n: number | null }
  | { kind: "landmarks_2d";     landmarks: PoseLandmarks2D }
  | { kind: "landmarks_3d";     landmarks: PoseLandmarks3D }
  | { kind: "diagnostics";      battery_level: number | null; charging: boolean | null;
                                rssi_dbm: number | null; quality: number | null };

type SourceEvent =
  | { type: "device_announced"; descriptor: DeviceDescriptor }
  | { type: "device_link";      device_id: DeviceId; connected: boolean; t_us: TimeUs }
  | { type: "observation";      observation: SensorObservation };
```

This satisfies the measurement rule: source, device, capture timestamp, frame, unit (in the field
name), calibration reference, measurement type, and confidence are all explicit.

### Landmarks

```ts
interface CameraFrameMetadata {
  device_id: DeviceId;
  frame_seq: number;
  t_us: TimeUs;
  device_time_us: number | null;
  time_quality: TimeQuality;
  image_size_px: [number, number];
}

interface Landmark2D { x_px: number; y_px: number; confidence: number }

interface PoseLandmarks2D {
  frame_meta: CameraFrameMetadata;
  frame: FrameRef;                 // always { type: "image", id }
  origin: Origin;
  schema_id: string;               // matches the camera's LandmarkSchema
  detector: string;                // e.g. "simulated-projection", later "rtmpose-m"
  points: (Landmark2D | null)[];   // index-aligned with the schema; null = not detected
}

type PoseLandmarks3D =
  | { kind: "model_relative";      // model-inferred. Not metric. Never placed in world
      frame_meta: CameraFrameMetadata; frame: FrameRef; origin: Origin;
      schema_id: string; detector: string;
      points: ({ p: Vec3; confidence: number } | null)[] }
  | { kind: "triangulated";        // metric, world frame
      t_us: TimeUs; frame: FrameRef; origin: Origin; schema_id: string;
      calibration_refs: string[];
      points: ({ m: Vec3; views: number; reprojection_error_px: number; confidence: number } | null)[] };
```

### Skeleton and pose

```ts
interface SegmentDef {
  id: SegmentId;
  parent: SegmentId | null;
  offset_m: Vec3;
  tail_m: Vec3;
  contact_points_m: Vec3[];
}

interface JointDef {
  id: string;
  label: string;
  segments: [SegmentId, SegmentId];
  method: "axis_angle" | "planar_angle";
  unit: "rad";
  validated: boolean;
}

interface SkeletonDefinition {
  model_id: string;                // "kinexa.body.v1"
  rev: number;                     // increments whenever the definition or its scaling changes
  height_m: number;
  proportions: CalibrationState;   // "assumed" until body-proportion calibration exists
  segments: SegmentDef[];          // parents precede children
  joints: JointDef[];
}

interface SegmentPose {
  rotation: Quat;                  // q_world_from_segment
  position_m: Vec3;                // segment origin in world, from forward kinematics
  provenance: Provenance;          // of the rotation
  confidence: number | null;
  sources: DeviceId[];
}

interface SkeletonPose {
  skeleton_rev: number;
  root_position_m: Vec3;
  root_position_provenance: Provenance;
  segments: SegmentPose[];         // index-aligned with SkeletonDefinition.segments
}

interface JointMeasurement {
  value: number | null;            // in JointDef.unit
  provenance: Provenance;
  confidence: number | null;
  reasons: Reason[];
}

interface TrackingQuality {
  state: TrackingState;
  observed_segments: number;       // segments whose rotation is measured or derived
  total_segments: number;
  reasons: Reason[];
}
```

### System state

```ts
type FeatureId =
  | "segment_orientation" | "skeleton_pose" | "joint_angles" | "root_position"
  | "landmarks_2d" | "landmarks_3d_triangulated" | "camera_imu_fusion" | "ground_contact"
  | "recording" | "playback";

interface FeatureStatus {
  id: FeatureId;
  state: "available" | "degraded" | "unavailable";
  reasons: Reason[];
}

interface CalibrationProfileSummary {
  id: string;
  kind: CalibrationKind;
  state: CalibrationState;
  device_id: DeviceId | null;
  segment_id: SegmentId | null;
  method: string;                  // e.g. "reference_pose", "declared_by_simulator", "default"
  created_utc: string | null;      // RFC 3339
}

interface RecordingSession {
  id: SessionId;
  name: string;
  created_utc: string;
  duration_us: number;
  size_bytes: number;
  origins: Origin[];
  device_count: number;
  engine_version: string;
  protocol_version: number;
}

interface RecordingState { session_id: SessionId; started_t_us: TimeUs; events: number; dropped_events: number }

interface PlaybackState {
  session: RecordingSession;
  paused: boolean;
  speed: number;
  position_us: number;             // 0 .. session.duration_us
  looping: boolean;
}

interface SimulationState {
  source_id: SourceId;
  motion: string;
  available_motions: { id: string; label: string }[];
  speed: number;
  paused: boolean;
}

interface SystemState {
  rev: number;                     // increments on every change
  engine: { version: string; protocol_version: number; mode: EngineMode; state: TrackingState; uptime_s: number };
  devices: DeviceStatus[];
  calibration: CalibrationProfileSummary[];
  features: FeatureStatus[];
  recording: RecordingState | null;
  playback: PlaybackState | null;
  simulation: SimulationState | null;   // null when no simulated source is configured
}
```

## 4. WebSocket stream

`GET /api/v1/stream?topics=frame,state,landmarks_2d&max_rate_hz=60`

- `topics` — comma-separated subset of `frame`, `state`, `landmarks_2d`. Default: `frame,state`.
- `max_rate_hz` — upper bound for `frame` delivery to this client. Default: the publish rate.
- Server push only. Clients send nothing but WebSocket control frames. All control goes over HTTP.
- Each text frame is one JSON object with a `type` field.

```ts
type ServerMessage =
  | { type: "hello";
      protocol_version: number;
      engine_version: string;
      session_epoch_utc: string;
      conventions: { handedness: "right"; up: "+y"; forward: "+z"; length_unit: "m";
                     angle_unit: "rad"; quaternion_order: "xyzw"; time_unit: "us" };
      skeleton: SkeletonDefinition;
      state: SystemState }
  | { type: "skeleton"; skeleton: SkeletonDefinition }      // sent when rev changes
  | { type: "state"; state: SystemState }                   // on change, and at 1 Hz
  | { type: "frame";
      seq: number;
      t_us: TimeUs;
      origins: Origin[];                                    // origins of every contributing source
      pose: SkeletonPose | null;
      joints: JointMeasurement[];                           // index-aligned with skeleton.joints
      quality: TrackingQuality }
  | { type: "landmarks_2d"; landmarks: PoseLandmarks2D }
  | { type: "notice"; level: "info" | "warning" | "error"; code: string; message: string };
```

### Sequence guarantees

- `hello` is always the first message on a connection.
- A `skeleton` message with a new `rev` is sent before the first `frame` that refers to it.
- `frame.seq` increases strictly. A client may miss frames; it never receives them out of order.
- Entering or leaving replay is announced by a `state` message before the first frame of the new
  mode. `frame.seq` continues to increase across the change.
- A slow client is never buffered for: it receives the most recent frame at its own pace.

### Example messages

Abbreviated; complete golden files are generated into `contracts/examples/`.

```json
{
  "type": "frame",
  "seq": 18230,
  "t_us": 303833333,
  "origins": ["simulated"],
  "pose": {
    "skeleton_rev": 1,
    "root_position_m": [0.0, 0.874, 0.0],
    "root_position_provenance": "inferred",
    "segments": [
      { "rotation": [0.0, 0.0, 0.0, 1.0], "position_m": [0.0, 0.874, 0.0],
        "provenance": "derived", "confidence": 0.97, "sources": ["SIM-T01"] },
      { "rotation": [0.0, 0.0, 0.0, 1.0], "position_m": [0.0, 0.979, 0.0],
        "provenance": "inferred", "confidence": null, "sources": [] }
    ]
  },
  "joints": [
    { "value": 1.318, "provenance": "derived", "confidence": 0.95, "reasons": [] },
    { "value": null, "provenance": "unavailable", "confidence": null,
      "reasons": [{ "code": "segment_not_observed", "subject": "right_shin",
                    "message": "Right shin has no tracker assigned." }] }
  ],
  "quality": {
    "state": "degraded", "observed_segments": 11, "total_segments": 19,
    "reasons": [{ "code": "device_disconnected", "subject": "SIM-T08",
                  "message": "Tracker SIM-T08 is disconnected." }]
  }
}
```

```json
{
  "type": "landmarks_2d",
  "landmarks": {
    "frame_meta": { "device_id": "SIM-CAM1", "frame_seq": 9114, "t_us": 303800000,
                    "device_time_us": 303712345, "time_quality": "estimated",
                    "image_size_px": [1280, 720] },
    "frame": { "type": "image", "id": "SIM-CAM1" },
    "origin": "simulated",
    "schema_id": "kinexa.body.v1.joints",
    "detector": "simulated-projection",
    "points": [{ "x_px": 640.2, "y_px": 402.7, "confidence": 0.98 }, null]
  }
}
```

```json
{
  "descriptor": {
    "id": "SIM-T01", "source_id": "sim", "kind": "imu_tracker", "origin": "simulated",
    "model": "Simulated IMU", "firmware": null, "transport": "simulated",
    "capabilities": ["orientation", "battery", "signal_strength", "quality"],
    "nominal_rate_hz": 100, "camera": null
  },
  "state": "tracking",
  "reasons": [{ "code": "heading_alignment_assumed", "subject": "SIM-T01",
                "message": "Heading alignment has not been calibrated; identity is assumed." }],
  "assignment": "pelvis",
  "stream": { "measured_rate_hz": 99.8, "last_seen_t_us": 303829000, "received": 30311,
              "dropped": 0, "time_quality": "estimated" },
  "battery": { "level": 0.82, "charging": false },
  "signal": { "rssi_dbm": -54 },
  "quality": 0.97,
  "calibration": [
    { "kind": "mounting", "state": "valid", "profile_id": "mounting-SIM-T01-pelvis" },
    { "kind": "heading_alignment", "state": "assumed", "profile_id": null }
  ]
}
```

## 5. HTTP API

Base: `http://127.0.0.1:7700/api/v1`. All bodies are JSON.

### Envelope

```ts
type ApiResponse<T> =
  | { ok: true;  data: T }
  | { ok: false; error: { code: ApiErrorCode; message: string; details: unknown | null } };

type ApiErrorCode =
  | "invalid_request"   // 400  malformed body, unknown segment, value out of range
  | "forbidden_origin"  // 403  Origin header not in the allow-list
  | "not_found"         // 404  unknown device, session or profile
  | "conflict"          // 409  not valid in the current mode, e.g. recording during replay
  | "unavailable"       // 503  dependency missing, e.g. data directory not writable
  | "internal";         // 500
```

Error messages are written for the user and never include paths outside the data directory, stack
traces or internal identifiers.

### Routes

| Method and path | Body | Returns | Notes |
|---|---|---|---|
| `GET /health` | — | `{ status: "ok" }` | Liveness only |
| `GET /state` | — | `SystemState` | Same object as the `state` message |
| `GET /skeleton` | — | `SkeletonDefinition` | |
| `PUT /devices/{id}/assignment` | `{ segment: SegmentId \| null }` | `DeviceStatus` | Marks dependent calibration `stale`. A segment may hold one orientation device; assigning a second returns `conflict` |
| `POST /calibration/reference-pose` | `{ device_ids: DeviceId[] \| null }` | `CalibrationProfileSummary[]` | Captures mounting for all assigned, tracking devices, or the listed ones. `conflict` in replay mode |
| `DELETE /calibration/{profile_id}` | — | `{ deleted: true }` | |
| `PUT /simulation` | `{ motion?: string; speed?: number; paused?: boolean }` | `SimulationState` | `not_found` if no simulated source |
| `POST /simulation/devices/{id}/fault` | `{ action: "disconnect" \| "reconnect" \| "degrade" \| "restore"; quality?: number }` | `DeviceStatus` | Manual counterpart of scripted scenario events |
| `GET /sessions` | — | `RecordingSession[]` | Newest first |
| `POST /recording/start` | `{ name?: string }` | `RecordingState` | `conflict` if already recording or in replay |
| `POST /recording/stop` | — | `RecordingSession` | |
| `POST /playback/start` | `{ session_id: SessionId; speed?: number; looping?: boolean }` | `PlaybackState` | Switches the engine to replay mode |
| `PUT /playback` | `{ paused?: boolean; speed?: number; position_us?: number }` | `PlaybackState` | |
| `POST /playback/stop` | — | `{ mode: "live" }` | Returns to live mode |

Every state-changing route results in a `state` message on the stream, so clients update from the
stream and treat the HTTP response as an acknowledgement.

### Local security

The engine has no authentication, so its exposure is limited instead:

- It binds to `127.0.0.1` by default. Binding to another address is an explicit configuration
  choice and logs a warning.
- Requests carrying an `Origin` header are accepted only if the origin is in
  `api.allowed_origins`. This covers both CORS and the WebSocket handshake, and stops an arbitrary
  web page from driving a local engine. Requests without an `Origin` (native clients such as
  Godot or curl) are accepted.
- State-changing routes require `Content-Type: application/json`.
- Path parameters are validated against the identifier patterns in §3 before use. A `SessionId`
  is never joined into a filesystem path without passing its pattern.
- Request bodies have a small size limit.

## 6. Configuration

One YAML file, validated at startup. The engine refuses to start on an invalid file and prints
every problem with its path.

```yaml
schema_version: 1

engine:
  publish_rate_hz: 60          # skeleton solve and publication rate
  stale_after_ms: 250          # an older observation no longer drives its segment
  data_dir: ./data             # calibration, sessions, runtime state

api:
  bind: 127.0.0.1:7700
  allowed_origins:
    - http://localhost:5173
    - http://127.0.0.1:5173
    - tauri://localhost
    - http://tauri.localhost

logging:
  level: info                  # trace | debug | info | warn | error
  format: pretty               # pretty | json

skeleton:
  model: kinexa.body.v1
  height_m: 1.75

# Initial device → segment assignments. Changes made at runtime are stored in
# <data_dir>/state/assignments.yaml and take precedence.
assignments:
  SIM-T01: pelvis
  SIM-T02: chest
  SIM-T03: left_thigh
  SIM-T04: left_shin

quality:
  degraded_rate_fraction: 0.7  # measured rate below this fraction of nominal → degraded
  degraded_quality: 0.5        # device-reported quality below this → degraded
  disconnect_after_ms: 1000    # no data for this long → disconnected

sources:
  - id: sim
    kind: simulated
    enabled: true
    seed: 42                   # mounting rotations and noise are derived from this
    motion: standing           # reference_pose | standing | squat | knee_flexion | arm_raise | front_kick
    speed: 1.0
    trackers:
      - { id: SIM-T01, rate_hz: 100, noise_deg: 0.2, heading_offset_deg: 0 }
      - { id: SIM-T02, rate_hz: 100, noise_deg: 0.2, heading_offset_deg: 0 }
    cameras:
      - id: SIM-CAM1
        rate_hz: 30
        resolution_px: [1280, 720]
        fov_y_deg: 50
        position_m: [0.0, 1.2, 3.2]
        look_at_m: [0.0, 1.0, 0.0]
        noise_px: 1.5
    scenario_period_s: 60      # optional; the scenario repeats with this period
    scenario:
      - { at_s: 20, device: SIM-T04, action: disconnect }
      - { at_s: 26, device: SIM-T04, action: reconnect }
      - { at_s: 34, device: SIM-T03, action: degrade, quality: 0.3 }
      - { at_s: 42, device: SIM-T03, action: restore }
```

Tracker and camera lists may be empty. Nothing in the engine or the clients depends on their
length.

### Validation rules

| Rule | Example error |
|---|---|
| `schema_version` is supported | `schema_version: 2 is not supported by this build (supports 1)` |
| Unknown fields are rejected | `sources[0].trackers[1].rate: unknown field, did you mean rate_hz?` |
| Identifiers match their pattern and are unique across all sources | `sources[0].cameras[0].id: "SIM-T01" is already used by a tracker` |
| Rates, sizes and durations are positive and within sane bounds | `engine.publish_rate_hz: must be between 1 and 1000` |
| Assigned segments exist in the selected skeleton | `assignments.SIM-T03: "left_tigh" is not a segment of kinexa.body.v1` |
| No two orientation devices share a segment | `assignments: SIM-T03 and SIM-T09 are both assigned to left_thigh` |
| Assignment keys refer to configured devices | Warning only: a device may appear later |
| `api.bind` parses; non-loopback binds warn | `api.bind: 0.0.0.0:7700 exposes the engine to the network` |
| Scenario devices exist; times are non-negative and ordered | `sources[0].scenario[2].device: "SIM-T99" is not defined in this source` |
| `data_dir` exists or can be created, and is writable | `engine.data_dir: ./data is not writable: permission denied` |

Overrides: `KINEXA_CONFIG` selects the file when `--config` is absent, `KINEXA_LOG` overrides the
log level. There are no secrets in configuration.

### What is not in YAML

| Data | Where |
|---|---|
| Calibration profiles | `<data_dir>/calibration/*.json` |
| Runtime assignment changes | `<data_dir>/state/assignments.yaml` |
| Session metadata | `<data_dir>/sessions/<id>/session.json` |
| Recorded streams | `<data_dir>/sessions/<id>/recording.mcap` |

### Designed source kinds

These shapes are reserved so the configuration model does not need to change when the adapters
are written. They are rejected with `not implemented in this build` until then.

```yaml
sources:
  - id: slime
    kind: slimevr_solarxr
    url: ws://127.0.0.1:21110
  - id: trackers
    kind: esp32_udp
    bind: 0.0.0.0:6970
  - id: cam-left
    kind: camera
    device: "USB Camera (046d:085c)"
    resolution_px: [1280, 720]
    rate_hz: 60
    processing_profile: body-fast

processing_profiles:
  body-fast:
    detector: rtmpose-m
    model_path: models/rtmpose-m.onnx
    execution: cuda            # cuda | directml | cpu
    max_inference_hz: 30
```

## 7. ESP32 telemetry (designed, draft 0)

Recorded here so custom firmware and the engine adapter start from the same contract. It is not
implemented and will be finalised with the firmware milestone.

- **Transport**: UDP, little-endian, one datagram per message, no fragmentation.
- **Discovery**: the tracker broadcasts `HELLO` until the engine answers with `CONFIG`.
- **Clock sync**: periodic `SYNC` exchange carrying four timestamps (request sent, request
  received, reply sent, reply received). This yields offset and round-trip time and upgrades
  `time_quality` to `synchronized`.
- **Loss**: `seq` per device; the engine counts gaps and never asks for retransmission.

Common header, 20 bytes:

| Offset | Size | Field |
|---|---|---|
| 0 | 2 | Magic `0x4B58` ("KX") |
| 2 | 1 | Protocol version |
| 3 | 1 | Message type |
| 4 | 6 | Device id (MAC) |
| 10 | 2 | Flags |
| 12 | 4 | `seq` |
| 16 | 4 | Payload length |

| Type | Direction | Payload |
|---|---|---|
| `HELLO` | Tracker → engine | Firmware version, board and IMU model, capability bitmask, supported rates |
| `CONFIG` | Engine → tracker | Output rate, enabled streams |
| `IMU` | Tracker → engine | `device_time_us` u64, orientation as four f32 (`q_sensor_reference_from_sensor`, x y z w), optional gyro and accelerometer as three f32 each, quality u8 |
| `STATUS` | Tracker → engine | Battery millivolts, charge state, RSSI, temperature |
| `SYNC` | Both | The four timestamps |
| `AUX` | Tracker → engine | Tagged extra sensors: distance, pressure, contact |

The adapter converts the IMU's native axes into Kinexa's sensor conventions before emitting an
observation. Firmware does not need to know the world convention.

Before this is built, real trackers arrive through SlimeVR Server (ADR-008), which already
supports the ESP8266 + LSM6DSR and ESP32-C3 + ICM-45686 boards when they run SlimeVR firmware.
