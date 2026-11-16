# 03 — Domain model

Conventions in this document are normative. Every adapter converts vendor conventions into these
before emitting anything; nothing downstream of an adapter ever sees a vendor convention.

## 1. Coordinates

### World frame

| Property | Convention |
|---|---|
| Handedness | Right-handed |
| Up | +Y |
| Forward reference | +Z |
| Left when facing forward | +X |
| Units | Metres |
| Floor | The plane `y = 0` |
| Origin | A point on the floor fixed at calibration. Without positional calibration: the subject's standing position |

This matches glTF, VRM 1.0 and Godot's character convention, and shares handedness, up-axis and
unit with three.js, Godot and OpenXR. No axis flip is needed in any client.

The floor plane is `y = 0` by definition. A floor calibration profile stores the transform that
makes this true for a given capture space; it does not store a plane equation that clients have to
apply.

### Quaternions

- Unit Hamilton quaternions, wire order `[x, y, z, w]`.
- `q_a_from_b` transforms coordinates expressed in frame `b` into frame `a`:
  `v_a = q_a_from_b · v_b · q_a_from_b⁻¹`.
- Composition reads right to left: `q_a_from_c = q_a_from_b · q_b_from_c`.
- `q` and `−q` are the same rotation. Receivers interpolate along the shortest arc.
- The engine emits normalised quaternions. A received quaternion whose norm differs from 1 by more
  than a tolerance is an invalid observation and is rejected at validation.

### Frames

Every spatial value names its frame with a `FrameRef`.

| FrameRef | Meaning |
|---|---|
| `world` | Defined above. Metric only as far as calibration makes it so |
| `sensor_reference/<device>` | Gravity-aligned frame in which an IMU reports orientation: +Y up, right-handed, **heading arbitrary** unless the device has a heading reference |
| `sensor/<device>` | The device's body frame, right-handed |
| `segment/<segment>` | A body segment frame, see §3 |
| `camera/<device>` | Origin at the optical centre, +X right, +Y down, +Z along the optical axis (the OpenCV convention), metres |
| `image/<device>` | Pixels. Origin at the top-left corner, +x right, +y down |
| `model/<device>` | A detector's own non-metric space for model-inferred 3D. Never convertible to `world` without a separate measurement |
| `xr_stage` | The XR runtime's stage space. Same axes and units as `world`; related by a rigid transform (yaw and translation) |

### Transforms that connect them

| Transform | Stored in | PoC |
|---|---|---|
| `q_world_from_sensor_reference` (heading alignment, yaw only) | `heading_alignment` profile | Assumed identity, labelled `assumed` |
| `q_sensor_from_segment` (mounting) | `mounting` profile | Captured by reference pose |
| `T_world_from_camera` (extrinsics) | `camera_extrinsics` profile | Declared by the simulator |
| Camera intrinsics | `camera_intrinsics` profile | Declared by the simulator |
| `T_xr_stage_from_world` | `xr_alignment` profile | Identity |

The segment orientation equation used by the solver:

```text
q_world_from_segment(t) = q_world_from_sensor_reference · q_sensor_reference_from_sensor(t) · q_sensor_from_segment
                          └──── heading alignment ────┘   └──────── IMU reading ────────┘   └─── mounting ───┘
```

## 2. Time

| Clock | Definition | Comparable across devices |
|---|---|---|
| **Session clock** | Monotonic microseconds since the engine started (`TimeUs`, an integer). Anchored once to UTC as `session_epoch_utc` | Yes — it is the only clock used for ordering and fusion |
| **Device clock** | Whatever the device reports (`device_time_us`), typically microseconds since boot | No |
| **Wall clock** | UTC, used only for naming sessions and metadata | — |

Every observation carries `t_us`, the estimated **capture instant on the session clock**, together
with how that estimate was obtained:

| `TimeQuality` | Meaning |
|---|---|
| `synchronized` | Device and engine clocks are aligned by a two-way exchange. Designed, for custom firmware |
| `estimated` | Mapped from the device clock by an offset estimate |
| `receive_time` | No device clock; the arrival time is used, minus a configured latency if one is known |

PoC offset estimate: for each device, `offset = min(receive_time − device_time)` over a sliding
window. Jitter only ever adds delay, so the minimum tracks the true offset plus the smallest
transport latency. Drift is handled by the window sliding. This is one small pure function with a
unit test; it is replaced, not extended, when two-way sync arrives.

### Ordering

- Each stream has a monotonically increasing `seq`. Gaps are counted as loss; a lower `seq` than
  the last accepted is dropped.
- The solver orders by `t_us`, not by arrival.
- At each tick the solver uses, per device, the latest observation with `t_us ≤ tick time`. An
  observation older than `engine.stale_after_ms` no longer drives its segment.
