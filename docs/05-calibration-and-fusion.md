# 05 — Calibration and fusion

## 1. Calibration as a platform capability

All calibration lives in the `calibration` module: procedures, the profile store and the
invalidation rules. Solvers read profiles; they never compute or cache calibration themselves.

A **calibration profile** is a persistent, versioned record of one calibration result.

```json
{
  "schema_version": 1,
  "id": "mounting-SIM-T01-pelvis",
  "kind": "mounting",
  "method": "reference_pose",
  "origin": "simulated",
  "created_utc": "2026-11-16T10:22:05Z",
  "subject": { "device_id": "SIM-T01", "segment_id": "pelvis" },
  "depends_on": [{ "kind": "heading_alignment", "device_id": "SIM-T01" }],
  "data": { "q_sensor_from_segment": [0.0, 0.0, -0.70710677, 0.70710677] },
  "quality": { "capture_window_ms": 1000, "angular_spread_rad": 0.002 }
}
```

`data` and `quality` are specific to the profile kind; each kind has a typed form in the contracts
crate (`MountingData`, `MountingQuality`). The `id` is a validated `ProfileId` and is the file
name.

One JSON file per profile in `<data_dir>/calibration/`. Files are written atomically (write to a
temporary name, then rename). A file with an unsupported `schema_version` or that fails to parse
is reported as `invalid` and ignored; it is never deleted automatically.

### States

| State | Meaning | Effect on output |
|---|---|---|
| `missing` | No profile exists | Dependent values are not derived |
| `valid` | Profile exists and its dependencies hold | Used |
| `stale` | Profile exists but something it depended on changed | Not used; UI asks for recapture |
| `assumed` | No calibration was performed; a documented default is in use | Used, and every dependent output carries a reason |
| `invalid` | Profile could not be loaded or failed a consistency check | Not used |

`assumed` exists so that a default is never silently presented as a calibration.

## 2. Procedures

Each procedure is separate, with its own input, output and failure explanations.

| Procedure | Profile kind | What the user does | Output | Status |
|---|---|---|---|---|
| IMU readiness and bias | `imu_bias` | Leaves trackers still on a surface | Gyro bias, readiness flag | Designed. Simulated trackers have no bias |
| Mounting alignment | `mounting` | Holds the reference pose | `q_sensor_from_segment` per assigned tracker | **PoC** |
| Heading alignment | `heading_alignment` | Reference pose, then bends forward | `q_world_from_sensor_reference` (yaw) per tracker | Designed, §4. Assumed identity in the PoC |
| Body proportions | `body_proportions` | Enters height; later, measured segment lengths or a camera-based fit | Scaled skeleton | PoC: height only, state `assumed` |
| Joint axes | part of `body_proportions` | Flexes knees and elbows | Functional joint axes | Extension point |
| Camera intrinsics | `camera_intrinsics` | Shows a calibration board to each camera | Focal lengths, principal point, distortion | Designed |
| Camera extrinsics | `camera_extrinsics` | Shows a board or markers visible to several cameras | `T_world_from_camera` | Designed |
| Floor plane | `floor_plane` | Places a marker on the floor, or stands still | Transform that makes the floor `y = 0` | Designed |
| Camera / IMU alignment | `heading_alignment` (camera-referenced) | Moves in view of calibrated cameras | Heading from optical observation instead of the bend | Extension point |
| Headset / world alignment | `xr_alignment` | Stands in the reference pose wearing the headset | `T_xr_stage_from_world` | Designed |
| Session time sync | `time_sync` | Nothing (automatic), or a clap/flash for cameras | Per-device clock offset | PoC: automatic offset estimate, not persisted |

## 3. Mounting alignment by reference pose (PoC)

Goal: find `q_sensor_from_segment` for each tracker, so that its readings can be turned into a
segment orientation.

The user stands in the reference pose (the rest T-pose), facing world +Z. In that pose every
segment's world rotation is identity by definition, so for each assigned tracker:

```text
identity = A · R̄ · M            A  heading alignment (assumed identity in the PoC)
       M = (A · R̄)⁻¹           R̄  mean reading over the capture window
                                M  q_sensor_from_segment
```

Procedure:

1. Collect readings for every targeted tracker over a capture window (default one second).
2. **Stillness check.** If the angular spread of any tracker's readings in the window exceeds a
   threshold, the capture fails for that tracker with "Movement detected during capture".
3. Compute `R̄` as the normalised mean quaternion (hemisphere-aligned).
4. Write one `mounting` profile per tracker, keyed by `(device, segment)`.
5. Publish the new state.

Failure explanations are specific: movement detected, tracker not delivering data, tracker not
assigned, engine in replay mode.

The engine cannot verify that the subject was actually in the reference pose; nothing in an
IMU-only system can. The UI therefore instructs before capturing and shows the result immediately
so a wrong capture is obvious. With the simulator, the user selects the "Reference pose" motion
first.

