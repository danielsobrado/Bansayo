# Bansayo Phase 01 — OpenXR observation pipeline (implementation contract)

**Status:** Ready for implementation; specification only.  
**Date:** 2026-11-20.  
**Owner repository:** [danielsobrado/Bansayo](https://github.com/danielsobrado/Bansayo), branch **main**.  
**Verified target baseline:** [b348d89016a5cec7ecf6a6c536f2ff44d0e25c90](https://github.com/danielsobrado/Bansayo/commit/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90).  
**Reference repository:** [maceq687/FullBodyPoseEstimation](https://github.com/maceq687/FullBodyPoseEstimation).  
**Verified reference baseline:** [be895709c0bf86d35f64eb2cd5d2ffb4c6838593](https://github.com/maceq687/FullBodyPoseEstimation/commit/be895709c0bf86d35f64eb2cd5d2ffb4c6838593).  
**Phase boundaries:** Phase 01 in the 2026-11-20 Bansayo roadmap, **not** historical Stage/Phase 1 in docs/13-implementation-status.md.  
**Implementation method:** One implementation AI, small sequential changes, commit directly to **main** after each passing gate. Never force-push.  
**Deliverable:** The Godot OpenXR client publishes three independent, timestamped and validated 6-DoF device observations to the Rust engine. The engine records, replays and reports those observations, **without** using them to alter the human skeleton yet.

> **Mandatory refresh gate:** Before editing, fetch current main, record its new SHA, and diff every listed target file against the pinned baseline. The paths and line references below were verified at the pinned commit, **not guaranteed at a later main**. Preserve subsequent functionality. If a file was significantly refactored or this feature was implemented already, update the source map and adapt the implementation before changing anything. Never revert unrelated commits.

## 0. Purpose and acceptance boundary

The upstream Unity project reads headset and two controller poses in its avatar update loop. Bansayo already has an OpenXR VR runtime and a robust Rust observation/recording pipeline, but currently does not upload the VR device poses as source observations. This phase joins **input capture and ingestion only**.

When complete:

1. In headset mode, Bansayo samples a head pose and **grip** poses for the left and right controllers from OpenXR.
2. All three poses are expressed in **XR stage space**, metres, right-handed, +Y up, and transmitted as xyzw Hamilton quaternions.
3. Rust independently validates, timestamps on its session clock, records, replays and exposes the three XR devices in normal device status.
4. A tracked-to-untracked transition is observable. Disconnect/reconnect does not reuse stale observations or incorrectly reject reset sequence numbers.
5. Desktop mode, all prior devices, camera-only pose, SlimeVR inputs, protocol-v1 downlink and current avatar behavior are unchanged.
6. No camera-to-XR world transform, anthropometric wrist offsets, elbow estimation, avatar IK, foot landmarks or calibrated fusion is introduced here.

**STOP:** Phase 01 is **not** complete just because JSON reaches a WebSocket handler. Validate the entire adapter → engine event → registry → recording/replay path.

## 1. Proven implementation reference map — actual source, not guessed paths

All links in this section point to immutable commit snapshots. Line numbers are valid for those snapshots.

| Topic | Upstream source and observed behavior | Verified Bansayo target and precise adaptation |
|---|---|---|
| XR pose acquisition | [VNectModel.cs L122-L132](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L122-L132) holds Unity XR input devices and poses; [L169-L183](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L169-L183) reads device position and rotation. | [apps/vr/scripts/training_room.gd L331-L377](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/apps/vr/scripts/training_room.gd#L331-L377) creates XR origin, camera, controllers; add a dedicated capture/send component, never place acquisition inside avatar_renderer.gd. |
| Controller action poses | Upstream reads generic Unity controller device transforms without distinguishing aim/grip. | [training_room.gd L350-L357](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/apps/vr/scripts/training_room.gd#L350-L357) explicitly selects **aim_pose** for UI pointing; [openxr_action_map.tres L115-L124](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/apps/vr/openxr_action_map.tres#L115-L124) already declares aim_pose and grip_pose. Add separate invisible grip-pose controller nodes. Do **not** change UI aim nodes. |
| Reference's shortcut **not** to copy | [VNectModel.cs L188-L214](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L188-L214) offsets controller positions from HMD/Nose, applies fixed Euler wrist rotations, and computes phantom elbows. | In Phase 01 store **raw grip tracked-device pose only**. No uncalibrated controller→wrist conversion, fixed Euler offsets, phantom elbow, or direct skeleton assignment. Those belong to Phases 03–04. |
| XR lifecycle | Unity reacquires XR device handles during initialization at [VNectModel.cs L228-L236](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L228-L236). | [training_room.gd L379-L408](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/apps/vr/scripts/training_room.gd#L379-L408) handles focus loss, XR runtime stopping, recentering and desktop fallback. Hook sender lifecycle here; do not create a second XR session. |
| Existing outgoing stream | No matching engine-owned typed stream in upstream. | [apps/vr/scripts/engine_client.gd L25-L108](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/apps/vr/scripts/engine_client.gd#L25-L108) is a **receive-only** protocol-v1 WebSocket. Keep it untouched; create a separate uplink with matching URL and reconnect conventions. |
| Existing action API | No matching architecture upstream. | [apps/vr/scripts/engine_actions.gd L35-L58](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/apps/vr/scripts/engine_actions.gd#L35-L58) performs infrequent HTTP controls. **Do not** send 60-Hz telemetry with HTTPRequest. |
| Input wire contract | Unity directly mutates its avatar. | [crates/bansayo-contracts/src/observation.rs L12-L79](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-contracts/src/observation.rs#L12-L79) defines SensorObservation, ObservationData and SourceEvent. Add **one atomic RigidPose observation kind**. |
| Frame and time | Upstream ties positions to Unity avatar space. | [frames.rs L9-L35](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-contracts/src/frames.rs#L9-L35) already has FrameRef::XrStage and Quat xyzw; [time.rs L8-L21](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-contracts/src/time.rs#L8-L21) already has TimeQuality::ReceiveTime. **Reuse**, do not define new coordinate conventions. |
| Device metadata | Unity uses controller handle identity. | [device.rs L12-L89](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-contracts/src/device.rs#L12-L89) defines DeviceKind, Transport, SensorCapability and DeviceDescriptor. Extend enums additively, announce three explicit XR devices at startup only if configured. |
| Input validation | Unity checks hmdDevice.isValid but not each TryGetFeatureValue result, [VNectModel.cs L169-L183](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L169-L183). | [sources/validation.rs L30-L128](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/sources/validation.rs#L30-L128) checks identity, frame, capability and finite values. Add strict XR rigid-pose validation. |
| Engine ingress | Unity has no corresponding engine. | [api/mod.rs L45-L63](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/api/mod.rs#L45-L63) defines routes including camera websocket ingestion; [L384-L422](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/api/mod.rs#L384-L422) demonstrates cancellable websocket receive loop. Add **GET /api/v1/ingest/xr** with a separate typed handler. |
| Engine event queue | Not applicable upstream. | [app.rs L63-L80](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/app.rs#L63-L80) creates bounded ingest queue and descriptors; [L162-L163](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/app.rs#L162-L163) constructs Registry; [L255-L276](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/app.rs#L255-L276) constructs API. Reuse existing mpsc::Sender<Ingest>, SessionClock and drops. |
| Acceptance and replay | Upstream does not have Bansayo's recording contract. | [runtime.rs L572-L614](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/pipeline/runtime.rs#L572-L614) validates, accepts, records source events; [registry/mod.rs L12-L38, L112-L207](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/registry/mod.rs#L12-L207) selects observation kinds and enforces sequence order. Do **not** bypass either. |
| Configuration | Unity scene settings are not Bansayo config. | [config/schema.rs L14-L75](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/config/schema.rs#L14-L75), [validate.rs L293-L330](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/config/validate.rs#L293-L330). Add typed **xr** YAML section with explicit bounds and off-by-default behavior. |
| Licence | [FullBodyPoseEstimation LICENSE](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/LICENSE) states GPL-3.0. | Use the **idea** of tracking XR devices, not any upstream code or assets. Bansayo's implementation must be independent. |

**Verified existing behavior:** Rust engine solver currently consumes assigned IMU orientations and camera triangulation, not XR devices ([pipeline/solve.rs L1-L130](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/pipeline/solve.rs#L1-L130)). Do not modify that solver this phase.

## 2. Fixed architecture and hard constraints

### 2.1 Data flow

~~~text
Godot OpenXR runtime
  -> XRCamera3D (head) + two hidden XRController3D grip_pose nodes
  -> xr_pose_capture.gd (sample + stage coordinates + tracking flags)
  -> xr_pose_uplink.gd (one JSON frame per sample tick, WebSocketPeer)
  -> GET /api/v1/ingest/xr (Axum WebSocket, loopback-only)
  -> api/xr.rs (protocol/auth/session/rate checks)
  -> sources/xr.rs (typed validation + SourceEvent translation)
  -> existing bounded mpsc::Sender<Ingest>
  -> pipeline/runtime.rs::accept
  -> registry/mod.rs::Registry
  -> existing state reporting / MCAP recording / replay
  X -> pipeline/solve.rs  [NO XR integration in Phase 01]
  X -> avatar_renderer.gd  [NO avatar changes in Phase 01]
~~~

### 2.2 Fixed choices

- **Transport:** one long-lived JSON text WebSocket, **GET /api/v1/ingest/xr**. Do not use HTTP POST per frame, MQTT, gRPC, OSC, or the existing downlink /api/v1/stream.
- **Protocol:** independent typed XR uplink envelope inside **API v1**, additive to existing contracts. Do **not** increment protocol version or change existing ServerMessage payloads.
- **Devices:** exactly three reserved IDs, **XR-HMD**, **XR-LEFT-GRIP**, **XR-RIGHT-GRIP**, with source ID **xr-openxr**. Roles are **head**, **left_grip**, **right_grip**; order is fixed and validated.
- **Source registration:** register the three descriptors in app.rs before Registry::new only when xr.enabled is true. Do not issue unbounded dynamic DeviceAnnounced events for arbitrary user-supplied IDs.
- **Single writer:** only one XR uplink websocket may be active. A concurrent second connection receives HTTP **409** before WebSocket upgrade.
- **Local-only for this phase:** only a verified loopback peer is allowed. A remote peer receives HTTP **403** even when LAN pairing is enabled. No new authentication bypass. Future remote XR is a separate change.
- **Coordinate frame:** FrameRef::XrStage. Do not apply T_world_from_xr, BodyRoot alignment, floor offsets, or camera transforms in Phase 01.
- **Controller point:** OpenXR **grip_pose**, not UI **aim_pose**. Never call the grip origin an anatomical wrist.
- **Time:** map the complete incoming frame to **one server receive timestamp** using SessionClock::now_us(). time_quality is **receive_time**. Preserve the Godot monotonic client_time_us as device_time_us; do **not** pretend clocks are synchronized.
- **Origin:** Origin::Live for all three; missing tracking is a link/unavailable event, not a fake zero pose.
- **Atomicity:** position and rotation from one tracking node are a **single** ObservationData::RigidPose. Different devices have independent events and may drop independently under queue pressure.
- **Rate:** Godot target rate 60 Hz, Rust acceptance ceiling 120 frames/s. No extrapolation. Runtime may deliver fewer actual frames; measure rather than assume.
- **Config:** engine parameters live in typed YAML; Godot-specific user preferences live in project.godot as already established by the VR client. Do not introduce a YAML parser dependency in Godot solely for Phase 01.
- **Feature flags:** engine xr.enabled defaults **false**; Godot bansayo/xr_uplink_enabled defaults **false**. Both must be enabled for actual ingest.
- **Recording:** accepted XR observations flow through the existing event recorder. Replay produces the same observations, but must not consult the current XR session.
- **Failure mode:** reject invalid frames and log counters. Never substitute identity quaternion or zero vectors for absent tracking.

### 2.3 Coordinate conversion, explicitly

In [training_room.gd](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/apps/vr/scripts/training_room.gd#L331-L348), XROrigin3D is positioned at **(0,0,3)** for presentation. Therefore **global_position** is **not** the raw stage position.

Use the full rigid transform:

~~~gdscript
var stage_from_world_node: Transform3D = xr_origin.global_transform.affine_inverse()
var stage_from_device: Transform3D = stage_from_world_node * tracked_node.global_transform
var p: Vector3 = stage_from_device.origin
var q: Quaternion = stage_from_device.basis.orthonormalized().get_rotation_quaternion()
var position_m := [p.x, p.y, p.z]
var rotation_xyzw := [q.x, q.y, q.z, q.w]
~~~

This is the required calculation, not a code fragment to paste without validation. For a head at world position (0,1.7,3), with the origin at (0,0,3), the stage position must be (0,1.7,0). Tests must also cover nonzero origin yaw. **Do not use BodyRoot's** T_xr_stage_from_world: it is the *camera-world alignment* transform for a later phase ([body_root.gd](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/apps/vr/scripts/body_root.gd)).

The transmitting node's rotation is **q_xr_stage_from_device**. Hamilton quaternion component order **[x,y,z,w]**. Preserve the sign: q and -q represent the same orientation, but choose deterministic continuity when testing. Do not flip axes to imitate Unity.

## 3. Exact new wire contract

Create **crates/bansayo-contracts/src/xr.rs**. This file owns the XR uplink messages; it must derive serde Serialize/Deserialize and ts_rs::TS. No loose serde_json::Value in the handler after decode.

~~~rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum XrRole {
    Head,
    LeftGrip,
    RightGrip,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct XrDeviceSample {
    pub role: XrRole,
    pub tracked: bool,
    pub position_m: Option<Vec3>,
    pub rotation_xyzw: Option<Quat>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum XrClientMessage {
    Hello { protocol_version: u32 },
    Frame {
        seq: u64,
        client_time_us: u64,
        samples: [XrDeviceSample; 3],
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum XrServerMessage {
    Ready {
        protocol_version: u32,
        time_quality: TimeQuality,
    },
    Error { code: String, message: String },
}
~~~

**Wire examples.** Client sends Hello exactly once after connection:

~~~json
{"type":"hello","protocol_version":1}
~~~

Server confirms:

~~~json
{"type":"ready","protocol_version":1,"time_quality":"receive_time"}
~~~

Then one sample packet per Godot tick (values illustrative, not recorded hardware measurements):

~~~json
{
  "type": "frame",
  "seq": 42,
  "client_time_us": 123456789,
  "samples": [
    {"role":"head","tracked":true,"position_m":[0.0,1.72,0.0],"rotation_xyzw":[0.0,0.0,0.0,1.0]},
    {"role":"left_grip","tracked":true,"position_m":[-0.35,1.25,-0.22],"rotation_xyzw":[0.0,0.0,0.0,1.0]},
    {"role":"right_grip","tracked":false,"position_m":null,"rotation_xyzw":null}
  ]
}
~~~

**Protocol invariants — all mandatory:**

1. Hello must be the first text message, match PROTOCOL_VERSION=1, and be followed by Ready before frames are sent. Frames before Hello: WebSocket close with **1002**.
2. Exactly three entries in the array, in order **head → left_grip → right_grip**. Duplicate role, unknown role, extra role or incorrect order: reject the frame; no partial event emission for that frame.
3. If tracked=true, both position_m and rotation_xyzw must exist; if tracked=false, both must be null. Reject other combinations.
4. All numeric values must be finite. Position vector Euclidean norm at most **20.0 m**; rotation quaternion norm within **0.005** of 1.0, and strictly nonzero. Reject rather than silently normalize impossible input.
5. seq strictly increases for each WebSocket connection starting at **0**; client_time_us is monotonic nondecreasing within the connection. Seq reset is permitted only after a genuinely new accepted WebSocket session.
6. Only text frames are supported. Binary frame: close with **1003**. Frame/message size at most **4096 bytes**; oversized frame closes with **1009**.
7. Server samples one **t_receive_us** when the complete frame arrives. Each tracked sample becomes one SensorObservation:
   - device_id fixed by role;
   - source_id **xr-openxr**;
   - origin **live**;
   - seq equal to the frame seq;
   - t_us equal to server **t_receive_us**;
   - device_time_us equal to incoming **client_time_us**;
   - time_quality **receive_time**;
   - calibration_ref **null**;
   - confidence **null** (tracking-validity does not quantify measurement accuracy);
   - data **{"kind":"rigid_pose","frame":{"type":"xr_stage"},"position_m":[...],"rotation":[x,y,z,w]}**.
8. For tracked=false, send a SourceEvent::DeviceLink with connected=false once on state transition; never emit an Observation with a zero pose. On reacquisition send connected=true before the next observation.
9. On WebSocket close or XR shutdown, mark any still-tracked device disconnected. On the next accepted connection, send reconnect DeviceLink events **before** observations so Registry clears old sequence state.
10. No client-supplied device ID, arbitrary frame name, world transform or origin field exists. The **server** assigns these identifiers and metadata.

**Contract example for Rust-generated SensorObservation:**

~~~json
{
  "device_id": "XR-HMD",
  "source_id": "xr-openxr",
  "origin": "live",
  "seq": 42,
  "t_us": 4567890,
  "device_time_us": 123456789,
  "time_quality": "receive_time",
  "calibration_ref": null,
  "confidence": null,
  "data": {
    "kind": "rigid_pose",
    "frame": {"type": "xr_stage"},
    "position_m": [0.0, 1.72, 0.0],
    "rotation": [0.0, 0.0, 0.0, 1.0]
  }
}
~~~

Add to **ObservationData** in observation.rs:

~~~rust
/// Atomic 6-DoF device pose. rotation = q_frame_from_device.
RigidPose {
    frame: FrameRef,
    position_m: Vec3,
    rotation: Quat,
},
~~~

Add **SensorCapability::RigidPose** in device.rs, **DeviceKind::XrTrackedDevice** in device.rs, **ObservationKind::RigidPose** in registry/mod.rs. Reuse existing **Transport::Websocket**, **FrameRef::XrStage**, **TimeQuality::ReceiveTime**, **Origin::Live**, **Vec3**, **Quat**, **DeviceId**, **SourceId**. Do not add a fake DeviceKind::Controller that could be confused with anatomical segment assignment.

Update **crates/bansayo-contracts/src/lib.rs** to export xr types. Update **crates/bansayo-contracts/tests/support/generated.rs** to visit XrClientMessage and XrServerMessage in the TypeScript exporter. Update **crates/bansayo-contracts/tests/support/examples.rs** with at least Hello, Ready, full Frame, partially untracked Frame and RigidPose examples. Regenerate generated contracts via the existing **BANSAYO_BLESS=1** mechanism; never hand-edit generated files.

## 4. Configurations — exact defaults and validation

### 4.1 Rust engine YAML

Add to **Config** in config/schema.rs:

~~~rust
#[serde(default)]
pub xr: XrConfig,
~~~

Create **XrConfig** in the same file with serde deny_unknown_fields/default and fields:

| YAML key | Rust type | Default | Validation |
|---|---|---|---|
| xr.enabled | bool | false | Off disables descriptors and route admission |
| xr.nominal_rate_hz | f64 | 60.0 | finite, 1.0–120.0 |
| xr.max_ingest_rate_hz | f64 | 120.0 | finite, >= nominal_rate_hz, <= 240.0 |
| xr.max_position_norm_m | f32 | 20.0 | finite, 1.0–50.0 |
| xr.max_quaternion_norm_error | f32 | 0.005 | finite, greater than 0, <= 0.02 |
| xr.max_message_bytes | usize | 4096 | 512–16384 |
| xr.tracking_stale_after_ms | u64 | 250 | 50–2000 |

**Validated phase-01 example, to create at config/xr-local.example.yaml:**

~~~yaml
schema_version: 2
engine:
  publish_rate_hz: 60
  stale_after_ms: 250
  data_dir: ./data
api:
  bind: 127.0.0.1:7700
xr:
  enabled: true
  nominal_rate_hz: 60.0
  max_ingest_rate_hz: 120.0
  max_position_norm_m: 20.0
  max_quaternion_norm_error: 0.005
  max_message_bytes: 4096
  tracking_stale_after_ms: 250
sources: []
~~~

- Add checks to **config/validate.rs::validate** and emit errors prefixed **xr.** for invalid fields; preserve schema v1/v2 support and current Config::deny_unknown_fields behavior.
- A nonempty **xr** section requires **schema_version: 2**, matching the pattern for newer sections; a default/absent xr field still works with schema 1. Enforce this explicitly in validation, not by rejecting all older YAML.
- The checked-in example is local-only and intentionally contains no simulator/camera input. Camera and IMU inputs are compatible but not mandatory to test XR ingest.
- Do not modify other example YAML files to enable XR implicitly.
- Do not expand api.bind or bypass the existing LAN middleware. A remote XR client is out of scope.

### 4.2 Godot project settings

Modify **apps/vr/project.godot**, adding in the existing **[bansayo]** section:

~~~ini
xr_uplink_enabled=false
xr_uplink_rate_hz=60
xr_uplink_max_buffered_bytes=8192
xr_uplink_reconnect_initial_ms=500
xr_uplink_reconnect_max_ms=8000
~~~

This is Godot's native project configuration; the engine continues to use **YAML**. Reuse **BANSAYO_ENGINE_URL** and **bansayo/engine_url** resolution from [engine_client.gd L25-L39](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/apps/vr/scripts/engine_client.gd#L25-L39). When enabled, the sender uses the same base URL with path /api/v1/ingest/xr, converts http→ws or https→wss, and never embeds a LAN token in URLs.

Set xr_uplink_enabled=true **only** in local development configuration for hardware testing, not in the committed default. The sender must log a single clear disabled/no-headset status, not print it every frame.

## 5. File-by-file implementation manifest

All paths below are repository-relative. **New** means the file did not exist at the verified target baseline. Keep each new file focused and small. If current main added an equivalent module, update the plan against that actual code instead of creating a duplicate.

| Action | File | Exact responsibility |
|---|---|---|
| **New** | crates/bansayo-contracts/src/xr.rs | Four typed XR protocol types; strict serde field/role rules, no engine dependencies |
| **Edit** | crates/bansayo-contracts/src/lib.rs | Export xr module/types |
| **Edit** | crates/bansayo-contracts/src/observation.rs | Add atomic ObservationData::RigidPose |
| **Edit** | crates/bansayo-contracts/src/device.rs | Add DeviceKind::XrTrackedDevice and SensorCapability::RigidPose |
| **Edit** | crates/bansayo-contracts/tests/contracts.rs | XR example round trips, unknown enum rejection, legacy compatibility |
| **Edit** | crates/bansayo-contracts/tests/support/examples.rs | Golden XR Hello/Frame/Ready/observation examples |
| **Edit** | crates/bansayo-contracts/tests/support/generated.rs | Add TS generation entry points |
| **Generated** | contracts/ts/* and contracts/examples/* | Update using BANSAYO_BLESS, then check clean generation |
| **Edit** | crates/bansayo-engine/src/config/schema.rs | Typed off-by-default XrConfig + Config.xr |
| **Edit** | crates/bansayo-engine/src/config/validate.rs | Validate all XR fields and schema-version gating |
| **Edit** | crates/bansayo-engine/src/sources/mod.rs | Export source module **xr** |
| **New** | crates/bansayo-engine/src/sources/xr.rs | Reserved device descriptors, constant IDs, frame sample validation, SourceEvent conversion |
| **Edit** | crates/bansayo-engine/src/sources/validation.rs | Permit RigidPose **only** for XR-tracked descriptors in XrStage; finite/norm checks |
| **Edit** | crates/bansayo-engine/src/registry/mod.rs | ObservationKind::RigidPose; rate accounting; handling of reconnect and tracking loss |
| **New** | crates/bansayo-engine/src/api/xr.rs | WebSocket handler; local peer gate, single writer, protocol parsing, rate/backpressure, shutdown |
| **Edit** | crates/bansayo-engine/src/api/mod.rs | mod xr; Api ingress context; /api/v1/ingest/xr route |
| **Edit** | crates/bansayo-engine/src/app.rs | Pre-register XR descriptors; add drops; construct XR ingress with existing clock/event sender/cancel |
| **Edit** | crates/bansayo-engine/tests/recording.rs | XR recording/replay regression or separate focused test |
| **New** | crates/bansayo-engine/tests/xr_ingest.rs | Actual network ingress integration tests using tokio-tungstenite |
| **New** | config/xr-local.example.yaml | Explicit, minimal opt-in example |
| **New** | apps/vr/scripts/xr_pose_capture.gd | Pure node→stage transform builder; finite/norm checks; no networking |
| **New** | apps/vr/scripts/xr_pose_uplink.gd | WebSocket lifecycle, Hello/Ready, rate limiting, outbound backpressure |
| **Edit** | apps/vr/scripts/training_room.gd | Construct two extra grip nodes and publisher; connect XR lifecycle |
| **Edit** | apps/vr/project.godot | Disabled defaults / sender options |
| **New** | apps/vr/tests/xr_observation_smoke.gd | Headless pure coordinate and serialization tests |
| **Edit** | apps/vr/README.md | Run instructions, diagnostics, known limitations and commands |
| **Edit** | docs/13-implementation-status.md | Phase 01 verification record, explicitly distinct from earlier Phase 1 |

### 5.1 Contract changes (step A)

Implement enum additions exactly as specified in §3. Adjust exhaustive matches in compiler-reported sites; do not add an empty wildcard arm. In particular:

- **registry/mod.rs::ObservationKind::of** must map RigidPose.
- **registry/mod.rs::Registry::accept** must count **RigidPose** as the XR descriptor's primary stream, so status shows about 60 Hz, not 0 Hz or the sum of three devices.
- **sources/validation.rs::observation** must validate:
  - the descriptor's **DeviceKind::XrTrackedDevice**;
  - frame == **FrameRef::XrStage**;
  - position and quaternion finite;
  - rotation norm within config tolerance;
  - declared SensorCapability::RigidPose.
- Existing IMU **Orientation { frame: SensorReference }** checks remain unchanged; never loosen them to accept XR.
- The engine must apply the configured XR norm/tolerance bounds in its XR adapter **before** the generic pipeline validator, whose signature currently does not accept config.
- Do not repurpose OrientationData::Position + Orientation into two events: they would not be atomic and risk mixed-timestamp pose pairs.
- Keep **ServerMessage**, SkeletonPose, MotionSample and all protocol-v1 downlink payload shapes unchanged.

### 5.2 Engine source adapter (step B)

**sources/xr.rs** defines immutable mappings:

| XrRole | DeviceId | Display label/model | Capability |
|---|---|---|---|
| Head | XR-HMD | OpenXR HMD | RigidPose |
| LeftGrip | XR-LEFT-GRIP | OpenXR left controller grip | RigidPose |
| RightGrip | XR-RIGHT-GRIP | OpenXR right controller grip | RigidPose |

All descriptors: source_id=xr-openxr, kind=xr_tracked_device, origin=live, transport=websocket, camera=None, firmware=None, nominal_rate_hz=xr.nominal_rate_hz. They are created **at engine startup**, not from client-controlled JSON.

Expose small pure functions (exact public names):

~~~rust
pub fn descriptors(config: &XrConfig) -> Vec<DeviceDescriptor>;
pub fn validate_frame(frame: &XrFrame, config: &XrConfig) -> Result<(), XrValidationError>;
pub fn observation(
    frame: &XrFrame,
    sample: &XrDeviceSample,
    received_us: TimeUs,
) -> Option<SensorObservation>;
~~~

Here **XrFrame** is a dedicated extracted struct with seq, client_time_us and three samples, defined in contracts/xr.rs and used by XrClientMessage::Frame with **#[serde(flatten)]** only if its generated wire shape remains identical to §3. To avoid a mismatch, preferred implementation is a plain XrFrame { seq, client_time_us, samples } struct and a tagged XrClientMessage::Frame { seq, client_time_us, samples }, then build XrFrame after decoding; no serde flatten is required.

**Reject whole frame** before generating any events if any sample is malformed. If a validated frame contains one untracked device, that device alone emits no pose; other tracked devices still publish normally.

### 5.3 Engine API adapter (step C)

**api/xr.rs** owns **XrIngress**:

~~~rust
pub struct XrIngress {
    pub config: XrConfig,
    pub clock: SessionClock,
    pub events: tokio::sync::mpsc::Sender<Ingest>,
    pub cancel: CancellationToken,
    pub single_writer: std::sync::Arc<tokio::sync::Semaphore>,
    pub dropped: DropCounters,
}
~~~

**Required control flow:**

1. In api/mod.rs register **.route("/api/v1/ingest/xr", get(xr::upgrade))**. Handler must return 404 / an existing structured ApiError when xr.enabled=false.
2. Check real **ConnectInfo<SocketAddr>** and require loopback; do not treat missing connection metadata as trusted. Reject remote with 403 before upgrade even if LAN token is present.
3. Acquire the single writer by **try_acquire_owned** prior to upgrade; if unavailable return 409. The permit remains held throughout the connection and releases on exit.
4. Upgrade with **max_message_size(config.max_message_bytes)**; disable or reject binary application frames.
5. Require typed **Hello** first. Send typed **Ready**. Reject wrong protocol version and unexpected message order, with no source events.
6. For every accepted Frame, validate fixed samples/order, seq and monotonic client timestamp. Limit to config.max_ingest_rate_hz using server arrival time; over-rate frames are **dropped and counted**, not queued. Use a simple per-connection next-allowed-time strategy with bounded jitter tolerance. Record the exact limit behavior in tests.
7. Capture **one** t_receive_us via SessionClock::now_us(); map the three samples with sources/xr.rs and send SourceEvent::Observation and DeviceLink transitions via existing **Ingest::Event**.
8. Use **try_send** on the existing bounded queue; never wait indefinitely on full ingestion. On failed send, increment per-device drop counter. If a frame cannot enqueue all three observations, partial device delivery is allowed and accounted for; never falsely report them as all accepted.
9. For loss of tracking: emit DeviceLink(false) once for that device (or on link close), and DeviceLink(true) on next acquisition. The target Registry already clears recent data on reconnect; test that path rather than adding independent caches.
10. Close and send DeviceLink(false) for all tracked devices on WebSocket disconnection, protocol failure, XR end, or engine shutdown. On cancellation, exit promptly without blocking the runtime channel. Log controlled summaries; no frame-by-frame info logs.
11. On engine replay, **reject** active XR upload at connection or frame gate (409/explicit error); the existing runtime also ignores live events during replay. Do not allow new XR observations to pollute a replay.
12. A sender that holds the socket open but sends no frames for longer than xr.tracking_stale_after_ms must be marked disconnected by a bounded receive timeout; the next valid frame may reconnect and resume.

**Event ordering requirement:** For each newly tracked device, SourceEvent::DeviceLink(connected=true) precedes its first RigidPose observation. Link events and observations use the same server receive clock domain. This is essential because Registry::event clears latest data on reconnection ([registry/mod.rs L112-L155](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/registry/mod.rs#L112-L155)).

**Security requirement:** Apply the existing [LAN/auth/origin middleware](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/api/mod.rs#L350-L490). XR does not receive special LAN exemptions. Input is rejected before using it for telemetry; strict JSON decoding, bounded WS frames, bounded queue, rate limiting and one active writer protect the engine. No new secrets, dynamic plugin loading or remote arbitrary source IDs.

### 5.4 App bootstrapping (step D)

In [app.rs L63-L80, L120-L163](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/app.rs#L63-L163), after existing source descriptors have been collected but **before** Registry::new:

- If config.config.xr.enabled, append exactly three XR descriptors from sources::xr::descriptors.
- Insert an Arc<AtomicU64> drop counter for each XR device into existing DropCounters.
- Reject collisions with existing configured source IDs/device IDs during validation or startup; **do not** silently overwrite a descriptor/drop counter. A configured physical camera named XR-HMD is invalid when XR is enabled.
- The engine's existing event sender and SessionClock are cloned into the XR ingress object. Do **not** spawn a redundant main fusion pipeline or second recorder.
- Add **Api.xr: Option<Arc<XrIngress>>** (or an equivalent Arc-backed typed field) in [api/mod.rs L20-L43](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/api/mod.rs#L20-L43) and populate it at [app.rs L255-L276](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/app.rs#L255-L276).
- Confirm source registration is visible in SystemState even before first observation and starts in **acquiring** rather than **tracking**.
- Preserve existing static device identity and assigned segments. XR devices have **no SegmentId assignment** in Phase 01.

**No edits to:** pipeline/solve.rs, pipeline/fusion.rs, pipeline/triangulate.rs, skeleton definitions, calibration profiles, presentation buffer or training scorer.

### 5.5 Godot capture (step E)

**apps/vr/scripts/xr_pose_capture.gd** must be pure enough for headless testing. It accepts an XROrigin3D or equivalent origin Node3D, an XR camera, left and right grip nodes, and three tracking-validity booleans. Return a Dictionary conforming to §3; do not serialize or transmit here.

**Node setup in training_room.gd:**

- Preserve the current **aim_pose** XRController3D nodes for _input_router.add_hand and UI interaction.
- Create exactly two additional XRController3D children of _xr_origin, tracker=left_hand/right_hand, **pose=&"grip_pose"**. They are **not** added to input router, not visually rendered, and never handle trigger/buttons.
- Use existing _xr_camera as head position reference. The head is tracked when XR is initialized, focused and the XR tracking state is valid; reject non-tracking runtime states rather than synthesizing a pose.
- Use **get_has_tracking_data()** for each grip node, consistent with [vr_input_router.gd L47-L49](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/apps/vr/scripts/vr_input_router.gd#L47-L49).
- Capture all three **in one processing tick**, with one **Time.get_ticks_usec()** value, before preparing the JSON.
- Convert each using **inverse(xr_origin.global_transform) * node.global_transform**; verify output finite and normalized. A missing/invalid pose serializes as tracked=false and null values.
- The existing right-hand recenter and alignment functionality remains bound to its current aim controller; no behavior change to buttons/panels.

**Critical head-pose API check:** Before implementation, confirm the exact Godot version's **XRInterface.get_tracking_status()** tracking-status values and how XRCamera3D reports HMD validity in Godot 4.4/4.5 runtime. If the API differs, record the verified equivalent and update the small head-validity function before writing code. **Do not** assume XRCamera3D implements the controller-only get_has_tracking_data method.

### 5.6 Godot transport (step F)

**apps/vr/scripts/xr_pose_uplink.gd** is one Node with **WebSocketPeer**, independently owned by training_room.gd:

- Only active when XR initialized and bansayo/xr_uplink_enabled true.
- engine_url set from the same resolved URL as EngineClient; convert to ws/wss and append /api/v1/ingest/xr.
- On open send **Hello** and wait for **Ready** with protocol_version==1. On failure close and log one diagnostic.
- Call socket.poll every frame. Sample/send **at most 60 Hz** based on monotonic Time.get_ticks_usec and project setting; do not depend on engine frame publish rate.
- If get_current_outbound_buffered_amount() exceeds configured 8192 bytes, **skip the current frame** and increment a local dropped counter; do not accumulate unbounded pending frames.
- Sequence number is per **connection**, starts at 0 on every new Ready; no multiple parallel sockets. Client clock sample remains monotonic across reconnect because the Godot process clock continues.
- On XR focus loss emit all untracked samples promptly if connected, then pause samples until focused. On session stopping/desktop fallback close WebSocket, stop publisher, restore desktop UI unaffected.
- On recenter keep reporting XR-stage poses (which may shift), clear any local cache and surface a single alignment-invalid indication. Do not silently apply BodyRoot alignment or reset calibrated world.
- Use capped exponential backoff, default 500–8000 ms as configured in project.godot. On a disabled endpoint (404), report an informative status and retry only with capped backoff, not every frame.
- Sender must never mutate _body, _buffer, _root, _clinic.pose_available or incoming EngineClient frame data.
- Optionally expose one compact status string in the existing setup/tracking UI if feasible without coupling; no separate feature panel in Phase 01. Always preserve useful error logging.

### 5.7 Lifecycle details

| Situation | Required behavior |
|---|---|
| XR runtime unavailable / --desktop | No uplink connection, no grip capture, no crash |
| Engine xr.enabled=false | HTTP 404 or structured disabled error; Godot retries with cap if locally enabled |
| XR visible but unfocused | No tracked=true poses; one transition to unavailable |
| Right grip occluded | Right tracked=false/null; head + left continue |
| HMD tracking unavailable | Head tracked=false/null; hands may continue only if runtime reports them tracked |
| WebSocket remote peer | Reject 403 before upgrade even with LAN mode enabled |
| Second uplink | Reject 409; first uplink uninterrupted |
| Client reconnect with seq=0 | DeviceLink(true) resets registry stream history; observation accepted |
| Duplicate or backward seq | Reject frame, count; no recording |
| Over-rate frames | Drop/count, no queue growth |
| Queue full | try_send failure count per affected device, never block |
| 250 ms with no frames | Mark tracked devices disconnected; remain connected socket for recovery |
| Engine in replay | Refuse uplink; live XR cannot change replay or its recording |
| XR recentered | Capture remains in raw stage; invalidate later world alignment, preserve other features |
| Application exits | Socket close and cancellation; no hanging task |
| Camera-only or simulator config | Identical behavior to before, since XR remains off by default |

## 6. Deterministic tests — exact test names, fixtures and assertions

### 6.1 Contract tests

Add in **crates/bansayo-contracts/tests/contracts.rs** or new focused module:

| Test name | Assertions |
|---|---|
| xr_client_hello_round_trip | Exact {"type":"hello","protocol_version":1} serde round trip |
| xr_frame_requires_three_ordered_roles | Three samples, correct order accepted; duplicates/order errors rejected by validation |
| xr_untracked_requires_absent_pose | tracked=false requires null position + rotation; true requires both |
| rigid_pose_contract_round_trip | XR stage frame, xyzw quaternion, version-1 SensorObservation round trip |
| legacy_observation_examples_unchanged | Old Orientation, Position, Landmarks2d goldens still decode |
| xr_generated_types_current | BANSAYO_BLESS generated output checked in and stable |

Use generated-output checking already implemented in [tests/contracts.rs L10-L20](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-contracts/tests/contracts.rs#L10-L20) and [support/generated.rs L171-L199](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-contracts/tests/support/generated.rs#L171-L199).

### 6.2 Engine unit tests

Place focused tests in sources/xr.rs, sources/validation.rs, config/validate.rs or registry/mod.rs. Naming:

- **xr_descriptors_are_stable_and_disabled_by_default**: false→0, true→3, exact IDs and capabilities.
- **xr_config_rejects_invalid_bounds_and_unknown_fields**: NaN, negative values, oversized message, wrong schema version.
- **xr_adapter_accepts_valid_stage_pose**: exact expected serialized SensorObservation and shared server timestamp.
- **xr_adapter_rejects_nonfinite_and_nonunit_quaternion**: NaN/Inf/zero, norm tolerance boundaries.
- **xr_adapter_rejects_misordered_or_duplicate_roles**: no accepted partial events.
- **xr_adapter_rejects_pose_when_untracked**: no fabricated pose.
- **xr_registry_counts_pose_as_primary_stream**: 60 pose observations at a deterministic cadence report 60 Hz within tolerance.
- **xr_registry_reconnect_resets_sequence_floor**: prior seq 50 then link false/true, new seq 0 accepted.
- **xr_does_not_affect_camera_imu_solver**: compare canonical pose and provenance with XR disabled versus enabled but unassigned; same fixture output.
- **xr_source_is_recording_compatible**: typed XR SourceEvent survives serialization and replay.

### 6.3 Live network integration tests

**New file: crates/bansayo-engine/tests/xr_ingest.rs.** Use **bansayo_engine::start**, the existing test engine helpers, and **tokio_tungstenite** already present in engine dependencies ([Cargo.toml](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/Cargo.toml#L1-L49)). Use OS-assigned port, temporary data directory, bounded timeouts, deterministic message order.

| Test name | Exact requirement |
|---|---|
| xr_ingest_disabled_by_default | No XR descriptors, no accepted uplink with default config |
| xr_ingest_announces_three_devices | First state contains exactly three XR devices in acquiring state |
| xr_ingest_hello_ready_and_three_poses | Ready has protocol_version=1, time_quality=receive_time; registry receives head/left/right samples with same t_us |
| xr_ingest_rejects_bad_version_and_missing_hello | Reject with WS protocol error, no SourceEvent accepted |
| xr_ingest_rejects_binary_and_oversized | Correct websocket close/error and no engine panic |
| xr_ingest_rejects_bad_role_order_and_nonfinite_payload | No partial frame acceptance |
| xr_ingest_tracking_loss_and_reacquire | Right device disconnected on tracked=false, head/left continuous, seq continues correctly |
| xr_ingest_single_writer_rejects_second_connection | Second HTTP request returns 409; primary still publishes |
| xr_ingest_sequence_reset_only_after_reconnect | Duplicate/old seq rejected; new socket seq=0 accepted |
| xr_ingest_times_are_server_receive_time | All three observations share t_us, device_time_us preserves original, time_quality receive_time |
| xr_ingest_idle_disconnects_without_false_pose | No frames > configured timeout leads to disconnected status |
| xr_ingest_replay_does_not_accept_live_samples | Replay data/state remains unchanged by attempted live uplink |
| xr_ingest_recording_roundtrip | Start recording, ingest known XR frames, stop, inspect MCAP with existing reader and replay accepted event order |
| xr_camera_only_regression | Existing simulator/camera pose output unchanged with XR disabled |

For **remote-peer rejection**, unit-test the connection-origin gate with a non-loopback SocketAddr; a full non-loopback integration test is optional only when CI host networking supports a test address. **Never** infer security from an unavailable network test alone.

### 6.4 Godot headless tests

**New file apps/vr/tests/xr_observation_smoke.gd** (SceneTree pattern already used in [contract_smoke.gd](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/apps/vr/tests/contract_smoke.gd)).

Use synthetic Node3D parent/origin and child device transforms, **not** an active OpenXR runtime:

1. **identity_stage_transform** — inverse origin transform gives same coordinates.
2. **presentation_offset_is_removed** — origin (0,0,3), camera world (0,1.7,3) returns (0,1.7,0).
3. **origin_yaw_is_removed** — stage coordinates unchanged by parent rotation.
4. **xyzw_unit_quaternion_serialization** — exactly [x,y,z,w], finite, normalized.
5. **tracking_lost_emits_null_pose** — tracked=false plus both fields null.
6. **samples_have_fixed_role_order** — head, left_grip, right_grip.
7. **desktop_mode_does_not_require_openxr** — no Runtime error and no network started.

Keep this test entirely deterministic. Do not substitute headless test success for validation in a physical headset.

### 6.5 CI and regression commands

Run from repository root on each implementation commit:

~~~bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
BANSAYO_BLESS=1 cargo test -p bansayo-contracts
cargo test --workspace
node scripts/verify-gate1.mjs
node scripts/verify-gate2.mjs

godot --headless --path apps/vr --editor --quit
godot --headless --path apps/vr --xr-mode off --script tests/contract_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/spatial_ui_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/xr_observation_smoke.gd
~~~

The existing CI workflow is [ .github/workflows/ci.yml ](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/.github/workflows/ci.yml). Update it to include the new Godot smoke test only after confirming Godot installation behavior in that workflow. If Godot is unavailable on CI, add a documented, pinned Godot setup rather than silently skipping the new test. Re-run the normal **cargo test --workspace** after blessing; the second run must not modify generated files.

**Expected:** no outdated TS/golden artifacts, no clippy warnings, no protocol-v1 regression, no broken VR desktop mode. No requirement to install any Unity, MediaPipe, MocapForAll or Kinect dependency.

## 7. Manual headset validation protocol

Required once the code exists and an OpenXR headset is available. Record device model/runtime/firmware and engine commit SHA; never mark this done from simulation.

1. Start Bansayo using **config/xr-local.example.yaml** and verify health + status.
2. For local testing, temporarily enable **bansayo/xr_uplink_enabled=true** through a non-committed Godot override and launch VR.
3. With headset stationary and both grip controllers tracked, verify **three device statuses** progress from acquiring to tracking.
4. Record 10 seconds of still poses. Confirm measured receipt rate per device; target **>=45 Hz** at configured 60 Hz, and no unbounded socket queue growth. This is a provisional operational threshold, not a tracking-accuracy claim.
5. Move head only, left hand only and right hand only. Verify the intended device's pose changes and the others remain stable within physical tracking noise. Compare relative movements; world calibration is not yet available.
6. Put right controller out of tracking view. Right becomes disconnected/untracked while head and left continue; restore it and verify recovery.
7. Recenter XR. Confirm **raw stage values can change**, that no camera-world transformation was silently applied, and that future alignment must be redone.
8. Kill and restart Godot while the engine stays running. New seq=0 samples must be accepted after link reconnection.
9. Record a short session, inspect MCAP events and replay it. Compare pose values and origin/time-quality fields with captured data.
10. Stop the engine while Godot is open, restart it, and verify bounded reconnect and no flood of errors.
11. Run existing simulator camera-only and desktop Godot scenarios. Confirm unchanged pose and UI behavior.
12. Measure ingest time per frame and UI/engine status latency with instrumentation. Do not claim camera/XR fusion accuracy or end-to-end latency until later phases.

**Hardware report template:** commit SHA; headset; controllers; OpenXR runtime; Godot version; OS; engine config; frame rate median/p95; per-device received/dropped; queue depth; disconnect/reconnect behavior; coordinate sanity screenshots; recording/replay comparison; crashes; missing hardware; outcome **PASS/FAIL/BLOCKED**. Save at docs/validation/YYYY-MM-DD-phase01-openxr-uplink.md.

### Pass/fail targets

| Criterion | Pass | Fail |
|---|---|---|
| Typed wire compatibility | Exact §3 shape; v1 downlink unaffected | Any breaking existing payload |
| Input validity | 100% malformed frames refused before registry | NaN, zero pose or invalid role enters registry |
| Clock honesty | time_quality=receive_time, server timestamp | Client ticks mislabeled as synchronized engine time |
| Coordinate invariance | Raw stage output independent of presentation origin transform | Parent transform leaks into source coordinates |
| Tracking loss | Absent sample never becomes origin/identity measurement | Ghost controller/head pose produced |
| Reconnect | New seq=0 accepted after true reconnection | Permanently stale device or mixed-session pose |
| Recording/replay | Accepted XR observations round-trip unchanged | Samples bypass recorder or replay changes |
| Regression | cargo + Godot checks pass; camera/IMU outputs unchanged | Existing tracking or VR UI broken |
| Device status | Three devices visible when enabled; zero when disabled | Phantom devices in disabled config |
| Live headset target | >=45 Hz each over 10 seconds (provisional) | Below threshold, unexplained loss or blocking |
| Security | Local-only, one writer, bounded messages and queue | Remote unauthorized uplink or unbounded growth |

**Scope caveat:** The 45-Hz live target is not proof of tracking accuracy. Phase 02 will assess room alignment and timing, Phase 03 body calibration, Phase 04 constrained IK, Phase 05 avatar retargeting and Phase 06 additional foot landmarks.

## 8. Prescribed implementation order — no discretionary steps

Complete in this order. A gate failure blocks all later steps. Commit directly to **main** only after reviewing main again to avoid overwriting unrelated work.

### Commit 01 — Contracts

- [ ] Refresh main; record SHA and source-map changes.
- [ ] Add xr.rs types and RigidPose/DeviceKind/SensorCapability variants.
- [ ] Add strict serde and golden example tests.
- [ ] Regenerate TypeScript/golden outputs with BANSAYO_BLESS.
- [ ] Run cargo fmt, contracts tests and workspace compile.
- [ ] Commit: **contracts: define version-one OpenXR ingress and rigid pose observation**.
- **Gate:** Legacy examples unchanged; new XR examples round-trip; all exhaustive matches resolved.

### Commit 02 — Configuration and engine adapter

- [ ] Add XrConfig and validate with explicit §4 bounds; default off.
- [ ] Add sources/xr.rs and source/validation matching.
- [ ] Add ObservationKind::RigidPose and primary rate accounting.
- [ ] Add config/xr-local.example.yaml.
- [ ] Run engine unit tests, workspace tests, clippy.
- [ ] Commit: **engine: validate and register OpenXR device observations**.
- **Gate:** Exactly three reserved descriptor IDs and no changes to solver output.

### Commit 03 — WebSocket uplink

- [ ] Add api/xr.rs with loopback, single-writer and typed protocol.
- [ ] Wire through api/mod.rs and app.rs using existing event queue and SessionClock.
- [ ] Add idle timeout, tracking loss, reconnect and nonblocking drops.
- [ ] Add network tests with recorded-event and replay assertions.
- [ ] Run all engine/contract tests and clippy.
- [ ] Commit: **engine: accept and record local OpenXR tracked-device stream**.
- **Gate:** All network tests in §6.3 pass and legacy /api/v1/stream unchanged.

### Commit 04 — Godot capture

- [ ] Add xr_pose_capture.gd with pure frame conversion.
- [ ] Add separate grip nodes; retain current aim nodes and UI controls.
- [ ] Add headless synthetic smoke tests and verify Godot API for HMD tracking validity.
- [ ] Run existing Godot contract/spatial tests.
- [ ] Commit: **vr: capture OpenXR head and grip poses in stage coordinates**.
- **Gate:** No active headset required for deterministic geometry tests.

### Commit 05 — Godot sender

- [ ] Add xr_pose_uplink.gd; reuse engine URL and bounded reconnect.
- [ ] Hook training_room.gd into focus/session/recenter/desktop lifecycle.
- [ ] Add disabled project defaults and sender diagnostics.
- [ ] Run Godot headless tests and Rust tests.
- [ ] Commit: **vr: stream timestamped OpenXR poses to local engine**.
- **Gate:** Desktop remains clean; no unbounded queued packets; protocol handshake works.

### Commit 06 — Documentation, test hardening and real-hardware gate

- [ ] Update apps/vr/README.md and docs/13-implementation-status.md.
- [ ] Add new smoke test to CI with appropriate Godot setup.
- [ ] Capture hardware validation report; mark untested scenarios BLOCKED rather than PASS.
- [ ] Run complete §6.5 command set and check git diff/status.
- [ ] Commit: **docs: validate phase-01 OpenXR observation pipeline**.
- **Gate:** Reproducible tests, code review, source reference audit, no uncommitted generated files.

## 9. Non-goals and interfaces for next phases

**Explicit non-goals**:

- Camera-to-XR alignment: **Phase 02**, using calibrated T_world_from_xr. The current XR room alignment [training_room.gd L243-L268](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/apps/vr/scripts/training_room.gd#L243-L268) is for presentation, not an engine sensor calibration profile.
- Device clock offset/drift estimation: **Phase 02**, using existing [clock.rs::OffsetEstimator](https://github.com/danielsobrado/Bansayo/blob/b348d89016a5cec7ecf6a6c536f2ff44d0e25c90/crates/bansayo-engine/src/clock.rs#L45-L157). The raw client clock remains available for later mapping.
- Controller grip→wrist offset and body dimensions: **Phase 03**. Grip pose is not anatomy.
- Head/controller/camera/SlimeVR fusion and elbow IK: **Phase 04**. Do not borrow upstream hard-coded Euler offsets.
- Humanoid mesh retargeting: **Phase 05**. No Godot avatar API changes here.
- BlazePose 33-keypoint/heel/toe evaluation: **Phase 06**, not a dependency for Phase 01.
- SteamVR/VMT/OSC, Kinect v2, third-party Unity packages, cloud relay and remote VR clients: deferred.

**Future consumer guarantee:** After Phase 01, the engine owns a typed and replayable stream of stage-relative XR tracked-device poses. A later solver may consume it only after alignment, clock mapping and anatomical calibration. The solver must never silently reinterpret a device pose as an anatomical joint pose.

## 10. Implementation AI completion report

After executing the six commits, deliver the following, with verifiable evidence:

1. Before/after main SHA and six commit links.
2. Final file manifest with new/modified paths and nontrivial deviations from this document.
3. Full commands executed and PASS/FAIL results (do not simply say “tests pass”).
4. Explicit contract-v1 compatibility proof and evidence that existing simulator/camera/IMU output is unchanged.
5. Summary of tracking drop, reconnect, rate-limit and malformed-payload tests.
6. Hardware validation report, or **BLOCKED: no physical OpenXR headset tested** with exact blocked items.
7. Any remaining tasks as **TODO** with exact owner/target file and why blocked.

**Definition of done:** The engine accepts only well-formed, local, rate-bounded OpenXR tracked-device observations; its existing state/recording/replay reports them honestly with raw XR-stage coordinates and receive-time timestamps; Godot acquires head and both **grip** poses without disrupting aim-based UI; all existing tests and hardware gates are explicitly accounted for; no camera/IMU fusion behavior was changed.

**Licensing:** FullBodyPoseEstimation ships GPL-3.0. Its public code is cited strictly as an architectural and behavioral reference. Do **not** copy, translate or incorporate GPL source files, Unity scenes, models or vendor assets into Bansayo. Verify the license of any future third-party runtime package independently before importing it. Phase 01 requires **no external code or model weights** from the reference.