- Frames have their own `seq` and a `t_us` equal to the tick time. Clients drop frames whose `seq`
  is not greater than the last one applied.
- Fusion that needs to rewind for late observations is an extension point. The PoC never rewinds;
  late observations are still recorded.

Cameras that are not synchronised with each other are usable individually. Triangulation requires
either hardware sync or a `time_sync` profile; without one the feature is unavailable and says why.

## 3. Skeleton

### Canonical model `kinexa.body.v1`

A skeleton is **data**: a list of segments, each with a parent, and a list of joint measurement
definitions. Acquisition code refers to segments by `SegmentId` strings validated against the
active definition, so a model with a finer spine or hands is a new definition, not a code change
in sources or clients. Hot paths resolve ids to indices once.

Nineteen segments:

```text
pelvis (root)
├── spine ── chest ── neck ── head
│                └── left_clavicle  ── left_upper_arm  ── left_forearm  ── left_hand
│                └── right_clavicle ── right_upper_arm ── right_forearm ── right_hand
├── left_thigh  ── left_shin  ── left_foot
└── right_thigh ── right_shin ── right_foot
```

Each `SegmentDef` has:

| Field | Meaning |
|---|---|
| `id` | Segment identifier |
| `parent` | Parent segment, `null` for the root |
| `offset_m` | Position of this segment's origin in the parent's frame, at rest |
| `tail_m` | Vector from the origin to the distal end, in this segment's frame |
| `contact_points_m` | Points that can touch the floor, in this segment's frame (feet only) |

The origin of a segment is its proximal joint centre. For the root, whose parent is the world,
`offset_m` is its rest position in world; in a pose it is replaced by `root_position_m`.

### Rest pose and segment frames

The rest pose is a T-pose facing world +Z. **In the rest pose every segment frame is aligned with
the world axes**: +X to the subject's left, +Y up, +Z forward. Consequences:

- A segment rotation of identity means "as in the rest pose".
- A segment's world rotation is directly comparable between segments, which keeps joint-angle
  definitions simple.
- Retargeting to an avatar is one multiplication per bone (see [08](08-vr-client.md)).

### Default proportions

Offsets are fractions of body height `H`, scaled by `skeleton.height_m`. They follow the
anthropometric ratios of Drillis and Contini as tabulated by Winter; the subdivision of the spine
is a modelling choice. Agent 1 verifies these numbers against the source before they become
constants, and they live in one table (`skeleton/proportions.rs`).

| Segment | Parent | `offset_m` / H | `tail_m` / H |
|---|---|---|---|
| pelvis | — | root, at height 0.530 | (0, 0.060, 0) |
| spine | pelvis | (0, 0.060, 0) | (0, 0.110, 0) |
| chest | spine | (0, 0.110, 0) | (0, 0.118, 0) |
| neck | chest | (0, 0.118, 0) | (0, 0.052, 0) |
| head | neck | (0, 0.052, 0) | (0, 0.130, 0) |
| left_clavicle | chest | (0.020, 0.100, 0) | (0.1095, 0.018, 0) |
| left_upper_arm | left_clavicle | (0.1095, 0.018, 0) | (0.186, 0, 0) |
| left_forearm | left_upper_arm | (0.186, 0, 0) | (0.146, 0, 0) |
| left_hand | left_forearm | (0.146, 0, 0) | (0.108, 0, 0) |
| left_thigh | pelvis | (0.0955, 0, 0) | (0, −0.245, 0) |
| left_shin | left_thigh | (0, −0.245, 0) | (0, −0.246, 0) |
| left_foot | left_shin | (0, −0.246, 0) | (0, −0.039, 0.110) |

Right-side segments mirror X. Foot contact points: heel `(0, −0.039, −0.042)·H` and toe
`(0, −0.039, 0.110)·H`.

These are population averages, known to be approximate (the implied arm span is too wide for most
people). The `body_proportions` calibration state is `assumed` until a calibration replaces them,
and the UI says so.

### Pose

A `SkeletonPose` holds, for every segment in definition order:

- `rotation` — `q_world_from_segment`.
- `position_m` — the segment origin in world, always produced by forward kinematics from the root
  position, the rotations and the proportions. Clients never compute kinematics.
- `provenance` of the rotation, `confidence`, and the contributing `sources`.

and for the root, `root_position_m` with its own provenance.

A directly observed joint position (from triangulation or an XR device) will be an additional
optional field, `observed_position_m`. Adding it is a backward-compatible change.

### Unobserved segments