## 4. Heading alignment (designed)

A six-axis IMU knows which way is up but not which way is north. Each tracker's reference frame
has its own arbitrary heading, so one static pose cannot separate heading from mounting. The PoC
assumes heading alignment is identity, which is true for the simulator when `heading_offset_deg`
is zero and is **not true for real trackers**. Setting a non-zero `heading_offset_deg` in the
simulator shows the resulting error deliberately.

The designed procedure adds a second pose:

1. Reference pose: record `R₁` per tracker.
2. The user bends forward at the hips, keeping the body in the sagittal plane: record `R₂`.
3. `D = R₂ · R₁⁻¹` is the rotation the tracker underwent, expressed in its own reference frame.
   Its axis `a` is, physically, the subject's left-right axis.
4. In world, that axis is +X. Heading alignment `A` is the yaw that takes the horizontal
   projection of `a` to +X.
5. With `A` known, recompute mounting as in §3.

Checks: the rotation angle of `D` must exceed a minimum (otherwise "Bend further forward"), and
its axis must be close to horizontal (otherwise "Bend forward without twisting"). Trackers on
segments that do not move in the bend (feet) need a different second pose; that is part of the
design work for this milestone.

Heading drifts over time on six-axis IMUs. Re-running this procedure is the manual correction;
camera-based correction is fusion stage 4.

## 5. Invalidation

| Event | Profiles affected | New state |
|---|---|---|
| Tracker reassigned to another segment | `mounting` for the old `(device, segment)` pair | `stale`. The new pair is `missing` |
| Tracker reassigned back to a segment it had before | The old `mounting` profile | Stays `stale`: the strap may have moved. Recapture required |
| Heading alignment recalibrated | `mounting` for that device | `stale` |
| Camera resolution or lens setting changed | `camera_intrinsics`, then `camera_extrinsics` | `stale` |
| Camera moved (user declares, or reprojection error rises) | `camera_extrinsics`, `floor_plane` if it was the reference | `stale` |
| Skeleton height or proportions changed | Nothing rotational. `SkeletonDefinition.rev` increments | — |
| Skeleton model changed | Every `mounting` profile whose segment no longer exists | `invalid` |
| Device firmware changes its axis convention | `mounting`, `imu_bias` | `stale` |

The rules are one table in `calibration/invalidation.rs`, a pure function from an event and the
current profiles to state changes.

## 6. Calibration in the UI

- Every device card shows its calibration kinds and states.
- The top status bar shows a single calibration readiness summary: ready, action needed, or
  running on assumptions.
- A `stale` or `missing` state always offers the action that fixes it.
- An `assumed` state is shown wherever its output is shown, not hidden in a settings page.
- A failed procedure states what went wrong and what to do differently.

## 7. Sensor-fusion roadmap

Fusion is built in stages. Each stage is useful on its own, and each has a named place in the code
where it goes. A stage is started only when the one before it is working with real hardware.

| Stage | Capability | Where it lives | Status |
|---|---|---|---|
| 1 | Independent camera and IMU pipelines | `sources`, `pipeline` | **PoC** |
| 2 | Unified timestamps and coordinates | `clock`, `calibration`, `FrameRef` on every value | **PoC** for IMU and simulated cameras; camera sync designed |
| 3 | Calibrated body-segment orientations | `pipeline::solve` orientation stage | **PoC** |
| 4 | Camera-based root position and heading correction | A positional stage after the orientation stage; a slow complementary correction on each tracker's heading alignment | Designed |
| 5 | Joint and skeletal constraints | A constraint stage after forward kinematics: joint limits, bone-length consistency, foot locking | Designed |
| 6 | Contact and distance observations | Additional terms in the positional and constraint stages | Extension point |
| 7 | Joint estimation over a time window | Replaces stages 3–6 with one estimator behind the same stage boundary | Only if evidence demands it |

### Solver structure

The solver is an ordered list of **stages**, each a plain function from the current observation
snapshot and a draft pose to an updated draft:

```text
observations ─► orientation ─► fill unobserved ─► forward kinematics ─► root placement ─► draft pose
                 (stage 3)                                              (floor contact; later stage 4)
```

In the PoC these are four functions called in sequence in `pipeline/solve.rs`. A trait and
configurable stage list are introduced when a second implementation of the same stage exists, not
before (ADR-011).

### Why no EKF or factor graph now

Stages 1–3 are closed-form. Stage 4 is a complementary correction with one gain per tracker. An
EKF or factor graph becomes worth its complexity when camera positions and IMU orientations must
be reconciled under occlusion with a motion model, which is stage 7. Introducing it earlier would
mean tuning a filter against simulated data, which proves nothing. The stage boundary is the
extension point; nothing upstream or downstream of the solver changes when the estimator does.
