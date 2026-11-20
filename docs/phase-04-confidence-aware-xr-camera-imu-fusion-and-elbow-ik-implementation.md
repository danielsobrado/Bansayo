# Bansayo Phase 04 — Confidence-aware XR, camera and IMU fusion with constrained elbow IK

**Status:** Implementation specification, NOT implementation code.  
**Date:** 2026-11-20.  
**Target repository:** https://github.com/danielsobrado/Bansayo — implement directly on \`main\`.  
**Target source baseline inspected:** [38e999487b3ed609a7a78b592ee133966b2f55c3](https://github.com/danielsobrado/Bansayo/commit/38e999487b3ed609a7a78b592ee133966b2f55c3).  
**Reference source baseline:** [FullBodyPoseEstimation@be895709](https://github.com/maceq687/FullBodyPoseEstimation/commit/be895709c0bf86d35f64eb2cd5d2ffb4c6838593).  
**Prerequisites:** [Phase 01 — XR observation ingest](phase-01-openxr-observation-pipeline-implementation.md), [Phase 02 — clock and room alignment](phase-02-openxr-room-alignment-and-time-synchronization-implementation.md), and [Phase 03 — body proportions/grip-to-wrist calibration](phase-03-body-proportions-t-pose-and-controller-wrist-calibration-implementation.md).  
**Next:** Phase 05 — Godot humanoid avatar retargeting.  
**License policy:** Upstream is GPL-3.0. Independently implement the discussed concepts; do not port its source or assets.

> **BLOCKING PRECONDITION — do not skip.** At the inspected target commit, Phases 01–03 consist of **Markdown plans only**, not the planned XR/body calibration modules. Before implementing Phase 04, fetch current main, verify Phase 01's atomic XR pose ingress, Phase 02's valid world alignment/clock mapping, and Phase 03's athlete-scoped arm lengths and controller wrist offsets **exist as compiled and tested code**. If not, implement those prerequisites first. The current document is not evidence of functional fusion.

## 0. What Phase 04 delivers

Bansayo already triangulates cameras, fits camera body axes, selects mounted IMU segment orientations, applies slow camera-based tracker heading correction and maintains a camera/world root anchor. The Phase 04 work is a **small, optional arm constraint layer inside this existing Rust solve path**. No replacement full-body solver, Unity avatar logic, additional neural model or parallel network transport.

The engine can use a valid, time-matched **OpenXR controller grip pose**, **Phase 03 local grip→wrist translation**, **Phase 02 stage→camera-world transform**, measured per-arm lengths and a reliable camera elbow cue to solve a consistent two-link arm. It can maintain a bounded prior bend plane during short optical gaps; otherwise it falls back conservatively. IMU observations remain independent evidence rather than being overwritten.

**Functional acceptance:**
- Both left/right are independently solvable in world metres with correct shoulder/elbow/wrist side.
- Elbow and wrist satisfy the current upper/forearm lengths and agree with one canonical Skeleton FK result.
- Trustworthy optical elbow bend plane wins; then recent bend plane; then a deterministic, explicitly inferred anatomical fallback.
- High-quality mounted IMUs are preserved; contradictory sensors create visible reasons instead of wrist-chasing or snaps.
- All sensors/time/room/athlete/profile identity checks fail closed.
- Quaternion inputs and outputs are finite/unit, unreachable targets are bounded/rejected, no random 180° elbow flip.
- Camera-only, IMU-only, disabled XR and legacy recordings remain unchanged.
- Every new method, confidence, time and provenance attribution is accurate; no inferred elbow labeled measured.
- Deterministic replay and seeking reproduce the same bend-plane state.
- A synthetic test cannot substitute for physical held-out validation.

**Provisional non-verified performance goals:** added Rust fusion overhead p95 <1 ms/frame, full Rust fusion p95 <5 ms, standalone IK <0.1 ms/arm, held-out elbow point error <=5 cm only when the optical elbow itself is high confidence. Log actual benchmarks rather than claiming these are achieved.

### 0.1 What NOT to do

No guessed HMD→nose/head offset; HMD eye pose is not head joint center. No fixed controller Euler wrist rotation. No controller-to-controller span as shoulder width. No inferred body-scale correction. No editing the camera intrinsics/extrinsics solver. No direct Godot elbow manipulation. No unsourced axial twist, no invented confidence=1 for tracked OpenXR hardware, no sensor extrapolation, no unbounded history, no GPL C# translation, no entire-body EKF/factor graph at this phase.

## 1. Source-to-target code map (both repositories; immutable permalinks)

### 1.1 Upstream FullBodyPoseEstimation

| Exact upstream code | Observed technique | Chosen Bansayo adaptation |
|---|---|---|
| [VNectModel.cs L122–132](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L122-L132) | HMD and controller input device/pose members | Already specified in Phase 01; no duplicate device clients. |
| [VNectModel.cs L169–183](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L169-L183) | Polls native device positions and quaternions | Consume time-mapped, recorded atomic XR pose observations. |
| [VNectModel.cs L189–202](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L189-L202) | HMD/nose-position offset and fixed Euler wrist rotations | **Reject** as anatomical calibration. Use T_world_from_xr_stage and independently measured r_grip_from_wrist instead. |
| [VNectModel.cs L203–214](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L203-L214) | "Phantom elbow" by projecting optical elbow displacement onto a shoulder-controller line | **Use only the bend-plane idea.** Derive elbow from a two-sphere intersection with actual upper/forearm lengths, not a midpoint heuristic. |
| [VNectModel.cs L312–333](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L312-L333) | Adds separate phantom elbow/controller child links | Bansayo keeps **one canonical skeleton** and one FK pass. |
| [VNectModel.cs L475–522](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L475-L522) | Directly modifies Unity joint transforms | Rust solver computes poses; Godot only displays validated engine output. |
| [PoseVisualizer3D.cs L51–77](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/PoseVisualizer3D.cs#L51-L77) | 33 model-predicted landmarks | Only Bansayo **metric triangulated** world landmarks may anchor arm IK. Richer feet are Phase 06. |
| [VmtReceiver.cs L119–160](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VmtReceiver.cs#L119-L160) | Receives limb positions from external tracker messages | Existing Bansayo SourceEvent/SensorObservation is the input boundary; no OSC/VMT port. |

Upstream GPL-3.0 license: [LICENSE](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/LICENSE). Independently implement equations/tests; never paste C# code or include Unity assets.

### 1.2 Current Bansayo target code

All below pinned to commit **38e9994**. If later main changes, explicitly re-map before editing.

| Verified target and line reference | Current behavior | Exact Phase 04 edit |
|---|---|---|
| [pipeline/solve.rs L1–67](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/solve.rs#L1-L67) | Solved includes pose, rotations, observed, methods, t_ref, root evidence, fusion_next | Extend only optional per-arm constraints, preserve existing output seam and default-off behavior. |
| [pipeline/solve.rs L69–169](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/solve.rs#L69-L169) | Mounted tracker selection, triangulated world points, IMU SLERP, t_ref=t_imu.or(t_cam) | Reuse selected t_ref; do not combine camera shoulder at t_cam with XR wrist when skew is too high. |
| [pipeline/solve.rs L170–254](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/solve.rs#L170-L254) | IMU-first segment arbitration, otherwise optical fit; unobserved parent fallback; heading correction | Add gated IK **after baseline rotations**; do not blindly overwrite reliable mounted IMUs. |
| [pipeline/solve.rs L263–346](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/solve.rs#L263-L346) | Fresh camera root, short held camera anchor or floor-contact inference | Require **fresh camera root** for XR arm IK; no translating whole body toward wrists. |
| [pipeline/solve.rs L348–410](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/solve.rs#L348-L410) | One FK pass populates SegmentPose, evidence, t_obs and Solved | Insert per-arm selected rotations before FK; preserve single geometry authority. |
| [pipeline/fusion.rs L18–52](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/fusion.rs#L18-L52) | METHOD_VERSION=bansayo.fusion.anchored_fk.v1; state has per-device heading and held root | Add bounded two-arm continuity state and method version when enabled. |
| [pipeline/fusion.rs L62–126](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/fusion.rs#L62-L126) | Slow heading correction and outlier suppression | Follow gated/limited update pattern without replacing heading algorithm. |
| [pipeline/align.rs L19–80](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/align.rs#L19-L80) | No-extrapolation quaternion SLERP and reference_time() | Implement XR rigid pose interpolation using existing slerp and linear translation. |
| [registry/mod.rs L7–67](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/registry/mod.rs#L7-L67) | Orientation history cap=64; camera history cap=8; Registry owns FusionState | Add **64-entry** XR RigidPose history, strictly device-bound. |
| [registry/mod.rs L113–213](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/registry/mod.rs#L113-L213) | Accept/reconnect drops stale sequence and clears histories | Drop XR poses on disconnect/reconnect/stage reset; never interpolate through tracking loss. |
| [skeleton/definition.rs L18–45, L89–160](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/skeleton/definition.rs#L18-L160) | Parent-linked segment IDs, local offsets/tails; baseline height ratios | Consume actual **Phase 03 measured** upper arm and forearm lengths; never substitute assumed dimensions. |
| [skeleton/fk.rs L1–24](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/skeleton/fk.rs#L1-L24) | Child origin = parent origin + parent rotation×offset | Solve consistent upper/forearm swing; one FK generates elbow and wrist. |
| [pipeline/skeleton_fit.rs L15–34, L95–166](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/skeleton_fit.rs#L15-L166) | COCO17 shoulders 5/6, elbows 7/8, wrists 9/10; camera fit mapping | Reuse known semantic extractor, strict per-arm confidence and reprojection checks. |
| [pipeline/skeleton_fit.rs L249–328](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/skeleton_fit.rs#L249-L328) | Fit yields rotations, observed masks, source IDs and confidence | Keep all non-arm fitted rotations unchanged; retain evidence for fallback. |
| [pipeline/triangulate.rs L202–315](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/triangulate.rs#L202-L315) | Native Rust calibrated multi-camera triangulation; skew and reprojection QA | Do not re-triangulate or use model-relative XYZ. Use existing accepted result. |
| [contracts/landmarks.rs L81–123](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-contracts/src/landmarks.rs#L81-L123) | TriangulatedLandmark3D carries metres, views, error, confidence, propagated sigma | Honor measurement quality; propagated sigma is not a validated accuracy bound. |
| [contracts/motion.rs L31–74](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-contracts/src/motion.rs#L31-L74) | MethodId has ImuMounted, ImuOpticalFused, SkeletonFitAxis, XrDevice, RestFill etc. | Add **XrConstrainedIk** for segment swing derived from XR wrist and calibrated arm lengths. |
| [contracts/pose.rs L9–39](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-contracts/src/pose.rs#L9-L39) | SegmentPose already includes method/confidence/provenance/sources/t_obs_us | Reuse without changing ServerMessage::Frame or protocol version. |
| [contracts/quality.rs L27–75, L78–96](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-contracts/src/quality.rs#L27-L96) | Measured/Derived/Inferred/Unavailable + FeatureId | Add **FeatureId::XrArmConstraints** and reason codes; fallback is Inferred, not Measured. |
| [pipeline/motion.rs L28–78, L79–178](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/motion.rs#L28-L178) | Maps COCO17 into canonical points, sends body evidence; IMU has twist, camera axis inferred twist | Preserve arm provenance in MotionSample; never claim XR grip gives anatomical wrist twist. |
| [pipeline/joints.rs L1–84](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/joints.rs#L1-L84) | Unobserved segment means corresponding joint angle null; known weaker confidence wins | Do not turn guessed elbow bend into "observed" just because XR wrist tracked. |
| [config/schema.rs L581–652](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/config/schema.rs#L581-L652) | TrackingMode HybridAuto / CameraOnly / ImuOnly; FusionConfig heading + root | Add **fusion.xr_arms** subtree default disabled; legacy modes keep semantics. |
| [config/validate.rs L329–433](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/config/validate.rs#L329-L433) | Validates existing fusion, time and camera range | Add bounded XR arm tuning and dependencies. |
| [pipeline/capability.rs L175–270](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/capability.rs#L175-L270) | Availability + reproducible tracking_config_id/calibration_set_id | Include enabled new method version, XR config and valid active athlete/room/calibration identities. |
| [pipeline/features.rs L1–128](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/features.rs#L1-L128) | CameraImuFusion is reported "not implemented" despite partial real heading correction | Report XR Arm Constraints distinctly, without falsely declaring full-body fusion. |
| [pipeline/runner.rs L20–110](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/runner.rs#L20-L110) | Single runner with skeleton watch/stream state | Preserve ownership, no second engine. |
| [pipeline/runtime.rs L234–280, L408–484](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/pipeline/runtime.rs#L234-L484) | Accepted SourceEvents, solver ticks, next fusion state and frame/recording production | Advance arm continuity with existing fusion state, record event before replay, no wall-clock-dependent solver. |
| [recording/replay.rs L1–140](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/crates/bansayo-engine/src/recording/replay.rs#L1-L140) | Context restore/replay/seek through accepted events | Ensure time-history IK state rebuilt on seek and sequential replay. |
| [apps/vr/scripts/pose_buffer.gd L1–78](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/apps/vr/scripts/pose_buffer.gd#L1-L78) | 50ms presentation buffer, revision-safe interpolation, no extrapolation | Preserve; no Godot-side IK. |
| [apps/vr/scripts/body_root.gd](https://github.com/danielsobrado/Bansayo/blob/38e999487b3ed609a7a78b592ee133966b2f55c3/apps/vr/scripts/body_root.gd) | Engine content in XR stage, native headset camera independent | BodyRoot applies Phase 02 transform **once**; Phase 04 emits camera-world coordinates. |

### 1.3 Prerequisite symbols NOT present on inspected main

These are **planned by the earlier documents**; verify actual names after those phases are implemented:
- Phase 01: \`ObservationData::RigidPose\`, \`ObservationKind::RigidPose\`, \`DeviceKind::XrTrackedDevice\`, \`SensorCapability::RigidPose\`, and XR-HMD, XR-LEFT-GRIP, XR-RIGHT-GRIP device streams.
- Phase 02: \`XrAlignmentData\` with \`q_xr_stage_from_world\`, \`t_xr_stage_from_world_m\`, stage session/generation/room/fingerprint, and \`TimeSyncStatus\` for current mapped XR observations.
- Phase 03: \`BodyProportionsData\` measured individual left/right upper_arm and forearm lengths, \`XrGripWristOffsetData.r_grip_from_wrist_m\`, athlete+controller profile identity and validity.
- The code references in §1.2 identify **existing** modules; no false references to planned future code are used.

## 2. Architecture: one solver, a bounded optional arm stage

~~~text
Phase 01 XR HMD / LEFT-GRIP / RIGHT-GRIP observations
   |  Phase 02 engine session clock + verified XR stage to world
   |  Phase 03 athlete-scoped arm lengths + grip-to-wrist translations
   v
Registry XR RigidPose ring buffers (bounded per controller)
   |                    existing camera triangulation, t_cam
   |                      |  shoulder / elbow / root with QA
   |                      v
   +--> interpolate at t_ref --> transform to camera world wrists
                                  |
           existing baseline solve() (mounted IMU, camera fit, root)
                                  |
                        arm candidate + identity/time gate
                                  |
                         two-link closed-form IK
                           /              \
                  camera elbow plane      prior / fallback
                           \              /
                       IMU conflict arbitration
                                  |
                        final segment rotations
                                  |
                 existing forward_kinematics()
                                  |
                existing Pose + MotionSample + capability
                                  |
                       Godot / desktop read-only

NO new camera solver, XR transport, rig, database, full-body optimizer or renderer
~~~

### Non-negotiable invariants

1. **World metres:** Convert XR grip stage pose to verified camera world **before** IK. No visual/world double transform.
2. **Active person:** Body and wrist offset profiles must both match selected athlete and the current supported skeleton model. No profile sharing by display name.
3. **Current rig:** Stage session/generation, room, camera fingerprint, physical controller/grip revision match current Phase 02/03 evidence. Mismatches hard fail.
4. **One time:** All constraints use existing session-clock \`t_ref\`; compare against camera \`t_cam\` and XR bracketing timestamps. No receive-time-only wrist IK.
5. **One anatomy:** Fetch measured a/b from **current Skeleton segments**, never approximate from HMD height, controller span or a default ratio while claiming calibrated.
6. **One joint tree:** Update local arm world rotations, compute final positions **once** via current FK; no separate phantom elbows.
7. **No imaginary twist:** Bone directions fix swing; axial twist remains from real mounted IMU if valid, otherwise inferred.
8. **Hard fail on stale:** No extrapolation through missing XR poses; no held root, prior wrist, or old controller transforms used as fresh.
9. **Independent arms:** Left and right have distinct calibration, quality, continuity and failure state.
10. **Feature flag off is strict no-op:** Legacy camera/IMU-only fixtures, pose frame and recorded metadata are unchanged.

## 3. Contracts and new module boundaries

### 3.1 New shared identities (additive protocol-v1)

- \`MethodId::XrConstrainedIk\` in \`crates/bansayo-contracts/src/motion.rs\`: a calibrated two-link swing constraint using a trusted XR wrist target. It does **not** imply the anatomical wrist quaternion is measured.
- \`FeatureId::XrArmConstraints\` in \`crates/bansayo-contracts/src/quality.rs\`: independent status for optional arms, not full-body tracking.
- Reuse existing \`SegmentPose\` fields: \`rotation\`, \`position_m\`, \`provenance\`, \`confidence\`, \`sources\`, \`method\`, \`t_obs_us\`.
- Reuse existing \`BodyState.axis/twist\`, \`Evidence\`, \`TrackingQuality.reasons\` and \`CalibrationState\`. Do **not** modify \`ServerMessage::Frame\`, \`protocol_version=1\` or existing enum serialization names.

**Source identity:** \`XrDevice\` remains a direct XR tracked-device method, not an anatomical elbow. \`XrConstrainedIk\` means geometrically derived bone direction. \`Provenance::Derived\` requires measured, valid input constraints and a trustworthy bend plane; \`Provenance::Inferred\` is required for an anatomical/prior-only elbow plane.

### 3.2 Planned small files

**New \`crates/bansayo-engine/src/pipeline/xr_pose_align.rs\`:**

~~~rust
pub struct AlignedXrPose {
    pub position_stage: V3,
    pub rotation_stage_from_device: Rot,
    pub t_us: u64,
    pub bracket_us: u64,
}
pub fn rigid_pose_at(
    history: &[SensorObservation],
    t_ref: u64,
    max_gap_us: u64,
) -> Option<AlignedXrPose>;
~~~

Implementation may use an iterator instead of slice, but must be pure, accept **only** phase-01 atomic XR poses, validate same epoch, SLERP using existing \`align::slerp\`, LERP translation, and **never extrapolate**.

**New \`pipeline/xr_wrist.rs\`:**

~~~rust
pub struct WristTarget {
    pub side: ArmSide,
    pub position_world: V3,
    pub t_obs_us: u64,
    pub sources: Vec<DeviceId>,
    pub confidence: Option<f32>,
}
pub fn target_from_grip(
    aligned: &AlignedXrPose,
    alignment: &XrAlignmentData,
    offset: &XrGripWristOffsetData,
    quality: &TimeSyncStatus,
    config: &XrArmFusionConfig,
) -> Result<WristTarget, XrArmReject>;
~~~

The world conversion is **exactly**:

~~~text
R_world_from_stage = inverse(R_stage_from_world)
p_world_grip = R_world_from_stage*(p_stage_grip - t_stage_from_world)
R_world_from_grip = R_world_from_stage*R_stage_from_grip
p_world_wrist = p_world_grip + R_world_from_grip*r_grip_from_wrist
~~~

The Phase 02 \`r_grip_from_tip\` physical pointer is **NOT** the Phase 03 \`r_grip_from_wrist\`. No finger rotation, palm normal or pronation offset is inferred.

**New \`pipeline/arm_ik.rs\`:** pure analytic two-link solver and world rotations with no registry/network dependencies.

**New \`pipeline/arm_evidence.rs\`:** camera-root/shoulder/elbow QA, athlete/profile gate and per-arm IMU/XR arbitration.

**New \`pipeline/arm_state.rs\`:** deterministic bounded per-arm mode and bend-plane continuity, with reset key.

\`pipeline/fusion.rs\` extends the current \`FusionState\` with **two fixed arm-state slots**. \`pipeline/solve.rs\` remains orchestrator and calls these tiny pure helpers **after baseline rotations/root** and **before FK**. Keep files small; split a >250-line new module by responsibility rather than creating a gigantic new solve().

## 4. Temporal and spatial prerequisites (fail closed)

### 4.1 One existing reconstruction time

Current \`solve.rs\` uses \`t_ref = t_imu.or(t_cam)\`. **Preserve this logic when fusion.xr_arms.enabled=false**.

When enabled:
1. If \`tracking.mode\` is not \`hybrid_auto\`, skip XR constraints; \`camera_only\` and \`imu_only\` retain their established semantics.
2. If no \`t_ref\`, skip all arm IK. Phase 04 does not bootstrap a whole-body root from OpenXR.
3. Require **fresh camera root measured on this tick**, not previous camera anchor, inferred root or floor-contact root. Never shift pelvis to satisfy wrist.
4. Require camera observation \`t_cam\` within **20 ms** of \`t_ref\` for root/shoulder and optical bend-plane evidence. Otherwise skip arm IK entirely. This prevents applying a new wrist against stale shoulder/root pose.
5. Require a current valid Phase 02 \`T_stage_from_world\` and matching live stage identity, room and camera fingerprint.
6. Require Phase 02 four-timestamp estimator \`settled=true\`, current uncertainty <=5000us, XR \`TimeQuality::Estimated\` (or externally validated Synchronized). \`ReceiveTime\` **is insufficient** for arm IK during fast motion.
7. Retrieve each XR grip RigidPose **bracketing** \`t_ref\`; maximum bracket 40ms, no extrapolation. Require tracking valid and no device epoch change. Maximum sample age 100ms.
8. Require an athlete-matched **measured** upper-arm and forearm pair and a valid, controller-matched grip→wrist calibration. Assumed lengths are allowed for the **legacy model only**, not marked as a measured XR IK constraint.
9. Require finite unit quaternion, finite position within Phase 01 configured bounds and matching grip side (XR-LEFT-GRIP with left arm, XR-RIGHT-GRIP with right).
10. If the real camera's exposure timestamp is not verifiably available, label temporal accuracy **unverified** in hardware QA even if engine clock skew <20ms.

### 4.2 RigidPose ring buffer

Phase 01 adds \`ObservationKind::RigidPose\`, but latest accepted pose alone cannot bracket time.

In \`registry/mod.rs::Entry\` add \`rigid_poses: VecDeque<SensorObservation>\` capacity **64**. Only XR device descriptors with \`SensorCapability::RigidPose\` can use it. Accept sorted monotonic \`t_us\` and \`seq\` under current Registry rules. On \`DeviceLink(false)\`, \`DeviceLink(true)\`, XR stage-session change, tracking loss, engine/replay reset: **clear** buffer and prior mode. Do not interpolate across disconnected spans. All three Phase 01 streams are separate devices and have independent monotonic sequence.

For XR interpolation, if exact t_ref exists, return exact sample; otherwise require before <= t_ref < after and after-before <=40,000us, interpolate pos linearly and quaternion with shortest-path SLERP. Unknown tracker validity or any nonfinite result returns None. Bounded 64 items is enough for jitter at 60–120Hz; trim by count, not unlimited time.

### 4.3 Camera elbow source and strict evidence

The existing COCO17 index mapping is:
- left: shoulder 5, elbow 7, wrist 9;
- right: shoulder 6, elbow 8, wrist 10.

Use a **single shared semantic extractor** from \`pipeline/skeleton_fit.rs\` or its \`pipeline/motion.rs\` canonical mapping; don't duplicate hardcoded mapping in two files. Only \`PoseLandmarks3D::Triangulated\` **FrameRef::World** (metric) accepted; reject \`ModelRelative\`, unknown schemas and one-camera guess.

**Optical elbow gate per side:**
- Shoulder, elbow and (when present) camera wrist finite in metres.
- Shoulder/elbow triangulation each has >=2 distinct confirmed cameras, confidence >=0.70, reprojection RMS <=5px.
- Optional propagated \`sigma_m\` <=0.05m if present; missing sigma is not automatically a validated error bound.
- Camera \`t_cam\` <=20ms from \`t_ref\`; camera calibration/profile identities current.
- Camera shoulder vs current FK shoulder separation <=0.08m. If larger, reason \`camera_shoulder_mismatch\` and keep baseline, **do not rotate root to fit**.
- Camera elbow-to-shoulder length deviation from measured upper-arm length <=0.07m, otherwise use prior bend (if any) rather than accepting a bad camera elbow.
- If both camera wrist and XR wrist are qualified and differ >0.08m, classify \`xr_camera_wrist_disagree\`; do not silently choose one and mark it high-confidence.

Camera elbow supplies primarily a **bend-plane direction**, not a hard elbow point. It can be noisy or momentarily occluded while the XR wrist is still accurate.

## 5. Exact two-link elbow IK algorithm

### 5.1 Input geometry

Each side's inputs in **camera world metres**:
- \`S\` = shoulder joint from current FK of **measured camera-root** + torso/clavicle orientations.
- \`W\` = current time-aligned XR-derived anatomical wrist target.
- \`a\` = measured upper-arm length, norm of active Skeleton upper-arm tail.
- \`b\` = measured forearm length, norm of active Skeleton forearm tail.
- \`E_camera\` = optional quality-gated camera elbow point.
- \`bend_hint\` = selected plane cue (camera, prior or anatomical).
- \`q_clavicle_world\` = existing parent world orientation.
- \`t_ref\` = reconstruction tick in engine session clock.

Reject nonfinite S/W, lengths <=0, d<0.01m and absent validated profile identity. No medical/anatomical claim beyond the 2-link approximation.

### 5.2 Closed form

~~~text
D = W - S
d = ||D||
u = D / d

x = (a*a - b*b + d*d) / (2*d)
h2 = a*a - x*x
h = sqrt(max(0, h2))

v = normalize(bend_hint - u * dot(bend_hint,u))
E = S + x*u + h*v

||E-S|| = a
||W-E|| = b
~~~

**Reachability:** physically \`abs(a-b) <= d <= a+b\`. With a numeric epsilon 0.001m:
- If outside by **>0.04m**, reject IK and retain baseline with \`arm_target_unreachable\`.
- If outside by <=0.04m, clamp d to the nearest safe endpoint to form **W_solve=S+clamp(d)*u**, solve against W_solve; publish residual **abs(d-clamp(d))**, never claim raw wrist is exactly reached.
- When floating roundoff makes h² slightly negative (>=-1e-8), clamp h=0; more negative fails.
- Near full extension \`h<0.01m\` makes bend plane unobservable; hold latest short-lived plane or return an inferred stable fallback without a sudden flip.
- Check FK lengths against \`a,b\` to 0.002m tolerance; for a clamped wrist measure residual to **W_solve**, separately record raw-target residual.
- No blend of endpoints, no multi-iteration global optimizer and no fixed midpoint assumption.

**Deterministic numeric fixture:**

~~~text
S = (0, 0, 0)
W = (0.45, 0, 0)
a = 0.30
b = 0.25
bend_hint = (0, 0, 1)

d = 0.45
x = (0.09-0.0625+0.2025)/(0.90) = 0.255555...
h = sqrt(0.09 - x*x)
E = (x, 0, h)
assert ||E-S|| == 0.30 and ||W-E|| == 0.25
~~~

Also test identity, 90° world yaw, opposite side, straight extension, collapsed near-equal-length target, unreachable target, vertical wrist, finite/zero vectors and antialigned rotations.

### 5.3 Elbow bend-plane priority

Obtain orthogonal \`v\` in this **exact** priority:

1. **Camera elbow:** \`bend = E_camera - S\`, project against u: \`bend_perp = bend - dot(bend,u)*u\`. Require projected norm >=0.03m, optical QA passed, physically plausible upper-arm radius and no high-confidence XR/camera wrist conflict. Normalize to obtain v. Source \`camera\`.
2. **Recent prior plane:** if same athlete/room/stage/profile key and prior accepted \`last_bend\` age <=150ms, reproject perpendicular to current u; if projected norm >=0.10, normalize. Source \`prior\`, **inferred**.
3. **Anatomical fallback:** chest/world forward vector projected perpendicular to u, then chest-up if degenerate, then deterministic world axis least parallel to u. Source \`anatomical\`, **inferred**.

Do not change elbow side randomly: if a new camera plane is opposed to prior (dot<0), demand **three consecutive qualified optical frames** with same new hemisphere and plausible geometry before switching; until then retain prior or fallback with \`elbow_plane_ambiguity\`. When a genuine arm crosses extension, a switch can happen after confirmation. Maximum plane angular rate **360°/s**, maximum one-tick step **45°**, both configuration values. A plane is still just an estimate; do not report high accuracy from an inferred fallback.

### 5.4 Arm rotations and forward kinematics

The Bansayo rest model has upper-arm rest tails along left +X/right -X; child forearm offset equals upper tail, hand offset equals forearm tail.

Let \`upper_dir = normalize(E-S)\` and \`forearm_dir = normalize(W_solve-E)\`.

~~~text
R_upper = smallest_arc(R_clavicle*upper_rest_tail, upper_dir) * R_clavicle
R_forearm = smallest_arc(R_upper*forearm_rest_tail, forearm_dir) * R_upper

FK_elbow = S + R_upper * forearm_offset
FK_wrist = FK_elbow + R_forearm * hand_offset
~~~

**Do not rotate pelvis, chest, clavicle or camera to force a wrist match.** Use deterministic parent axis for 180° antiparallel swing, not arbitrary quaternion signs. Check FK_elbow≈E and FK_wrist≈W_solve before accepting a candidate. Store world rotations exactly as current \`SkeletonPose\` expects. BodyState twist stays **inferred** unless a valid independent mounted IMU observes it; XR controller orientation is a grip pose, not measured anatomical forearm pronation.

### 5.5 No fabricated shoulder / head

A reliable world root and torso rotation place the **model shoulder**. If camera shoulder disagrees by >8cm, abstain rather than stretching shoulder geometry. Do not align the body to the HMD. A valid XR-HMD pose remains available as a device observation for later calibrated head-center work; this Phase 04 plan deliberately does not equate eye centre with skull/joint centre.

## 6. Mounted IMU arbitration and confidence

The current solver is **IMU-first** on valid mounted segment orientations. Retain that deterministic baseline.

| Available arm data | Exact rule |
|---|---|
| Feature off | Baseline solve output byte/semantically unchanged. |
| XR/room/time/body invalid | No IK; preserve baseline and report reason only when enabled. |
| No valid mounted IMUs on either upper/forearm | Full two-link IK if camera-root, calibrated lengths, XR target, valid bend and all gates pass. |
| Both upper-arm and forearm mounted IMUs valid | **Preserve both rotations**. Compute FK predicted wrist vs XR wrist. If <=0.05m, report agreement, no overridden rotation; else \`imu_xr_wrist_disagree\`. |
| Upper-arm IMU valid, forearm IMU absent | Preserve upper-arm rotation and FK elbow; if \`abs(||W-E||-b)<=0.05m\`, solve **forearm swing only** and mark derived; else preserve baseline and report. |
| Forearm IMU valid, upper-arm absent | Preserve forearm orientation. Candidate upper swing only if it can satisfy wrist target with <=0.05m positional residual and the resulting forearm direction agrees with measured forearm IMU within **15°**. Else leave baseline. |
| One IMU stale/unmounted/low quality | Apply existing mount/recency gates; only allow XR IK where no **valid** mounted segment orientation remains. Never contradict existing baseline validity silently. |
| Optical elbow but no valid XR wrist | Existing camera fit, not Phase 04 IK. |
| XR wrist but elbow visually occluded | Use prior bend max 150ms or inferred anatomical fallback; don't label elbow observed. |
| Wrist unreachable / time skew / stage recenter | No IK. Other side can continue unaffected. |

**No weighted average of disagreeing cameras/IMUs/XR quaternions** for this phase. Confidence-aware means hard input validity, transparent residuals, per-source provenance and conservative arbitration, not uncalibrated statistical probabilities.

### 6.1 Confidence and observed[]

- OpenXR tracking=true with no accuracy model is **not confidence=1.0**. Keep \`confidence=None\` if no defensible numeric confidence.
- With reliable camera elbow, calibrated geometry and XR wrist, arm-axis provenance **Derived**, method \`XrConstrainedIk\`, and \`observed=true\` only for axes that are sufficiently constrained. Numerical quality can use min(qualified camera shoulder, camera elbow detector confidence) times deterministic freshness/residual penalties. It is a **relative quality indicator**, not calibrated probability.
- With prior/anatomical plane and no current camera elbow, geometry may be useful, but bend-dependent arm direction provenance is **Inferred** and \`observed=false\` for elbow-joint measurement. Otherwise \`pipeline/joints.rs\` would invent a numeric elbow angle.
- A current valid mounted IMU retains its existing \`ImuMounted\` or \`ImuOpticalFused\` method, source and twist evidence; do not relabel it as XR simply because the wrist agrees.
- Camera-only existing \`SkeletonFitAxis\` stays derived when qualifying observed evidence exists.
- Source IDs on a constrained segment include contributing \`XR-LEFT/RIGHT-GRIP\` plus camera IDs where a camera plane was used; sort and deduplicate. Calibration profile identity participates in \`calibration_set_id\`, not fake DeviceId strings.
- Set \`t_obs_us\` to the session time of the **oldest accepted contributing** observation, not engine render time; no later than t_ref. Unknown source timing must not acquire a made-up observation timestamp.
- The training \`BodyState.axis\` follows same method/provenance, \`twist\` stays inferred unless real IMU supplies it. Propagated sigma stays \`SigmaBasis::Propagated\`, not \`Validated\`.

## 7. Bounded continuity, hysteresis and reset behavior

Add per-side \`ArmConstraintState\` in the **existing** FusionState with exactly two arm slots (left/right), not a per-user unbounded map.

~~~rust
pub enum ArmConstraintMode {
    Baseline,
    XrTwoLink,
    XrForearmOnly,
    XrUpperOnly,
    ImuValidated,
}
pub enum BendOrigin { Camera, Prior, Anatomical }

pub struct ArmConstraintState {
    pub mode: ArmConstraintMode,
    pub last_bend_world: Option<V3>,
    pub bend_t_us: Option<u64>,
    pub last_wrist_world: Option<V3>,
    pub last_t_ref_us: Option<u64>,
    pub eligible_frames: u8,
    pub failed_frames: u8,
    pub opposite_plane_frames: u8,
    pub calibration_key: Option<ArmCalibrationKey>,
}
~~~

\`ArmCalibrationKey\` hashes selected athlete ID, skeleton model/revision, body-proportion profile content, wrist-offset profile content, XR alignment fingerprint, room, stage ID/generation and physical grip revision. Use the existing canonical hashing helper and include actual content hash rather than only profile IDs. Do not embed private athlete name in logs.

**Switch policy:**
- Enter a new XR mode only after **3 consecutive eligible** frames; meanwhile show normal baseline output.
- Exit after **2 consecutive soft failures** (occlusion, low confidence). During a soft-failure frame do **not** synthesize another wrist position from a stale target: baseline output may be used immediately while the mode label ages.
- Exit **immediately** on hard invalidation: stage recenter, device disconnect, profile/athlete/room/model changed, nonfinite transform, recording mode reset or missing camera root.
- Camera optical confidence enter >=0.70, stay >=0.50 if all remaining hard gates pass. Losing camera elbow may select recent plane only; losing camera root always disables IK.
- Reject apparent wrist displacement >6.0m/s between valid t_ref frames with \`xr_wrist_jump\` as provisional outlier gate, configurable for martial-arts testing rather than claiming a physiological maximum.
- Prior bend expires after 150ms, even if the controller keeps streaming.
- Never maintain a cross-session or cross-athlete prior.
- No observer switch may create a >45° new elbow-plane step in one tick; max 360°/s when updating.

**Replay caveat:** The existing Registry has \`fusion\` state advanced with each tick. The replay engine must replay the **same tick sequence** or restore checkpoints that include the arm continuity state. If random seek currently reconstructs only SourceEvents without equivalent solver ticks, deterministic seek will fail; correct this rather than silently dropping bend history. Do not persist arbitrary Godot transient poses as source truth.

## 8. Configuration: strict YAML, disabled by default

Add \`fusion.xr_arms\` as a nested \`XrArmFusionConfig\` in the already existing \`FusionConfig\`, \`serde(default, deny_unknown_fields)\`. All algorithm thresholds below are centralized in the config defaults and validated with path-qualified errors in \`config/validate.rs\`. The existing \`fusion\` heading/root options remain unchanged.

~~~yaml
schema_version: 2
tracking:
  mode: hybrid_auto
fusion:
  optical_heading_correction: true
  max_heading_correction_deg_s: 15.0
  heading_time_constant_s: 1.0
  heading_outlier_deg: 30.0
  heading_min_tilt_deg: 20.0
  min_optical_confidence: 0.5
  anchor_speed_bound_m_s: 3.0
  imu_camera_disagree_deg: 45.0
  xr_arms:
    enabled: true
    max_camera_to_ref_skew_ms: 20
    max_xr_pose_bracket_ms: 40
    max_xr_pose_age_ms: 100
    max_clock_uncertainty_us: 5000
    xr_history_capacity: 64
    min_camera_point_confidence_enter: 0.70
    min_camera_point_confidence_stay: 0.50
    min_camera_views: 2
    max_camera_reprojection_error_px: 5.0
    max_camera_sigma_m: 0.05
    max_camera_shoulder_error_m: 0.08
    max_camera_elbow_length_error_m: 0.07
    max_xr_camera_wrist_disagreement_m: 0.08
    min_target_distance_m: 0.01
    max_unreachable_tolerance_m: 0.04
    reach_epsilon_m: 0.001
    singular_bend_height_m: 0.01
    min_optical_bend_projection_m: 0.03
    min_prior_bend_projection_m: 0.10
    prior_bend_max_age_ms: 150
    max_bend_rate_deg_s: 360.0
    max_bend_step_deg: 45.0
    enter_good_frames: 3
    exit_bad_frames: 2
    opposite_bend_confirm_frames: 3
    max_fk_residual_m: 0.002
    imu_wrist_agreement_m: 0.05
    forearm_direction_agreement_deg: 15.0
    max_wrist_speed_m_s: 6.0
~~~

Default \`fusion.xr_arms.enabled=false\`. This snippet is an **XR-enabled section**, not by itself a full runtime config with actual camera/IMU/XR sources; the implementing AI must extend a verified complete Phase 01/02/03 example instead of checking an invalid YAML sample and declaring it ready.

**Validation:** strict ranges for all numeric values, finite thresholds, \`0<stay<=enter<=1\`, history capacity <=128, \`min_camera_views>=2\`, \`enter_good_frames>=1\`, \`exit_bad_frames>=1\`, \`opposite_bend_confirm_frames>=2\`, max clock/skew consistent with Phase 02 and max history gap, \`max_fk_residual_m\` <= \`max_unreachable_tolerance_m\`. Explicit XR-arm enabled configuration must be rejected under schema_version=1 or without required XR source config. Static config validates presence of XR support; **live calibration readiness remains dynamic**, not assumed from YAML.

### 8.1 Reproducibility identity

Current \`fusion::METHOD_VERSION\` is \`bansayo.fusion.anchored_fk.v1\`. Keep original identity for disabled old behavior; when XR arms enabled include **\`bansayo.fusion.xr_arms_ik.v1\`** in \`pipeline/capability.rs::identity_bodies\` plus all xr_arms settings, active athlete/skeleton geometry, XR alignment calibration content hash and current wrist-offset/body profiles. Current \`calibration_set_id\` builder filters profiles by subject.device_id and would **omit Phase 03 athlete-only body measurements**: fix this explicitly while retaining legacy behavior when feature is off. A calibration change must alter comparison identity; only current valid profiles count.

## 9. Implementation file manifest

| File | Operation | Precise responsibility |
|---|---|---|
| \`crates/bansayo-contracts/src/motion.rs\` | Edit | New \`MethodId::XrConstrainedIk\`, additive enum serialization. |
| \`crates/bansayo-contracts/src/quality.rs\` | Edit | New \`FeatureId::XrArmConstraints\`. |
| \`crates/bansayo-contracts/tests/contracts.rs\` | Edit | Golden serde/protocol backward compatibility. |
| \`crates/bansayo-contracts/tests/support/examples.rs\` | Edit | Samples for constrained method, tracking status, inferred fallback. |
| \`crates/bansayo-contracts/tests/support/generated.rs\` | Edit | Generate TypeScript with BANSAYO_BLESS, no hand editing. |
| \`crates/bansayo-engine/src/config/schema.rs\` | Edit | Typed nested XrArmFusionConfig default false. |
| \`crates/bansayo-engine/src/config/validate.rs\` | Edit | Strict numeric, cross-field, prerequisite validation. |
| \`crates/bansayo-engine/src/registry/mod.rs\` | Edit | Bounded XR rigid pose history, link/reset cleanup. |
| \`crates/bansayo-engine/src/pipeline/xr_pose_align.rs\` | **New** | Interpolate rigid position+rotation with no extrapolation. |
| \`crates/bansayo-engine/src/pipeline/xr_wrist.rs\` | **New** | Convert aligned grip pose and per-athlete local offset into metric world wrist target. |
| \`crates/bansayo-engine/src/pipeline/arm_ik.rs\` | **New** | Two-link geometry, reachability, swing rotations and FK residual. |
| \`crates/bansayo-engine/src/pipeline/arm_evidence.rs\` | **New** | Optical points, room/clock/profile gates, mounted IMU arbitration. |
| \`crates/bansayo-engine/src/pipeline/arm_state.rs\` | **New** | Bend plane continuity, source hysteresis and reset key. |
| \`crates/bansayo-engine/src/pipeline/fusion.rs\` | Edit | Add fixed two-arm state, preserve heading and camera root. |
| \`crates/bansayo-engine/src/pipeline/solve.rs\` | Edit | One optional orchestrator call after baseline rotations/root, before FK. |
| \`crates/bansayo-engine/src/pipeline/mod.rs\` | Edit | Exports for small new modules. |
| \`crates/bansayo-engine/src/pipeline/motion.rs\` | Edit | Accurate axis/twist/provenance and inferred keypoint labels. |
| \`crates/bansayo-engine/src/pipeline/joints.rs\` | Verify/edit as necessary | Inferred elbows produce null numeric joint angles. |
| \`crates/bansayo-engine/src/pipeline/capability.rs\` | Edit | XR fusion method/config/athlete profiles in tracking and calibration identity. |
| \`crates/bansayo-engine/src/pipeline/features.rs\` | Edit | Correct XR Arm Constraints status, no false full-body fusion claim. |
| \`crates/bansayo-engine/src/pipeline/runtime.rs\` | Edit | Publish per-arm new fusion state, reset on context/profile changes. |
| \`crates/bansayo-engine/src/recording/replay.rs\` | Edit if tests expose gap | Deterministic same-tick replay and seek rebuilding bend-plane state. |
| \`crates/bansayo-engine/tests/xr_arm_ik.rs\` | **New** | Unit geometry, fallback and source conflict matrix. |
| \`crates/bansayo-engine/tests/xr_arm_fusion.rs\` | **New** | End-to-end deterministic events, profile readiness, replay, perf smoke. |
| \`apps/vr/scripts/status_panel.gd\` | Small edit | Optional XR arm constrained/degraded reason and per-side status; no IK math. |
| \`apps/vr/tests/xr_arm_fusion_smoke.gd\` | **New** | Headless UI with simulated state/pose, false-measurement prevention. |
| \`apps/desktop/src/viewport/SkeletonRig.ts\` | Verify/edit only if necessary | Display current engine segment poses without duplicate IK. |
| \`docs/05-calibration-and-fusion.md\` | Edit after implementation | Describe actual fusion, gates and limitations. |
| \`docs/13-implementation-status.md\` | Edit after implementation | Link test results and real hardware status. |
| \`.github/workflows/ci.yml\` | Edit | Add Rust/Godot regression tests with pinned tools. |

**Leave alone:** OpenXR server transport and StageInfo contract, camera detectors/triangulation mathematics, ChArUco calibration geometry, independent IMU mounting solver, body proportions calibration process, avatar mesh/rig assets, training workouts/kata scoring and schema-1 fixture behavior.

## 10. Precise tests: geometry, timing, conflicts and replay

### 10.1 Analytic IK geometry (pure unit)

- [ ] \`ik_known_lengths_0p30_0p25_and_d0p45\`: computed E satisfies both lengths within 1e-9 f64.
- [ ] \`ik_left_right_rest_axes_have_consistent_mirror\`.
- [ ] \`ik_world_yaw90_and_translation_are_invariant\`.
- [ ] \`ik_exact_full_extension_is_finite_no_plane_flip\`.
- [ ] \`ik_near_collapsed_equal_length_arm_no_division_by_zero\`.
- [ ] \`ik_unreachable_4cm_tolerance_clamped_and_residual_reported\`.
- [ ] \`ik_unreachable_beyond_4cm_rejected_without_changing_baseline\`.
- [ ] \`ik_rejects_nonfinite_zero_and_negative_lengths\`.
- [ ] \`ik_antiparallel_swing_uses_deterministic_axis\`.
- [ ] \`ik_camera_elbow_plane_perpendicular_projection\`.
- [ ] \`ik_observed_camera_elbow_outlier_above_7cm_rejected\`.
- [ ] \`ik_previous_plane_150ms_expiry_and_reprojection\`.
- [ ] \`ik_opposite_bend_requires_three_qualified_frames\`.
- [ ] \`ik_bend_rate_limit_and_single_step_45deg\`.
- [ ] \`ik_parent_clavicle_rotation_unmodified\`.
- [ ] \`ik_fk_reconstructs_elbow_and_wrist_2mm_tolerance\`.
- [ ] \`ik_output_quaternions_unit_and_nan_free\`.

### 10.2 XR transform / clock / input

- [ ] \`xr_wrist_identity_stage_calibrated_local_offset\`.
- [ ] \`xr_wrist_stage_yaw90_sign_and_inverse_transform\`.
- [ ] \`xr_wrist_left_and_right_device_ids_never_swap\`.
- [ ] \`xr_pose_interpolation_two_positions_and_shortest_slerp\`.
- [ ] \`xr_pose_single_exact_t_ref_accepts\`.
- [ ] \`xr_pose_no_extrapolation_before_or_after_history\`.
- [ ] \`xr_pose_gap_gt40ms_rejected\`.
- [ ] \`xr_pose_ring64_bounded_and_reconnect_clears\`.
- [ ] \`xr_pose_tracking_loss_cannot_bridge_history\`.
- [ ] \`xr_clock_receive_time_only_cannot_drive_ik\`.
- [ ] \`xr_clock_uncertainty_gt5ms_and_not_settled_rejected\`.
- [ ] \`xr_stage_changed_and_room_or_camera_fingerprint_changed_rejected\`.
- [ ] \`xr_athlete_profile_cross_contamination_rejected\`.
- [ ] \`xr_assumed_body_length_never_claims_calibrated_ik\`.
- [ ] \`xr_camera_time_skew_gt20ms_preserves_baseline\`.
- [ ] \`xr_camera_root_held_or_floor_contact_preserves_baseline\`.

### 10.3 Sensor conflicts / truthfulness

- [ ] \`fusion_disabled_mode_matches_old_golden_frame_and_motion\`.
- [ ] \`fusion_tracking_mode_camera_only_ignores_xr_constraints\`.
- [ ] \`fusion_tracking_mode_imu_only_ignores_xr_constraints\`.
- [ ] \`fusion_two_valid_imus_are_preserved_even_on_xr_disagreement\`.
- [ ] \`fusion_upper_imu_preserved_forearm_ik_only_if_reachable\`.
- [ ] \`fusion_forearm_imu_preserved_upper_only_if_direction_compatible\`.
- [ ] \`fusion_low_quality_camera_elbow_reverts_to_prior_inferred\`.
- [ ] \`fusion_anatomical_plane_not_marked_measured_or_observed\`.
- [ ] \`fusion_inferred_elbow_joint_angle_is_null_with_reason\`.
- [ ] \`fusion_imu_twist_retained_and_xr_grip_twist_not_invented\`.
- [ ] \`fusion_wrist_jump_6m_s_emits_reason_no_pose_snap\`.
- [ ] \`fusion_left_failure_does_not_disable_right\`.
- [ ] \`fusion_three_good_frames_to_enter_two_to_exit_soft_mode\`.
- [ ] \`fusion_recenter_and_athlete_switch_clear_prior_immediately\`.
- [ ] \`fusion_sources_t_obs_us_and_provenance_correct\`.
- [ ] \`fusion_capability_identity_changes_with_profile_content_same_id\`.
- [ ] \`fusion_feature_status_only_reports_arm_constraints_not_full_body\`.

### 10.4 Persistence/replay and no regressions

- [ ] \`replay_recorded_xr_camera_imu_events_identical_sequential_ticks\`.
- [ ] \`replay_seek_rebuilds_same_bend_history_at_target_frame\`.
- [ ] \`replay_new_athlete_or_body_profile_outside_recording_has_no_effect\`.
- [ ] \`replay_stage_recenter_and_disconnect_reset_fusion_state\`.
- [ ] \`replay_legacy_recording_without_new_evidence_loads_unchanged\`.
- [ ] \`camera_only_existing_coco17_and_triangulation_fixtures_unchanged\`.
- [ ] \`imu_only_existing_mounting_and_heading_fixtures_unchanged\`.
- [ ] \`camera_plus_imu_existing_rung1_root_and_heading_behavior_unchanged_when_off\`.
- [ ] \`full_pose_has_no_nonfinite_world_values_and_same_skeleton_rev\`.
- [ ] \`replay_and_live_method_provenance_metadata_equal\`.
- [ ] \`performance_no_network_or_unbounded_heap_on_ik_hot_path\`.

### 10.5 Exact validation commands

After checking *actual* prerequisite code/test paths exist:

~~~bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
BANSAYO_BLESS=1 cargo test -p bansayo-contracts
cargo test -p bansayo-contracts
cargo test -p bansayo-engine --test xr_arm_ik
cargo test -p bansayo-engine --test xr_arm_fusion
cargo test --workspace
node scripts/verify-gate1.mjs
node scripts/verify-gate2.mjs
godot --headless --path apps/vr --editor --quit
godot --headless --path apps/vr --xr-mode off --script tests/contract_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/spatial_ui_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/xr_observation_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/xr_alignment_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/body_calibration_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/xr_arm_fusion_smoke.gd
~~~

A tool or prerequisite file absent from the actual implementation baseline is **BLOCKED**. Do not create a fake test that always passes, hand-edit generated TypeScript, skip tests quietly or claim hardware testing from synthetic geometry. Run contract tests a second time **without BANSAYO_BLESS** and check no regenerated changes in working tree.

## 11. Hardware validation: independent physical measurements

Write results to \`docs/validation/YYYY-MM-DD-phase04-xr-arm-fusion.md\` with before/after commit, camera models/FPS, headset/controller, active athlete ID (not name), calibration fingerprints, stage IDs, source timestamp quality, confidence, observed physical setup and raw error summary.

1. Confirm phases 01–03 on real hardware first: XR head and both grips tracked, multi-camera world calibration, current athlete-specific arm lengths and grip-to-wrist offsets. No fake calibration data.
2. Two or more calibrated cameras view shoulders/elbows/wrists; establish board room and physical reference floor marks. Record actual camera reprojection error and known frame time uncertainties.
3. Perform 10s static T-pose, then slow elbow extension/flexion with both controllers and cameras; measure shoulder→elbow and elbow→wrist constancy. Compare elbow against independent physical marker/tape setup **not used for fitting**; report per-side median and p95 3D error.
4. Hold elbows partially flexed; temporarily occlude one optical elbow, retain XR wrist, verify no 180° flip, explicit prior/fallback provenance, and immediate restoration after camera sight returns.
5. Deliberately bend across full extension and verify the algorithm can change bend-plane hemisphere without locking forever or snapping one frame.
6. Deliberately rotate the wrist/grip while holding elbow still; confirm only calibrated wrist **translation** affects IK, not a fabricated palm orientation.
7. Test a controller outside physical arm reach, slow then fast; ensure finite pose and no body root chase. Record clamped residual and rejection count.
8. Attach working upper-arm/forearm SlimeVR devices and calibrate mounting. Verify IMU+XR disagreement is reported and valid IMU orientations retained. Remove one IMU, validate only the unsupported link can be constrained.
9. Recenter HMD; move camera or change room calibration; switch athlete, restart one controller and restart engine. Every mismatch must revoke or block old XR IK immediately.
10. Record and replay a deterministic sequence (standing, T-pose, elbow bends, fast punches/guard, arm occlusion) and compare per-frame source/method/positions and joint angles to original. Random seek must match sequential replay, not just eventually converge.
11. Run slow punches, guards, hooks and knees with strong arm accelerations; assess **temporal misalignment** and wrong-ID/occlusion effects. A 6m/s outlier threshold might reject legitimate fast motion; report false rejects and revise only with measured evidence.
12. Report p50/p95 added solver time, full solver time, render delay separately, camera time uncertainty, clock RTT/uncertainty, wrist residual, elbow error and number of inferred frames. Do not call an input-to-screen metric "motion-to-photon" without headset timing measurement.
13. Run camera-only, IMU-only and legacy simulation. Confirm no regression and exact feature-off equivalence.
14. Mark each item PASS/FAIL/BLOCKED. **Physical accuracy target <=5cm is provisional** and only meaningful with a trustworthy independent reference.

**Known hardware limitation:** existing \`sources/camera/timing.rs\` tracks arrival intervals, not necessarily exposure start; the Phase 02 four-timestamp estimator maps Godot **polling** time, not guaranteed OpenXR sensor exposure time. Physical fast-motion accuracy therefore needs independent QA. Report the uncertainty, do not mask it with a synchronized flag.

## 12. Seven direct-to-main implementation commits, in exact order

### Commit 01 — prerequisites, contracts and YAML

- [ ] Fetch latest \`main\`; record HEAD and actual code mapping. Verify Phase 01/02/03 **implemented and tested**, not just docs.
- [ ] Add \`MethodId::XrConstrainedIk\`, \`FeatureId::XrArmConstraints\`; regenerate TS/golden.
- [ ] Add nested disabled-by-default \`XrArmFusionConfig\`, strict validation and complete runtime YAML example.
- [ ] Verify old config schema 1, protocol 1, existing camera/IMU test outputs.
- **Commit message:** \`contracts,config: define optional XR arm constraints and quality gates\`.
- **Gate:** no existing wire contract changed and feature-off behavior matches prior golden results.

### Commit 02 — bounded XR pose interpolation and world wrist

- [ ] Add 64-sample XR pose histories to Registry and reset on disconnect/session.
- [ ] Add pure \`xr_pose_align.rs\` and \`xr_wrist.rs\` with no extrapolation and correct frame transform.
- [ ] Implement Stage/athlete/room/fingerprint/clock and controller side guard.
- [ ] Test identity/yaw90, time skew, untracked data, identity mismatch, reconnect and staged calibration.
- **Commit message:** \`engine: align OpenXR grip poses and derive calibrated world wrist targets\`.
- **Gate:** no raw XR-stage point can reach the camera-world IK stage untransformed.

### Commit 03 — closed-form elbow IK and numerical safety

- [ ] Add pure \`arm_ik.rs\` and bend-plane math with all finite/reachability gates.
- [ ] Implement measured-current-length FK-consistent segment swing and 180° deterministic axis.
- [ ] Implement optical/prior/anatomical bend-plane policy and time/rate guards in pure module.
- [ ] Unit test geometry, unreachable/near-singular, left/right mirrors, held-out camera plane.
- **Commit message:** \`engine: solve two-link elbow geometry with calibrated lengths and safe bend planes\`.
- **Gate:** f64 length residuals and output quaternion validity pass; inferred source never reports observed camera elbow.

### Commit 04 — IMU arbitration and integrated solve

- [ ] Add \`arm_evidence.rs\` and exact §6 sensor rule table.
- [ ] Insert one optional arm stage in existing \`pipeline/solve.rs\` after baseline camera/IMU/root and before FK.
- [ ] Maintain existing heading estimator/root anchor unchanged. Preserve observed/method/age per arm.
- [ ] Integrate canonical \`motion.rs\` evidence, \`joints.rs\` null-on-inferred and new feature status.
- [ ] Run camera-only, IMU-only, mixed and no-XR regression suite.
- **Commit message:** \`engine: gate XR elbow constraints against optical and mounted IMU evidence\`.
- **Gate:** valid IMU rotations survive conflicts, one broken arm does not break opposite arm, no second solver.

### Commit 05 — continuity, recording and replay identity

- [ ] Add \`arm_state.rs\` to bounded \`FusionState\`; entry 3 frames, exit 2 soft / immediate hard.
- [ ] Wire reset to athlete/room/stage/profile/reconnect and explicit source gaps.
- [ ] Update tracking/config calibration identity to include body/wrist profiles and algorithm v1 while enabled.
- [ ] Assert **sequential replay == seek replay** on mixed camera/IMU/XR recordings with bend history.
- [ ] Check runtime ticks/recording reconstruction, no wall-clock hidden state or unbounded history.
- **Commit message:** \`engine: make XR arm continuity deterministic across occlusion and replay\`.
- **Gate:** recorded outputs and source attribution stable under restart and seek.

### Commit 06 — compact UI and operational feedback

- [ ] Add status for XR arm constraints, blocked reasons, left/right source and fallback state.
- [ ] Keep existing VR mannequin, BodyRoot, headset camera and 50ms presentation buffer; no new avatar IK.
- [ ] Desktop can show quality source/provenance without introducing a separate renderer or networking client.
- [ ] Add Godot headless smoke tests covering inferred vs measured labels and recenter.
- **Commit message:** \`ui: display per-arm XR fusion quality and calibration blockers\`.
- **Gate:** clear diagnostics, no false "full-body measured" state, no VR navigation regression.

### Commit 07 — CI, docs and physical signoff

- [ ] Execute exact supported commands in §10.5, verify contracts regenerated cleanly and test evidence captured.
- [ ] Update docs/05-calibration-and-fusion.md, docs/13-implementation-status.md and CI.
- [ ] Produce hardware validation with measured metrics or explicit **BLOCKED**; never claim simulated checks are hardware signoff.
- [ ] Fetch latest main before final commit, review diff for unrelated changes, preserve concurrent commits.
- **Commit message:** \`docs,ci: validate Phase 04 XR camera IMU elbow fusion\`.
- **Gate:** green CI, source identity evidence, reproducible replay, honest real-world limitations.

## 13. Phase 04 completion-report checklist (implementing AI)

Report:
1. Actual commits implementing Phases 01/02/03, and Phase 04 before/after HEAD. If missing, mark blocked and do not fabricate implementation.
2. Seven direct-main commits with URLs and exact file list, including any code path changes from this pinned map.
3. Golden/test commands with PASS/FAIL/BLOCKED; no claim of tests not run.
4. World transform and wrist-offset calculation for synthetic identity and yaw90 cases, with source IDs and time quality.
5. Measured arm lengths, optical elbow quality, IK elbow/W_solve geometry and residual, plus source priority result.
6. Complete per-side arbitration examples: zero IMUs, both IMUs, only upper, only forearm, no camera elbow, unreachable target, short dropout.
7. Proof of inferred vs derived/observed null joint semantics, stable elbow bend and no NaN.
8. Calibration/athlete/stage/profile invalidation and reproducible \`tracking_config_id\`/\`calibration_set_id\`.
9. Recorded-live vs sequential-replay vs seek-replay equality proof with bend-plane continuity.
10. Physically measured elbow accuracy and frame time/latency, or clearly **BLOCKED** if no headset/cameras.
11. Measured p50/p95 overhead and regression evidence against feature-off.
12. Remaining TODOs with precise owner/path and acceptance criteria.

## 14. Non-goals and future dependencies

- **Phase 05:** Humanoid \`Skeleton3D\` / skinned avatar mesh rest-pose retargeting and full VR character presentation. Phase 04 outputs canonical world pose only.
- **Phase 06:** Benchmark COCO17 vs 33-keypoint heel/toe tracking, not assumed improvement; no model switch in this phase.
- **Phase 07:** Hardware multi-camera/headset/controller/SlimeVR validation at martial-arts speed, including latency, exposure skew, drift and held-out error.
- **Deferred beyond phase:** Real hand/palm anatomical orientation calibration, calibrated HMD-eye-to-head-joint transform, EKF/factor graph, full-body mechanical optimization, OSC/VMT/SteamVR drivers, Kinect/Unity interoperability, markerless single-camera absolute metric arm measurement and advanced contact physics.

**Definition of done:** When the prerequisites exist, the engine optionally derives safe calibrated world wrists from real XR grips; solves left/right arm links with actual athlete proportions; uses camera elbow bend evidence and bounded fallback; conservatively preserves reliable mounted IMU orientations; produces a single FK-consistent skeleton with accurate per-source/provenance/time metadata; invalidates and replays reliably; and leaves the existing pipeline byte-for-byte equivalent when XR arm fusion is off. **No Phase 05 avatar rig work is claimed.**