A segment with no usable observation takes its **rest rotation relative to its parent** and is
marked `inferred`. It moves rigidly with its parent, which is visibly not tracking and is rendered
as such. The same happens when a tracker goes stale: the limb returns to its parent-relative rest
pose and changes appearance. It does not freeze in its last pose, because a frozen limb looks like
data.

Fresh orientation inputs that lack mounting calibration produce an entirely inferred rest pose,
with reason `mounting_calibration_missing`. This makes the reference-pose capture workflow visible
without presenting any segment as tracked (acceptance criterion A1.5 in [10](10-poc-plan.md)).
If no orientation source is configured, or every orientation observation is missing or stale,
`pose` is `null`. Camera landmarks alone never produce a world skeleton.

### Root position

Without a positional source the root is placed by a floor-contact rule: horizontal position fixed
at the origin, height chosen so that the lowest foot contact point lies on `y = 0`. Provenance is
`inferred`. A squat lowers the pelvis; a jump is not representable, and the feature list says root
position is not measured.

### Joint measurements

Defined as data alongside the skeleton, computed only in the engine.

| Id | Segments | Method | Zero |
|---|---|---|---|
| `left_knee_flexion`, `right_knee_flexion` | thigh, shin | Angle between long axes | Leg straight |
| `left_elbow_flexion`, `right_elbow_flexion` | upper arm, forearm | Angle between long axes | Arm straight |
| `left_hip_flexion`, `right_hip_flexion` | pelvis, thigh | Angle of the thigh axis from the pelvis' downward axis, in the pelvis' sagittal plane, positive forward | Standing |
| `left_shoulder_elevation`, `right_shoulder_elevation` | chest, upper arm | Angle between the upper-arm axis and the chest's downward axis | Arm hanging |
| `trunk_forward_lean` | pelvis, chest | Angle of the chest's up axis from world up, in the pelvis' sagittal plane | Upright |

On the wire, "angle between long axes" is method `axis_angle` (knee, elbow, shoulder elevation)
and an angle measured in a plane is `planar_angle` (hip flexion, trunk lean).

Each `JointDef` carries `unit` (`"rad"`) and `validated` (`false` for every definition in the PoC).
A measurement whose two segments are not both observed has `value: null`, provenance
`unavailable`, and a reason. An angle between a tracked thigh and a guessed shin is not published.

## 4. Provenance, origin and quality

Three independent labels. They are never merged into one.

### Origin — where the data came from

| `Origin` | Meaning |
|---|---|
| `live` | A physical device, now |
| `simulated` | Generated by the simulator |
| `fixture` | Prerecorded data replayed as if it were a device (reserved; none ships in the PoC) |

Engine **mode** is separate: `live` or `replay`. Replaying a simulated session is mode `replay`
with origin `simulated`.

### Provenance — how a value was obtained

| `Provenance` | Meaning | Examples |
|---|---|---|
| `measured` | Observed directly in the frame it is reported in | Triangulated landmark; headset pose |
| `derived` | Computed from sensor data through a valid calibration | Segment orientation from an IMU and its mounting profile; a joint angle between two derived segments |
| `inferred` | Filled in by a model, prior or heuristic | Rest-relative segment; floor-contact root height; monocular depth |
| `unavailable` | No value can be given | Joint angle with an unobserved segment |

A value computed from several inputs takes the weakest provenance among them.

### Tracking state

One vocabulary for devices, streams, segments and the system as a whole.

| State | Definition |
|---|---|
| `disconnected` | Known or configured, no transport link |
| `acquiring` | Link up, data arriving, not yet usable: warm-up, clock offset not settled, waiting for calibration to apply |
| `tracking` | Valid data at the expected rate and quality |
| `degraded` | Data arriving but below a threshold: rate drop, loss, low device-reported quality, stale calibration |
| `unavailable` | Cannot be used until someone acts: misconfiguration, missing model, unsupported hardware, fatal adapter error |

Transitions are driven by measured facts (last-seen age, measured rate against nominal rate,
reported quality) with thresholds in configuration, not by adapter-specific flags.

### Confidence

A number in `[0, 1]` or `null` when nothing meaningful can be said. In the PoC it is the
device-reported quality attenuated by observation age. It is a relative indicator, not an accuracy
figure, and the UI never converts it to degrees or millimetres.

### Reasons

Every degraded, unavailable or inferred condition carries machine-readable `Reason`s:
`{ code, subject, message }`. The UI renders the message and may link the code to an action.

| Code | Subject |
|---|---|
| `no_orientation_source` | — |
| `segment_not_observed` | Segment |
| `device_disconnected`, `device_degraded`, `device_stale` | Device |
| `mounting_calibration_missing`, `mounting_calibration_stale` | Device |
| `heading_alignment_assumed` | Device |
| `body_proportions_assumed` | — |
| `root_position_not_measured` | — |
| `camera_uncalibrated` | Camera |
| `needs_two_calibrated_cameras`, `cameras_not_synchronized` | — |
| `not_implemented` | Feature |

## 5. Devices and capabilities

### Identity is not placement

A `DeviceId` identifies hardware (a MAC-derived id for a tracker, a configured id for a camera).
Body placement is an **assignment** from device to segment, stored separately and changeable at
runtime. Calibration that depends on placement is keyed by the pair, so reassigning a tracker
invalidates exactly the profiles that depended on the old placement
([05](05-calibration-and-fusion.md)).

### Capabilities

A device declares what it can provide. Nothing is assumed from its `kind`.

| `SensorCapability` | Observation it enables | Unit and frame |
|---|---|---|
| `orientation` | `q_sensor_reference_from_sensor` | Quaternion, `sensor_reference/<id>` |
| `angular_velocity` | Gyroscope | rad/s, `sensor/<id>` |
| `acceleration` | Accelerometer | m/s², `sensor/<id>` |
| `position` | Position | m, a named frame |
| `distance` | Range to a surface or peer | m |
| `contact` | Contact state, optional load | boolean, N |
| `image` | Frames | pixels, `image/<id>` |
| `landmarks_2d` | 2D body landmarks | pixels, `image/<id>` |
| `landmarks_3d_model` | Model-inferred 3D landmarks | model units, `model/<id>` |
| `battery`, `signal_strength`, `quality` | Diagnostics | fraction, dBm, fraction |

Field names on the wire carry the unit (`rad_s`, `m_s2`, `m`, `px`), so a value cannot be read in
the wrong unit by accident.

### The four kinds of landmark, kept apart by type

| Kind | Type | Frame | Metric |
|---|---|---|---|
| Single-camera 2D landmarks | `PoseLandmarks2D` | `image/<camera>` | No — pixels |
| Model-inferred 3D | `PoseLandmarks3D::ModelRelative` | `model/<camera>` | No — root-relative, unknown scale |
| Multi-camera triangulated 3D | `PoseLandmarks3D::Triangulated` | `world` | Yes, with per-point view count and reprojection error |
| Calibrated metric measurements | Any value in `world` with a valid `calibration_ref` | `world` | Yes |

`ModelRelative` and `Triangulated` are different variants with different fields. There is no
function that turns the first into the second.

Each camera names the `LandmarkSchema` its landmarks follow. The simulated camera uses
`kinexa.body.v1.joints`: one landmark per segment origin, in skeleton order, with an edge for
each parent link. Real detectors bring their own schemas (for example COCO-17), and mapping a
schema onto skeleton joints belongs to the camera milestone.

### Feature availability

The engine evaluates one rule table (`pipeline/features.rs`), a pure function of devices,
assignments and calibration, and publishes the result. Clients render it; they contain no rules.

| Feature | Available when | Otherwise |
|---|---|---|
| `segment_orientation` | A tracking or degraded device with `orientation` is assigned to the segment and its mounting profile is valid | That segment is `inferred`, with a reason |
| `skeleton_pose` | At least one segment has a derived orientation | `pose: null` |
| `joint_angles` | Per joint: both segments observed | `value: null`, with a reason |
| `root_position` | A positional source exists | Inferred by floor contact; `root_position_not_measured` |
| `landmarks_2d` | At least one camera delivers landmarks | Unavailable |
| `landmarks_3d_triangulated` | Two or more cameras, each with valid intrinsics and extrinsics, and synchronised | Unavailable; `not_implemented` in the PoC |
| `camera_imu_fusion` | `skeleton_pose` and a positional camera observation | Unavailable; `not_implemented` in the PoC |
| `ground_contact` | A contact or pressure sensor is assigned to a foot | Unavailable; `not_implemented` in the PoC |
| `recording`, `playback` | Data directory writable; a session exists | Unavailable, with a reason |

Each `FeatureStatus` is `{ id, state: available | degraded | unavailable, reasons }`.

Worked examples:

| Inputs | Result |
|---|---|
| One IMU, assigned, calibrated | One derived segment; everything else inferred; no joint angles |
| Thigh and shin IMUs, calibrated | Knee flexion available for that leg |
| One camera | 2D landmarks; no 3D; skeleton pose unavailable unless IMUs are present |
| Two cameras, uncalibrated | 2D landmarks from each; triangulation unavailable: `camera_uncalibrated` |
| Twelve IMUs, mounting calibration missing | Devices tracking; all segments inferred; reason `mounting_calibration_missing` per device |
| Nothing connected | Engine healthy, system state `disconnected`, empty states in the UI |
