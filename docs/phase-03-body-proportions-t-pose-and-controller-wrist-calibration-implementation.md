# Bansayo Phase 03 — Body proportions, T-pose and grip-to-wrist calibration

**Status:** Prescriptive design and implementation plan, NOT application code.  
**Date:** 2026-11-20.  
**Target:** https://github.com/danielsobrado/Bansayo — direct commits on **main**, no pull request/force-push.  
**Verified target HEAD before this document:** [192d7b468c028859c174f9fbd6a269a51b124450](https://github.com/danielsobrado/Bansayo/commit/192d7b468c028859c174f9fbd6a269a51b124450).  
**Upstream reference commit:** [maceq687/FullBodyPoseEstimation@be895709](https://github.com/maceq687/FullBodyPoseEstimation/commit/be895709c0bf86d35f64eb2cd5d2ffb4c6838593).  
**Prerequisites:** [Phase 01 XR observation ingress](phase-01-openxr-observation-pipeline-implementation.md) and [Phase 02 XR world alignment + clock mapping](phase-02-openxr-room-alignment-and-time-synchronization-implementation.md).  
**Next milestone:** Phase 04 confidence-aware XR + camera + IMU fusion and constrained elbow IK.  
**Implementation rule:** Source snapshots and line numbers below are factual for the above HEAD, while Phase 01/02 planned files are only **future prerequisites**. Refresh main and test before every implementation commit; preserve all newer commits.

> **Critical sequencing:** At this verified HEAD Phase 01 and Phase 02 are detailed Markdown specifications, **not implemented XR modules**. Do not claim the calibrated XR wrist offsets can work until those phases exist. The camera-only anthropometry subsystem can be developed/tested against deterministic camera data, but XR-to-wrist calibration must be blocked by missing Stage, clock or calibration evidence. This file is the full Phase 03 implementation contract.

## 0 — Intended capability and the mistakes to avoid

Provide reproducible, athlete-specific body measurements that Bansayo can use later to constrain upper limbs, without mislabeling assumptions as measurements or destabilizing the current skeleton.

Required Phase 03 outcomes:

1. Existing Bansayo athlete height and default built-in proportions remain the **safe fallback**.
2. A camera-measured **five-second stationary T-pose** produces side-specific shoulder→elbow and elbow→wrist **metric lengths** where the triangulated joints are reliable; unknown lengths stay explicitly **assumed**, not invented.
3. With **valid Phase 02 alignment** and sufficiently synchronized XR and camera observations, the engine estimates each controller's **grip-origin-to-anatomical/optically-defined wrist-center offset** in the **controller-local frame**. This is a separate calibration per athlete and controller.
4. Measurement provenance, quality, sample counts, residuals, camera IDs, model/schema revision, athlete identity, room/camera fingerprint and XR stage/physical-controller identity are explicit.
5. Lengths and offsets are persisted as **typed CalibrationProfile** records by Bansayo's existing single-writer snapshot. Active athlete switches must never apply another athlete's body geometry.
6. Only measured limb lengths with passing quality gates modify the **active live SkeletonDefinition**. All other segments continue using the existing default fractions of height.
7. Skeleton changes are atomic and revisioned: any downstream Godot/desktop cached rig is rebuilt before a pose with a different skeleton revision is drawn.
8. Recording/context snapshot and replay use **the precise skeleton and calibration effective at recording time**, regardless of a later athlete switch or recalibration.
9. **No Phase 04 fusion yet**. Calibration can publish a grip-to-wrist local offset and measured arm lengths, but cannot directly drive elbows/IK/pose from XR inputs in Phase 03.

### 0.1 Critical technical corrections

- **Grip-to-grip span is not shoulder width.** Controller position includes forearms, wrists and the grip coordinate origin.
- **HMD elevation is not full body height.** It is the tracked eye/device pose relative to the selected XR reference space; it varies with posture and equipment.
- **A T-pose alone cannot determine accurate controller-to-wrist offsets without an independent wrist measurement.** Require calibrated multi-camera wrist landmark positions and correct stage/world/time mapping.
- **A camera COCO-17 wrist keypoint is an estimate of a wrist landmark, not a ground-truth anatomical joint center.** Report it as camera-derived; never claim medical motion-capture accuracy.
- **The existing mounting reference pose is a different calibration.** It aligns an IMU to a body segment; it does not measure bone length. Do not reuse its API to create anthropometry profiles.
- **Height entered in an athlete profile is self-reported or externally measured, not something Bansayo estimated from HMD.** Store origin accordingly.
- **One global calibration is wrong for a multi-athlete training system.** A body profile must have an athlete_id; a controller→wrist profile must additionally identify the device/attachment.
- **Phase 04 IK must not consume low-confidence lengths as precise constraints.** Unknown values remain safe assumed defaults with provenance and validity state.

## 1 — Verified upstream-to-target code map

### 1.1 Reference implementation: what to leverage conceptually

| Upstream source (immutable ref) | What the code actually does | Bansayo Phase 03 implementation decision |
|---|---|---|
| [VNectModel.cs L603–L651](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L603-L651) | Guided Kinect/camera/VR T-pose calibration and avatar scaling routines | Reuse guided user flow concept. Compute metric body lengths in the Rust engine, not Godot/Unity. |
| [VNectModel.cs L651–L670](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L651-L670) | Waits five seconds and uses left/right controller distance and HMD height as a VR size proxy | **Reject as anatomical measurement.** Span and eye height are not shoulder width or stature. |
| [VNectModel.cs L169–L183](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L169-L183) | Samples head, left and right controller 6-DoF poses | Phase 01 already plans those tracked-device inputs; Phase 03 uses right/left grip poses only after Phase 02 time/space alignment. |
| [VNectModel.cs L188–L214](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L188-L214) | Uses hard-coded Euler wrist rotations and phantom elbow projection | **Do not copy.** Independently estimate local grip→wrist *translation* and leave elbow IK for Phase 04. |
| [VNectModel.cs L238–L246](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L238-L246) | Measures avatar hand span and nose height | Such avatar dimensions are cosmetic rig properties, not a calibrated person's joint lengths. |
| [PoseVisualizer3D.cs](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/PoseVisualizer3D.cs) | Visualizes body landmarks and scaling | A useful UX reference; camera solver is already native Rust in Bansayo. No code/assets copied. |
| [Upstream GPL-3.0 license](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/LICENSE) | GPL-3.0 code | Treat reference as research only; independent implementation in Rust and Godot/React. |

### 1.2 Exact current Bansayo code references

| Target source at verified HEAD | Verified current state | Phase 03 required change |
|---|---|---|
| [skeleton/proportions.rs L1–L83](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/skeleton/proportions.rs#L1-L83) | One immutable built-in height-normalized anatomy table. Upper arm tail fraction 0.186H, forearm 0.146H; mirrored left/right. | Preserve as fallback; introduce runtime measured length overrides without mutating constants. |
| [skeleton/definition.rs L18–L47, L89–L159](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/skeleton/definition.rs#L18-L159) | Skeleton::builtin(model,height) scales all rows; SkeletonDefinition.proportions is always Assumed, revision currently constant 1. | Add a typed factory accepting optional measured upper-arm/forearm lengths. Keep topology, offsets and tails consistent and revise skeleton when geometry changes. |
| [contracts/src/skeleton.rs L8–L66](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-contracts/src/skeleton.rs#L8-L66) | SegmentDef has parent, offset_m and tail_m; SkeletonDefinition has height_m, proportions:CalibrationState and rev. | Add optional per-segment source/provenance metadata, backward compatible, or derive status directly from profile; prevent a partial calibration being falsely labeled all-measured. |
| [pipeline/skeleton_fit.rs L15–L34](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/pipeline/skeleton_fit.rs#L15-L34) | COCO-17 schema indexes shoulders 5/6, elbows 7/8, wrists 9/10. | Reuse exact landmark schema/index mapping; do not invent extra anatomical joints. |
| [pipeline/skeleton_fit.rs L62–L166](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/pipeline/skeleton_fit.rs#L62-L166) | Fit maps COCO17 points into internal keypoints, with minimum 0.3 confidence for normal tracking. | Calibration has stricter config threshold (0.70) and per-joint view/reprojection gates. Keep normal fit's 0.30 unchanged. |
| [contracts/src/landmarks.rs L81–L123](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-contracts/src/landmarks.rs#L81-L123) | TriangulatedLandmark3D already holds world metres, reprojection_error_px, confidence, used_views; PoseLandmarks3D::Triangulated includes capture time. | Consume these **actual** fields for calibration QA, never model-local / normalized 3D. |
| [pipeline/triangulate.rs L93–L179, L304–L318](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/pipeline/triangulate.rs#L93-L179) | Calibrated multi-camera DLT triangulation, view rejection and reprojection metrics. | Use accepted triangulation results; don't add a parallel triangulator or force different camera calibration. |
| [pipeline/solve.rs L1–L175](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/pipeline/solve.rs#L1-L175) | Existing camera/IMU arbitration; no XR wrist or arm IK. | Keep solver logic unchanged; measured Skeleton geometry only modifies segment definitions, not tracking-source selection. |
| [calibration/mounting.rs L1–L42](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/calibration/mounting.rs#L1-L42) | One-second mean IMU mounting reference capture. | Do not alter its one-second window, target selection or mounting rules. |
| [apps/desktop/src/calibration/ReferencePoseFlow.tsx L7–L69](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/apps/desktop/src/calibration/ReferencePoseFlow.tsx#L7-L69) | Current desktop T-pose dialog calls captureReferencePose, which calibrates IMU **mounting**, not body geometry. | Add a distinct **Body proportions** action/wizard and label existing dialog **Tracker mounting**. Never repurpose the existing endpoint. |
| [apps/desktop/src/engine/actions.ts L35–L46](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/apps/desktop/src/engine/actions.ts#L35-L46) | HTTP methods for skeleton and mounting capture already exist. | Add typed body-proportion capture/status and controller-offset calibration requests. |
| [contracts/src/calibration.rs L9–L44](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-contracts/src/calibration.rs#L9-L44) | **CalibrationKind::BodyProportions already exists**; no controller wrist offset kind. | Reuse BodyProportions; add distinct XrGripWristOffset kind rather than overload IMU Mounting or XrAlignment. |
| [contracts/src/calibration.rs L75–L110](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-contracts/src/calibration.rs#L75-L110) | CalibrationSubject only device_id and segment_id. CalibrationProfile uses typed kind + JSON data/quality. | Add backwards-compatible optional athlete_id to CalibrationSubject. Per-athlete profiles use athlete_id, not arbitrary user-supplied IDs. |
| [calibration/store.rs L162–L189](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/calibration/store.rs#L162-L189) | valid_profile currently requires device_id and supports only mounting/intrinsics/extrinsics. | Refactor validation **per kind**, allowing body profiles with athlete_id and no device_id; strictly validate separate wrist offset kind. |
| [calibration/store.rs L26–L155](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/calibration/store.rs#L26-L155) | Mounting lookup, camera room filtering, invalidation, summaries and insert. | Add typed per-athlete lookup and dependent wrist-offset invalidation; do not break camera/mounting. |
| [pipeline/command.rs L16–L88](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/pipeline/command.rs#L16-L88) | Single-writer typed engine controls, Capture for IMU mounting, Import for camera profiles. | Add separate BodyCapture and WristCapture commands; do not reuse mounting capture Command::Capture. |
| [pipeline/runtime.rs L234–L301, L309–L375](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/pipeline/runtime.rs#L234-L375) | Single event loop, 1-second IMU capture, atomic profile commit and SystemState updates. | Implement bounded 5-second body capture and wrist calibration without blocking event loop or colliding with existing calibration. |
| [pipeline/runtime.rs L90–L110, L171–L214](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/pipeline/runtime.rs#L90-L214) | Skeleton stream has watch updates and revision bumping; Frame and MotionSample use skeleton revision. | Atomically change active skeleton, publish new definition before pose; preserve existing revision/recording mechanisms. |
| [pipeline/runtime.rs L408–L460](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/pipeline/runtime.rs#L408-L460) | Each live tick solves with current skeleton and updates registry fusion. | Capture **accepted triangulated samples at solve tick** with bounded retention; no re-triangulation in wizard. |
| [pipeline/runner.rs L19–L45, L123–L210](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/pipeline/runner.rs#L19-L45) | Runner owns live skeleton, store, registry, training_status and exposes SystemState. | Rebuild Skeleton from selected athlete-specific profile and keep fallback when no athlete selected. |
| [training/mod.rs L20–L42](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/training/mod.rs#L20-L42) | TrainingStatus watch carries active AthleteRef but not athlete height. | Extend **internal** status with athlete height/revision to refresh active skeleton on profile edits. |
| [training/athletes.rs L45–L69, L125–L210](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/training/athletes.rs#L45-L210) | Athlete profiles are persisted; height validated between 0.5 and 2.5m. | Bind body measurements to AthleteId and height captured at calibration; new height invalidates measured body geometry. |
| [contracts/src/athlete.rs L69–L87](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-contracts/src/athlete.rs#L69-L87) | AthleteProfile.height_m exists and explicitly drives skeleton scaling. | Use active athlete height as preferred baseline over config.skeleton.height_m. No HMD-height substitution. |
| [config/schema.rs L255–L269](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/config/schema.rs#L255-L269) | SkeletonConfig default model bansayo.body.v1 and height 1.75m. | Keep unchanged for no selected athlete / fallback; add calibration thresholds in YAML, not code. |
| [registry/persistence.rs L78–L149](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/registry/persistence.rs#L78-L149) | RuntimeSnapshot already holds profiles, stale IDs and room with atomic save. | Store athlete-scoped profiles in same snapshot, no duplicate JSON store. |
| [recording/mod.rs L13–L82, L121–L177](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/recording/mod.rs#L13-L177) | Recording Context stores skeleton/active profiles but restore currently recreates height-only Skeleton and requires a device subject for any valid profile. | Extend restore for body profiles without device subject and reconstruct **exact recorded measured geometry**, not current athlete geometry. |
| [recording/replay.rs L1–L83](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/recording/replay.rs#L1-L83) | Replay rebuilds from the recorded Context. | Assert replay is deterministic after athlete edit/deletion or recalibration. |
| [api/mod.rs L45–L70](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/api/mod.rs#L45-L70) | Existing /calibration/reference-pose (mounting) and /calibration/profiles (camera) routes. | Add dedicated body/wrist calibration endpoints; no behavior changes to old routes. |

**Normative repo docs:** [03-domain-model.md](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/docs/03-domain-model.md), [05-calibration-and-fusion.md](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/docs/05-calibration-and-fusion.md), [08-vr-client.md](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/docs/08-vr-client.md). Current docs explicitly describe body proportions as **height-only assumed**; update status on implementation, not in advance.

## 2 — Architectural decisions, scope and data flow

~~~text
Selected athlete (AthleteId, entered height in AthleteProfile)
                 |
                 v
       5-second T-pose capture
 2–4 calibrated cameras -> existing triangulate (COCO17 world metres)
                 |
         strict QA / stillness
                 |
        shoulder/elbow/wrist side-specific lengths
                 |
      typed BodyProportions profile (per athlete)
                 |
      existing Store / RuntimeSnapshot / recorder
                 |
      Skeleton::with_body_proportions(...)
                 |
      watch SkeletonDefinition, SystemState, MotionSample
                 |
      existing engine solver uses new segment geometry
           X no new tracking fusion

For controller offset (optional dependent step):
 Phase 01 XR grip observations
       + Phase 02 room/world alignment + clock mapping
       + matched camera triangulated wrist points
                 |
     r_grip_from_wrist = R^-1 * (wrist_world - grip_world)
                 |
    robust per-arm, per-controller local offset estimate
                 |
    typed XrGripWristOffset profiles (athlete + device + rig)
                 |
       preserved for Phase 04
           X do not drive live wrist/elbow this phase
~~~

### 2.1 Separate independent calibration results

- **BodyProportions:** per athlete, room-independent *after* acceptance as a metric measurement; it records camera provenance for auditing but is not automatically made stale by moving the camera after calibration. It remains valid until athlete height changes, skeleton model changes, or measurement invalidation. Camera provenance is historical, not a permanently required active-room dependency.
- **XrGripWristOffset:** per athlete, per controller side and model/physical grip setup; needs a valid current camera profile, current XR alignment for transforming observations, clock quality and a matched camera wrist landmark. It becomes stale if the controller changes, the attachment changes, athlete changes, relevant body model revision changes or calibration evidence is no longer valid.
- **IMU Mounting:** existing one-second calibration, device/segment keyed, unchanged. No shared state machine between IMU capture and new anthropometry capture except resource exclusion.
- **XR Alignment:** Phase 02 measured room transform, stage-specific, unchanged. A stage recenter invalidates the active mapping; do not rotate stored local controller wrist offsets as though they were in world space.

### 2.2 Calibration methods and measured/assumed semantics

| Quantity | Source | Measurement status | Applied in Phase 03 |
|---|---|---|---|
| Athlete height (m) | AthleteProfile.height_m | User-entered / declared, not camera measured | Yes, as default scale |
| Left/right upper-arm length | Distance shoulder→elbow from COCO17 world landmarks | Camera-derived when quality passes | Yes |
| Left/right forearm length | Distance elbow→wrist from COCO17 world landmarks | Camera-derived when quality passes | Yes |
| Shoulder span | Distance between two COCO17 shoulder landmarks | Camera-derived QA only; **not exact clavicle anatomy** | **No**: saved as diagnostic, not used to alter clavicle |
| Hand lengths | No reliable COCO17 fingertips | Assumed from built-in model | Keep default |
| Torso, pelvis, leg lengths | Not independently calibrated in this phase | Assumed from height normalized table | Keep default |
| Grip→wrist local translation | Camera wrist minus aligned XR grip, rotated into grip local axes | Camera+XR derived, conditionally valid | Store only, for Phase 04 |
| Grip→hand orientation | No trusted anatomical palm/hand frame from COCO17 | Unknown | **Do not invent** Euler/rotation offset |
| HMD→head bone | Eye pose is not head joint center | Unknown | No anatomical head offset |
| Elbow bend plane/IK | Requires joints+temporal tracking | Not a calibration output | Phase 04 |

A valid *partial* BodyProportions profile means measured arm lengths passed, not that **every** segment is measured. The published SkeletonDefinition must include per-segment measured/assumed status; a single aggregate CalibrationState alone cannot express the distinction.

## 3 — Fixed measurements and five-second capture procedure

### 3.1 Prerequisites (camera-only component)

1. Selected **active athlete** exists; AthleteProfile.height_m finite in 0.5–2.5m. If none, do not create an anonymous measured profile.
2. At least two calibrated cameras in current active room, identities confirmed and fresh COCO17 triangulated world landmarks. Do not use single-camera model-local 3D, YOLO bounding boxes or unsynchronized RTSP frame arrival as if they provide metric joints.
3. Each required joint (left/right shoulder, elbow, wrist) has confidence **>=0.70**, reprojection RMS **<=5.0px**, **>=2 distinct used_views**, and finite world position in metres.
4. Current camera rig/world calibration is valid, profile revisions stable. Reject any sensor identity changes or room switch during the capture.
5. No recording, replay or active workout; no other calibration already in progress. UI gives an actionable conflict, not a hidden failure.
6. Once countdown starts, user stands upright facing world +Z with arms held approximately horizontal (T-pose). Do not use grip span as a criterion when cameras can measure arms directly.
7. If the camera detector schema differs from COCO17, only permit a known, tested canonical segment-origin mapping that has exactly equivalent shoulder/elbow/wrist semantics; reject unknown schemas rather than guessing indices.

### 3.2 Capture rate/window and stationary quality

The engine buffers validated **unique triangulated sets** for a wall-clock five-second capture window. Do not build a second triangulation subsystem or clone image buffers.

| Config | Default | Allowed |
|---|---:|---|
| body.capture_duration_ms | 5000 | 3000–10000 |
| body.min_unique_frames | 45 | 20–200 |
| body.max_frames | 600 | 100–1200 |
| body.min_point_confidence | 0.70 | 0.5–1.0 |
| body.max_reprojection_error_px | 5.0 | 1.0–15.0 |
| body.min_views_per_joint | 2 | 2–4 |
| body.max_joint_velocity_m_s | 0.12 | 0.03–0.40 |
| body.max_length_mad_m | 0.012 | 0.003–0.040 |
| body.max_length_relative_mad | 0.04 | 0.01–0.15 |
| body.max_arm_elevation_from_horizontal_deg | 18.0 | 5–35 |
| body.max_elbow_deviation_from_straight_deg | 25.0 | 5–45 |
| body.min_measured_length_m | 0.10 | 0.05–0.25 |
| body.max_measured_length_m | 0.60 | 0.35–0.85 |
| body.min_length_to_height_ratio | 0.07 | 0.04–0.10 |
| body.max_length_to_height_ratio | 0.30 | 0.24–0.40 |

**Validation procedure:**

1. On each newly accepted triangulated set (unique t_us and camera-set identity), extract six exact COCO17 arm points using skeleton_fit.rs mapped indexes: shoulders **5/6**, elbows **7/8**, wrists **9/10**.
2. Gate per-landmark confidence, reprojection RMS, distinct camera count, finite position. Reject the *corresponding arm side* for that frame if any of its three joints fail; other arm side may remain valid.
3. Require left/right shoulders approximately same Y within **0.10m** and wrists roughly shoulder height (absolute delta <= **0.14m**) for claimed stationary T-pose. When body orientation differs from +Z, show instruction to face front. Do not silently rotate person or infer camera facing from a single frame.
4. Detect actual elbow straightness using normalized vectors **(shoulder - elbow)** and **(wrist - elbow)**. At straight arm these vectors point in nearly opposite directions: elbow deviation from straight = abs(pi - angle(v1,v2)) <=25°.
5. For each arm, use **distance(S,E)** and **distance(E,W)** in metres. Temporal median and median absolute deviation (MAD) provide robust side-specific lengths. No projection onto controller-to-controller line. A bent elbow does not change physical bone length, but the straight T-pose gate simplifies keypoint reliability.
6. Compute joint velocity from successive accepted **capture timestamps**, not display frame rate. Reject periods above max_joint_velocity_m_s, time gaps above 150ms and repeated timestamps.
7. Accept a side when it has >=45 individually valid frames spanning >=4.0s inside the five-second capture and both bone MAD absolute/relative gates pass. If one arm passes and one fails, allow a **partial profile only if explicitly configured** (default allowed) and mark the other side assumed with reason; no silent copy/mirror from the good side.
8. Reject if neither arm passes. Never save a "measured" length from a single noisy frame or from an uncalibrated camera.
9. Keep per-side actual sample count, temporal span, confidence minimum, reprojection p95, MAD, included device IDs, active room and camera revision fingerprint. Persist the exact measured estimates and labels as **camera-derived**, not manually measured.
10. A camera-only capture must work in desktop mode **without headset**. The separate controller-offset phase is unavailable when Stage or XR alignment is absent.

**Important QA correction:** A strictly horizontal arm has hand height near shoulder, but joints can be off by centimetres due to COCO detector noise. Use the above thresholds to reject obviously wrong pose, not to assert anatomically perfect T-pose.

### 3.3 Robust length statistic

For each side and each length series L_i:

~~~text
median_length = median(L_i)
MAD = median(|L_i - median_length|)
relative_MAD = MAD / median_length

accept if:
  min_length <= median_length <= max_length
  min_ratio <= median_length / athlete_height <= max_ratio
  MAD <= max_length_mad_m
  relative_MAD <= max_length_relative_mad
  accepted_count >= min_unique_frames
  capture_time_span >= 4 seconds
~~~

Deduplicate camera timestamp sets; discard nonfinite data before statistics. Do not use a mean across good left and right lengths; asymmetry may be real. Do not use the first frame to set final lengths. Persist independent side values.

For the final confidence, retain empirical QA metrics and optional conservative score but **do not** reuse detector confidence as a medical-grade error bound or as a sensor-fusion weight without later validation.

## 4 — XR grip-to-wrist calibration: separate dependent step

### 4.1 Precise transform and equations

A wrist landmark is measured in camera world, while an OpenXR controller gives a **grip device pose in XR stage**. Phase 02 provides a valid **T_xr_stage_from_world** and time correspondence.

Define:

~~~text
T_world_from_xr_stage = inverse(T_xr_stage_from_world)
p_world_grip = T_world_from_xr_stage * p_xr_stage_grip
R_world_from_grip = R_world_from_xr_stage * R_xr_stage_from_grip

w_world_camera = triangulated COCO17 wrist (camera world)

r_grip_from_wrist_i =
    inverse(R_world_from_grip_i) * (w_world_camera_i - p_world_grip_i)
~~~

The solved **r_grip_from_wrist** is a **translation in the controller grip's local coordinate frame**. It is not a hand rotation offset. Do not confuse direction naming with the Phase 02 physical pointer pivot offset **r_grip_from_tip**; they are different physical points and use different calibration purposes.

If controller wrist position is used in Phase 04:

~~~text
wrist_world = p_world_grip + R_world_from_grip * r_grip_from_wrist
~~~

### 4.2 Preconditions

All are required:

- Active athlete and at least one measured arm side from the current BodyProportions profile, or valid camera wrists used with a declared independent measurement: **do not silently estimate controller offset solely from body ratios**.
- Current Phase 02 XR alignment is **valid** and StageInfo identity/generation match.
- Phase 02 clock status **settled** and estimated uncertainty <= **5,000us**; camera/XR paired observations must have **absolute time difference <=20ms** and no extrapolation. Report camera timestamp limitations.
- The correct XR grip device is tracked. Device ID must be the reserved **XR-LEFT-GRIP** or **XR-RIGHT-GRIP** from Phase 01, not UI aim pose.
- The triangulated camera wrist landmark for corresponding side has confidence >=0.70, reprojection <=5px and >=2 distinct calibrated views. If camera sees only glove/controller and misidentifies the wrist, calibration must fail quality or user should recheck/adjust camera placement.
- The controller model/role identity is recorded. Where a stable hardware serial is unavailable, mark persistence dependent on **current XR stage-session identity** by default; do not claim offsets portable to another physical controller.
- User holds controllers naturally and securely; changing the grip or strap invalidates offsets. No software-only test can verify a physically modified grip.

### 4.3 Data collection

This is a **separate step** after the five-second T-pose body capture. Ask the user to hold both controllers, start from T-pose and slowly bend/extend wrists/forearms while retaining camera visibility. Avoid karate-speed kicks/punches during calibration.

| Config key | Default | Allowed |
|---|---:|---|
| body.wrist_capture_duration_ms | 4000 | 2000–10000 |
| body.wrist_min_paired_samples | 30 | 15–120 |
| body.wrist_max_pairs | 300 | 60–600 |
| body.wrist_max_skew_ms | 20 | 5–50 |
| body.wrist_max_clock_uncertainty_us | 5000 | 1000–15000 |
| body.wrist_min_rotation_span_deg | 20 | 8–60 |
| body.wrist_max_offset_norm_m | 0.25 | 0.08–0.40 |
| body.wrist_max_residual_rms_m | 0.030 | 0.010–0.080 |
| body.wrist_max_residual_p95_m | 0.050 | 0.020–0.120 |
| body.wrist_min_inlier_fraction | 0.80 | 0.6–0.95 |

Pair each camera wrist to the closest valid XR grip observation of the **same side**. Prefer interpolation between two bracketing tracked XR poses separated by <=40ms, with linear positions and quaternion SLERP, then require interpolation target <=20ms from each acceptable bracket. Never extrapolate into a camera timestamp; stale/missing XR samples cause that pair to be skipped.

Compute r_i from equation above. Reject finite outliers above max(0.05m, 3×median residual) after initial robust median and refit using component-wise median of inliers. Require >=30 inlier pairs per calibrated side, >=80% inlier fraction, >=20° controller orientation span and position residual RMS <=0.030m, p95 <=0.050m and offset norm <=0.25m.

**Observability nuance:** Even with a constant controller pose, independent camera wrist position theoretically gives a translation; however, varied controller orientation/poses tests whether the local offset remains constant and reveals incorrect camera/XR alignment. Require the rotation-span gate rather than trusting stationary noise.

If left passes and right fails, persist **only left** controller offset and show right as **missing/failed**. Do not mirror the left offset or silently use the Phase 02 physical pointer tip.

## 5 — Typed calibration contracts and athlete isolation

### 5.1 Extend CalibrationSubject additively

Current CalibrationSubject contains device_id and segment_id, and old persisted profiles omit athlete information. Add:

~~~rust
#[serde(default, skip_serializing_if = "Option::is_none")]
#[ts(optional = nullable)]
pub athlete_id: Option<AthleteId>,
~~~

Old mounting/camera/XR alignment profiles decode with athlete_id=None. **No global body-proportion profile is allowed.** BodyProportions uses subject={athlete_id:active athlete,device_id:null,segment_id:null}. XrGripWristOffset uses athlete_id and the corresponding XR grip DeviceId, segment_id=null. Preserve existing CalibrationProfile schema_version=1 and existing summary shape as much as possible.

### 5.2 Add two concrete typed kinds

CalibrationKind::BodyProportions **already exists**; add **CalibrationKind::XrGripWristOffset** with serde tag xr_grip_wrist_offset. Do not overwrite Mounting, XrAlignment, CameraExtrinsics or TimeSync.

Define in crates/bansayo-contracts/src/calibration.rs:

~~~rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct BodyProportionsData {
    pub athlete_id: AthleteId,
    pub skeleton_model_id: String,
    pub athlete_height_m: f64,
    pub measured_segments_m: BTreeMap<SegmentId, f32>,
    pub measured_sources: BTreeMap<SegmentId, String>,
    pub camera_fingerprint_at_capture: String,
    pub landmark_schema: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct SegmentLengthQuality {
    pub accepted_frames: u32,
    pub capture_span_ms: u32,
    pub length_mad_m: f32,
    pub reprojection_p95_px: f32,
    pub confidence_min: f32,
    pub cameras: Vec<DeviceId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct BodyProportionsQuality {
    pub capture_window_ms: u32,
    pub per_segment: BTreeMap<SegmentId, SegmentLengthQuality>,
    pub shoulder_span_m: Option<f32>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct XrGripWristOffsetData {
    pub athlete_id: AthleteId,
    pub device_id: DeviceId,
    pub side: String,
    pub r_grip_from_wrist_m: Vec3,
    pub controller_model: String,
    pub grip_revision: String,
    pub stage_session_id: String,
    pub stage_generation: u64,
    pub camera_fingerprint_at_capture: String,
    pub xr_alignment_profile_id: ProfileId,
    pub body_proportions_profile_id: ProfileId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct XrGripWristOffsetQuality {
    pub paired_samples: u32,
    pub inlier_fraction: f32,
    pub rotation_span_deg: f32,
    pub residual_rms_m: f32,
    pub residual_p95_m: f32,
    pub max_clock_uncertainty_us: u32,
}
~~~

The planned field **grip_revision** is a conservative local user-confirmed grip/attachment revision string generated by the calibration wizard or profile; empty/unknown means do not reuse on another session. If hardware controller serial cannot be obtained from OpenXR, portability across sessions is **unproven**, not valid by default. Use controlled enum or validated ASCII identifier when implementing rather than arbitrary paths.

**Exact BodyProportions measured segment keys allowed initially:**
left_upper_arm, left_forearm, right_upper_arm, right_forearm. Reject any other key in the Phase 03 API or persisted measured map; do not accept arbitrary bone offsets or topological changes.

**Profile IDs:**

- Body: **body-proportions.<athlete_id>**.
- Left wrist: **xr-grip-wrist.<athlete_id>.left**.
- Right wrist: **xr-grip-wrist.<athlete_id>.right**.

All are valid under existing ProfileId syntax. Do not identify athletes by display name. Origin=live (for camera/XR captured), method="camera_coco17_tpose_v1" or "camera_xr_grip_wrist_v1"; use created_utc via existing clock helper; data/quality typed serialized to current CalibrationProfile.

### 5.3 Calibration profile validity checks

Refactor calibration/store.rs::valid_profile so validation depends on kind rather than the current global condition subject.device_id.is_some():

- Mounting: device_id+segment_id required, optional athlete_id only if explicitly designed to be athlete-specific in a future phase; preserve current behavior.
- CameraIntrinsics/CameraExtrinsics: device_id required, segment_id and athlete_id must be None.
- XrAlignment (Phase 02): validate its existing subject/room/stage; unaffected by body changes.
- BodyProportions: athlete_id required, device_id and segment_id must be None, supported skeleton_model_id, 0.5–2.5m height, 1–4 recognized measured arm segments, finite positive lengths within body-config sane absolute limits, matching quality for every measured key, no duplicated or unknown source, profile canonical ID.
- XrGripWristOffset: athlete_id and matching XR grip device required, correct side for reserved ID, valid unit/metric bounds, valid referenced XR alignment/body profile IDs, finite quality, stable grip identifier, canonical profile ID.
- Other unsupported kinds remain invalid until their actual implementations exist; do not introduce a permissive wildcard.
- Old CalibrationProfile JSON and examples continue deserializing unchanged.

**Important recording restore fix:** [recording/mod.rs L121–L177](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/recording/mod.rs#L121-L177) currently assumes every valid calibration has a device subject. Change to match expected subject by kind. For body profiles, **device_id==None is correct** and must not cause replay to fail. Validate the recorded athlete identity against the profile, never use current active athlete.

### 5.4 Per-athlete selection and height invalidation

The engine already watches [training::TrainingStatus](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/training/mod.rs#L20-L42) and receives changes in pipeline/runtime.rs::Event::TrainingStatus, but only receives AthleteRef ID and display_name.

Add to this **internal** status:

~~~rust
pub athlete_height_m: Option<f64>,
pub athlete_revision: u64,
~~~

On active athlete change or AthleteProfile height edit, increment revision (internal, not stored in wire). The **pipeline single-writer** derives the active skeleton using active athlete ID+height+valid body profile; no duplicate athlete database is maintained by the calibration module. On no athlete selected use existing config.skeleton.height_m and **Assumed** proportions, no measured profile.

If athlete_height_m changes from the value stored as profile.athlete_height_m, mark that profile **stale** (and dependent wrist profiles stale) **until recalibrated**. Do not rescale a camera-measured length automatically and continue calling it measured. The new baseline skeleton can still use the newly declared height with assumed model ratios.

If athlete changes while recording or a training session is running, do not mutate the skeleton in the middle of a recorded/active session. The training runner should refuse active-athlete changes in this state, or the engine must defer the switch atomically until session end; the implementation must have exactly one auditable policy: **refuse with 409 Conflict until session/recording stops**. Implement and test this guard across athlete API and engine state, rather than hoping UI hides the action.

### 5.5 Controller offset invalidation matrix

| Event | Body profile | Grip→wrist offset |
|---|---|---|
| Camera moved/recalibrated after a valid body measurement | Remains valid, since body segment length is physical; keep capture provenance | Stale when current camera/XR alignment evidence no longer matches |
| Athlete changed | Select different profile | Select other athlete's offset or missing; no cross-athlete reuse |
| Athlete height edited | Stale | Stale |
| Built-in skeleton model/topology changed | Stale | Stale |
| Active room changed | Body metric lengths still valid | Stale until calibration evidence for current room and stage verified |
| XR stage recentered | Body lengths unaffected | Local controller offset can still be physically the same, but **must not be applied** while world alignment invalid; status stale/blocked |
| XR runtime restarted with same physical controller | Body lengths unaffected | Stale by default if no stable hardware+grip revision evidence |
| Controller hardware/attachment/grip changed | Body unaffected | Stale |
| XR websocket reconnect same stage ID + unchanged controller | Body unaffected | Local offset may remain valid; clock sync must resettle |
| IMU tracker mounting recaptured | Body unaffected | Offset unaffected unless controller physical grip changed |
| Body profile recalibrated, same athlete | New profile revision | Stale to require evidence consistency |
| Profile corrupted/unknown kind | Invalid and fallback to height-only | Invalid and never used |
| Recording replay | Use recorded body snapshot | Use recorded calibration only; do not attach current XR session |

**Never automatically apply a stale wrist offset** to create a measured position in Phase 04. This boundary is part of Phase 03's acceptance contract.

## 6 — How to apply measured lengths without breaking skeleton topology

### 6.1 Current topology and required invariant

The built-in model from [skeleton/proportions.rs](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/skeleton/proportions.rs#L19-L62) has a parent-child chain:

~~~text
left_clavicle
   -> left_upper_arm  (offset = clavicle tail)
      -> left_forearm (offset = upper arm tail)
         -> left_hand (offset = forearm tail)
~~~

Right side mirrors the X axis. The arm rest direction is along +X for left and -X for right, and the topological segment IDs must remain unchanged.

**Adding only a new upper-arm tail is insufficient.** When a bone length changes, the *next child's origin offset* must change to exactly that new tail to avoid dislocated joints.

Implement **Skeleton::with_proportions(model, height_m, Option<&BodyProportionsData>)** or a similarly named typed factory with exactly these rules:

1. Build original model with Skeleton::builtin using the chosen athlete height; keep all other ratios unchanged.
2. For each measured upper arm, replace **the upper_arm tail** with a vector of measured magnitude along its original rest direction, and set matching **forearm parent offset** to exactly that tail. Do not change clavicle or shoulder coordinate.
3. For each measured forearm, replace **forearm tail** and **hand parent offset** correspondingly. Do not change default hand length.
4. Apply left and right independently. Never mirror a measured left arm to right without evidence.
5. Recompute published SegmentDef.offset_m/tail_m for all affected segments from same internal Skeleton.segments; no mismatch between internal FK and published contract.
6. Preserve floor-contact points, height, joint definitions, parents-before-children order and static skeleton model ID.
7. Set overall proportions status to **Valid only if at least one measured arm segment exists and profile matches athlete**, and include optional per-segment provenance (measured/assumed) for all segments. Other arms must still visibly show Assumed.
8. Revision **changes whenever geometry changes**, not merely when the model ID string changes. The existing watch mechanism increments skeleton rev, but verify downstream frame and motion revision ordering and fixture tests.
9. Do not write measured numbers into constants/proportions.rs; those remain the supported deterministic fallback.
10. Do not scale skin geometry independently in Godot. Avatar_renderer.gd already rebuilds from the SkeletonDefinition's tail_m and offset_m.

### 6.2 Per-segment source on the wire

The current SkeletonDefinition.proportions:CalibrationState cannot represent partly measured arms and assumed torso/legs. Add a **backwards-compatible optional** field:

~~~rust
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
#[ts(optional)]
pub segment_proportion_sources: BTreeMap<SegmentId, ProportionSource>,
~~~

Where ProportionSource is a tagged enum with **measured_camera** or **assumed_model**, and optional reference ProfileId if measured. The default empty map on *legacy* recordings means old clients continue to interpret proportions as previously. For all newly built skeleton definitions, populate all segment entries; no hidden "measured everything" implication. This field is display/calibration metadata, not a new solver trust or precision weight.

### 6.3 Recording and replay determinism

The existing [recording::Context::restore](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/crates/bansayo-engine/src/recording/mod.rs#L82-L177) rebuilds a height-only skeleton, ignoring measured offsets. This is a **hard replay bug** if Phase 03 only modifies live Runner.skeleton.

Prescribed remedy:

- Treat **Context.skeleton** (the recorded exact SkeletonDefinition with measured segment offsets) as authoritative for replay.
- Add **Skeleton::from_definition_validated(&SkeletonDefinition)** that validates model ID, topology, segment order/unique IDs, finite vectors, parent relationships, plausible limb lengths/offset magnitudes, and joint references; reconstructs internal Segment/Joint arrays from recorded offsets without reading current AthleteProfile or current calibration store.
- For historical unmodified contexts using standard definition, from_definition_validated returns geometry identical to the original Skeleton::builtin.
- Replay still uses recording.initial.config for solver settings and its recorded store/profile snapshot; do not call current athlete status.
- When a body profile is changed during a valid live session boundary, save a **Context** event before first pose generated from the new skeleton and update skeleton watch before next frame.
- If recording currently active, **deny** body calibration/athlete switch rather than changing a training body's geometry mid-recording. Thus historical recordings generally have one geometry; Context event design still supports future needs.
- Validate that recorded engine's measured offsets match recorded profile if available. If profile is absent but serialized skeleton is valid, preserve geometry for old/partial migration rather than silently reconstructing another geometry.
- Check that current pure FK/skeleton_fit do not assume default ratio table once Skeleton.segments was rebuilt; they should read internal vectors. Regression tests must prove unchanged joint indices/angles on identical motion.

**No change** to current framing of MCAP SourceEvents, sensor timestamps, or protocol-v1 ServerMessage::Frame fields.

## 7 — API surface and state machine

### 7.1 HTTP routes

Add to crates/bansayo-engine/src/api/mod.rs:

| Method/path | Body | Response |
|---|---|---|
| POST /api/v1/calibration/body/capture | BodyCaptureRequest | Async engine-owned capture completion; ApiResponse BodyProportionsResult |
| GET /api/v1/calibration/body/status | No body | Current athlete ID, height, known measured segments and quality states |
| POST /api/v1/calibration/xr-wrist/capture | XrWristCaptureRequest | Per-side offset results, or calibrated failures by side |
| DELETE /api/v1/calibration/{id} | Existing contract | Existing profile deletion; extend to invalidate related body/wrist profiles |

**Do not change** existing POST /api/v1/calibration/reference-pose (mounting) or POST /api/v1/calibration/profiles (camera-only import). Keep old client routes compatible.

BodyCaptureRequest contains:
- athlete_id:AthleteId matching the currently active athlete (server checks);
- expected_athlete_height_m:f64 (server checks current authoritative profile);
- duration_ms:Option<u64> (bounded by YAML);
- allow_partial:Option<bool> (default true).

XrWristCaptureRequest contains:
- athlete_id:AthleteId matching active;
- requested_sides:Vec<XrSide> (left and/or right, default both);
- expected_camera_fingerprint:String from Phase 02;
- expected_stage_session_id:String and expected_stage_generation:u64;
- expected_body_profile_id:ProfileId;
- declared_grip_revision:String (validated bounded ASCII fingerprint/identifier);
- requested duration field subject to YAML bounds.

Both routes must require local permissions per existing API policy; do not introduce remote unauthenticated sensor/calibration upload. No arbitrary point arrays are submitted by the Godot or desktop UI: the engine collects its own **accepted live observations** during the capture window.

### 7.2 Calibration runtime behavior

Define separate typed Commands in pipeline/command.rs:

~~~rust
Command::BodyCapture(BodyCaptureRequest, BodyCaptureReply)
Command::WristCapture(XrWristCaptureRequest, WristCaptureReply)
~~~

Add a small mutually exclusive **CalibrationOperation** enum in runtime rather than separate independent capture booleans. The preexisting one-second IMU mounting capture participates in this single operation guard. The runtime's select loop remains responsive; it collects sample values at normal engine ticks into bounded arrays and does not block for five seconds.

**States:** idle → preparing → collecting → evaluating → saving → complete / failed / cancelled.

- Preparing validates athlete identity/height, camera calibration, current live mode, no training, no recording/replay, no active conflicting calibration.
- Collecting captures triangulated camera wrist/shoulder/elbow samples at engine tick, uses latest accepted XR grip poses for controller calibration; caps max frames/pairs.
- On room/athlete/camera fingerprint/Stage change, cancel and notify with stable reason code.
- Evaluating uses pure solver helpers only (bounded memory, no unbounded SVD over raw images).
- Saving uses existing save_profiles, with rollback on persistence error and no prematurely published measured skeleton.
- Completing updates Store then selects/rebuilds Runner.skeleton, publishes SkeletonDefinition watch update, updates SystemState/quality and only then permits next body Frame.
- A successful **partial** profile lists all measured and assumed limbs by side. Failed side never inherits or mirrors measured opposite side.
- On engine shutdown, return Unavailable to capture client; do not leave operation or locked state hanging.
- Timed-out HTTP caller must not leave a detached active capture running forever: cancellation token plus deadline and explicit client disconnect policy.
- Persistent profile writes remain serial through existing runtime snapshot logic. No second JSON/SQLite per-athlete calibration database.

**Status reason codes:** athlete_missing, athlete_changed, athlete_height_changed, recording_active, training_active, playback_active, camera_calibration_missing, camera_identity_unconfirmed, camera_room_changed, camera_landmarks_stale, camera_keypoints_missing, camera_reprojection_high, tpose_not_stationary, elbow_not_straight, insufficient_unique_frames, segment_length_out_of_range, length_unstable, xr_uplink_unavailable, xr_alignment_missing, xr_alignment_stale, xr_clock_unsettled, xr_clock_uncertainty_high, xr_wrist_missing, controller_tracking_lost, controller_identity_changed, grip_pose_variation_insufficient, grip_wrist_residual_high, persistence_failed.

Use ApiErrorCode::Conflict for missing readiness/conflicting live operations and InvalidRequest for malformed values; keep reason codes structured and human-readable error text. Do not log exact athlete display names or raw session body measurements at INFO every frame.

### 7.3 UI

**Desktop**:
- Preserve the current [ReferencePoseFlow.tsx](https://github.com/danielsobrado/Bansayo/blob/192d7b468c028859c174f9fbd6a269a51b124450/apps/desktop/src/calibration/ReferencePoseFlow.tsx) as IMU mounting calibration, rename visible title to "Tracker mounting" if needed to distinguish.
- Add **BodyProportionsFlow.tsx** with athlete selector context, camera prerequisites, visible 3-second count-in, five-second measured pose capture, per-limb actual lengths and quality, assumed/not-measured labels.
- Add a separate **ControllerWristFlow.tsx** only when current XR pipeline, stage alignment and clocks are ready. Show per-side offset status and instructions to move controllers slowly; use one concise step panel, not a full-scroll form.
- Provide a direct recapture action and before/after comparison; never show a fake percentage accuracy.
- Reuse existing actions/state/query patterns and CSS conventions; no new UI framework or duplicate API client.

**Godot VR**:
- Add a compact **Body measurements** action to the existing setup panel only when engine is ready and the selected athlete is known.
- Optional XR wrist capture wizard may be invoked from VR and use existing controller inputs/menus without interfering with the Phase 02 alignment wizard.
- No new IMU mounting flow in VR. Do not expose a second body-solver or estimate proportions from Godot poses.
- Show calibration status measured/partial/assumed and actual lengths/error values. Keep headset camera and navigation independent of engine skeleton.

## 8 — YAML configuration and validation

Extend engine typed config as **body** or **body_calibration** under schema_version 2, default disabled for live measurement unless explicitly enabled. Prefer body_calibration to avoid ambiguity with the existing SkeletonConfig. All numbers below belong in typed config constants/defaults, not scattered in solvers. The active athlete height remains authoritative from AthleteProfile.

~~~yaml
schema_version: 2
skeleton:
  model: bansayo.body.v1
  height_m: 1.75
body_calibration:
  enabled: true
  capture_duration_ms: 5000
  min_unique_frames: 45
  max_frames: 600
  min_point_confidence: 0.70
  max_reprojection_error_px: 5.0
  min_views_per_joint: 2
  max_joint_velocity_m_s: 0.12
  max_length_mad_m: 0.012
  max_length_relative_mad: 0.04
  max_arm_elevation_from_horizontal_deg: 18.0
  max_elbow_deviation_from_straight_deg: 25.0
  min_measured_length_m: 0.10
  max_measured_length_m: 0.60
  min_length_to_height_ratio: 0.07
  max_length_to_height_ratio: 0.30
  allow_partial: true
  wrist:
    enabled: true
    capture_duration_ms: 4000
    min_paired_samples: 30
    max_pairs: 300
    max_skew_ms: 20
    max_clock_uncertainty_us: 5000
    min_rotation_span_deg: 20
    max_offset_norm_m: 0.25
    max_residual_rms_m: 0.030
    max_residual_p95_m: 0.050
    min_inlier_fraction: 0.80
~~~

The example may be added to **config/body-calibration.example.yaml** with the **required regular engine fields** and appropriate simulated camera sources **only after** the first full code gate is working. Do not check in an example that purports to produce camera-measured wrists without actual camera measurement data.

Add:
- body_calibration.enabled default false, so existing config/sim-full-body.yaml behavior stays unchanged.
- body_calibration.wrist.enabled effective only when body calibration and Phase 01/02 XR dependencies enabled and valid.
- Strict serde deny_unknown_fields and config/validate.rs ranges and cross-field constraints. min_unique_frames <= max_frames, min_paired_samples <= max_pairs, same for durations and thresholds.
- Schema-version 1 remains valid when section absent; explicit Phase 03 body_calibration requires schema_version 2.
- No secret, body length, athlete identifier or hardware model string hardcoded in Rust source.

## 9 — Precise implementation file manifest

New files must be small and single-purpose, as far as possible under roughly 250 lines. If a file becomes large, split pure solvers from runtime adapters/tests. Preserve existing public module boundaries.

| Change | File | Responsibility |
|---|---|---|
| Edit | crates/bansayo-contracts/src/calibration.rs | Add athlete_id to subject, grip wrist kind, BodyProportionsData/Quality and XrGripWristOffsetData/Quality |
| Edit | crates/bansayo-contracts/src/skeleton.rs | Optional per-segment measured/assumed provenance |
| **New** | crates/bansayo-contracts/src/body_calibration.rs | BodyCaptureRequest, BodyProportionsResult, WristCaptureRequest, WristCalibrationResult, status/error types |
| Edit | crates/bansayo-contracts/src/lib.rs | Export exact types |
| Edit | crates/bansayo-contracts/tests/contracts.rs | Golden wire compatibility, old subject missing athlete id remains decodable |
| Edit | crates/bansayo-contracts/tests/support/examples.rs | Typed body/wrist profile, request, partial measured skeleton |
| Edit | crates/bansayo-contracts/tests/support/generated.rs | TS generation of added types via BANSAYO_BLESS |
| Edit | crates/bansayo-engine/src/config/schema.rs | BodyCalibrationConfig and WristCalibrationConfig defaults, no hardcoded thresholds |
| Edit | crates/bansayo-engine/src/config/validate.rs | Strict cross-field and schema v2 validation |
| **New** | crates/bansayo-engine/src/calibration/body_capture.rs | Bounded camera landmark sample collector, T-pose/QA, per-side robust length estimate |
| **New** | crates/bansayo-engine/src/calibration/body_stats.rs | Median, MAD, temporal span/velocity and provenance helpers |
| **New** | crates/bansayo-engine/src/calibration/grip_wrist.rs | Pure wrist offset solver, synchronized paired camera/XR samples, robust statistics |
| **New** | crates/bansayo-engine/src/calibration/body_profiles.rs | Canonical profile construction, athlete/identity fields, validation and dependency fingerprints |
| Edit | crates/bansayo-engine/src/calibration/mod.rs | Export new pure modules |
| Edit | crates/bansayo-engine/src/calibration/store.rs | Body/wrist typed validation, lookups, validity states, staleness/invalidation and summaries |
| Edit | crates/bansayo-engine/src/skeleton/definition.rs | Factory for selective measured limb lengths and validated from_definition replay restore |
| Preserve | crates/bansayo-engine/src/skeleton/proportions.rs | Immutable default anthropometric ratios; no measured user constants here |
| Edit | crates/bansayo-engine/src/pipeline/command.rs | Distinct body and wrist capture commands/replies |
| Edit | crates/bansayo-engine/src/pipeline/runtime.rs | Single capture operation, bounded collection, atomic save, skeleton watch + state ordering |
| Edit | crates/bansayo-engine/src/pipeline/runner.rs | Selected-athlete skeleton loading, fallback rules, height revision |
| Edit | crates/bansayo-engine/src/training/mod.rs | Internal TrainingStatus athlete height/revision and active-athlete consistency |
| Edit | crates/bansayo-engine/src/training/athletes.rs | Active height changes generate new revision/status; preserve athlete profile contract |
| Edit | crates/bansayo-engine/src/training API handler | Reject athlete changes during active session/recording; actual handler path to be verified at implementation baseline |
| Edit | crates/bansayo-engine/src/api/mod.rs | New typed body/status/xr-wrist routes, existing errors/security |
| Edit | crates/bansayo-engine/src/registry/persistence.rs | Store old/new profiles in existing atomic snapshot, no new database |
| Edit | crates/bansayo-engine/src/recording/mod.rs | Subject-kind checks and exact recorded skeleton reconstruction |
| Edit | crates/bansayo-engine/src/recording/replay.rs | Validate measured geometry preserved on seek/replay |
| **New** | crates/bansayo-engine/tests/body_calibration.rs | Full deterministic API, athlete isolation, geometry, persistence and replay tests |
| **New** | crates/bansayo-engine/tests/grip_wrist_calibration.rs | Phase 02 dependent synthetic matched time/space offset tests |
| **New** | apps/desktop/src/calibration/BodyProportionsFlow.tsx | Separate body T-pose wizard and report |
| **New** | apps/desktop/src/calibration/ControllerWristFlow.tsx | Controller-to-wrist calibration wizard and source QA |
| Edit | apps/desktop/src/calibration/ReferencePoseFlow.tsx | Keep mounting functionality; clarify UI title |
| Edit | apps/desktop/src/engine/actions.ts | Typed API calls and status |
| Edit | apps/desktop calibration entry/router | Register body/wrist actions in existing UI, not a new workflow framework |
| **New** | apps/desktop/src/calibration/bodyCalibration.test.tsx | UI flow, partial measurement, error and athlete switch |
| **New** | apps/vr/scripts/body_calibration_status.gd | Optional compact VR calibration status; no local math |
| Edit | apps/vr/scripts/training_room.gd | Compact entry point and controller capture without UI overlap |
| Edit | apps/vr/scripts/status_panel.gd | Measured/assumed/provenance status |
| Edit | apps/vr/scripts/engine_actions.gd | Shared existing calibration action requests |
| **New** | apps/vr/tests/body_calibration_smoke.gd | Headless state/error/no-headset tests |
| **New** | config/body-calibration.example.yaml | Explicit opt-in sample valid after implementation |
| Edit | docs/05-calibration-and-fusion.md | Report actual implemented body calibration, not historical height-only |
| Edit | docs/13-implementation-status.md | Evidence and physical validation status |
| Edit | .github/workflows/ci.yml | Include deterministic new Rust/desktop/Godot tests |

**No changes to:** source detector inference model, camera calibration solver, IMU mounting algorithms, OpenXR controller aim UI, Phase 02 XR room alignment math, pipeline/solve.rs arbitration, Phase 04 elbow fusion or Phase 05 avatar skinning. Necessary exhaustive enum/serialization adjustments elsewhere are permitted if compiler/tests require them, but record any in final manifest.

## 10 — Named deterministic tests

### 10.1 Contracts

- [ ] body_subject_old_json_roundtrips_without_athlete_id.
- [ ] body_subject_new_profile_requires_athlete_id.
- [ ] body_proportions_accepts_only_four_supported_segment_keys.
- [ ] body_grip_wrist_kind_is_separate_from_mounting_and_xr_alignment.
- [ ] body_profiles_reject_nan_inf_negative_lengths_and_unknown_fields.
- [ ] body_partial_skeleton_provenance_serializes_and_old_clients_ignore_it.
- [ ] body_generated_types_and_golden_examples_current_after_bless.
- [ ] body_protocol_v1_legacy_messages_unchanged.

### 10.2 Robust T-pose length extraction

- [ ] body_coco17_indexes_shoulders_5_6_elbows_7_8_wrists_9_10_exact.
- [ ] body_length_identity_straight_arm_0p30_0p25_recovers_both_lengths: 60 deterministic frames spanning 5s.
- [ ] body_length_right_left_asymmetry_is_preserved.
- [ ] body_length_outliers_are_rejected_by_median_mad.
- [ ] body_length_camera_confidence_below_0p70_skipped.
- [ ] body_length_reprojection_over_5px_skipped.
- [ ] body_length_one_camera_view_cannot_claim_metric_calibration.
- [ ] body_length_bent_elbow_and_tpose_wrong_height_rejected.
- [ ] body_length_fast_arm_motion_rejected_by_velocity.
- [ ] body_length_reused_timestamp_does_not_inflate_sample_count.
- [ ] body_length_fewer_than_45_samples_marks_side_assumed.
- [ ] body_length_left_passes_right_fails_produces_explicit_partial_profile.
- [ ] body_length_both_fail_creates_no_valid_profile.
- [ ] body_length_out_of_range_and_height_ratio_fails.
- [ ] body_camera_only_calibration_works_with_no_xr_runtime.

### 10.3 XR wrist offset maths

- [ ] wrist_offset_identity_transform: known r=(0.025,-0.045,0.020)m recovered within 2mm on noiseless synthetic paired samples.
- [ ] wrist_offset_yaw90_transform_matches_camera_world_after_inverse_alignment.
- [ ] wrist_offset_left_right_devices_not_interchanged.
- [ ] wrist_offset_20ms_pairing_and_no_extrapolation.
- [ ] wrist_offset_unsettled_clock_refuses_calibration.
- [ ] wrist_offset_camera_wrist_occlusion_cannot_publish_offset.
- [ ] wrist_offset_rotation_span_below_20deg_fails.
- [ ] wrist_offset_reprojection_and_alignment_fingerprint_gate.
- [ ] wrist_offset_outliers_rejected_with_inlier_fraction_0p80.
- [ ] wrist_offset_rms_above_30mm_fails.
- [ ] wrist_offset_housing_tip_is_not_anatomical_wrist.
- [ ] wrist_offset_xr_recenter_invalidates_current_application.

### 10.4 Skeleton geometry and versioning

- [ ] skeleton_builtin_unchanged_without_body_profile: compare every existing SegmentDef.
- [ ] skeleton_left_upper_arm_override_changes_left_forearm_offset: tail and next child origin equal.
- [ ] skeleton_left_forearm_override_changes_left_hand_offset.
- [ ] skeleton_right_override_does_not_modify_left_arm.
- [ ] skeleton_no_leg_pelvis_torso_hand_length_changed.
- [ ] skeleton_partial_measured_provenance_keeps_unknown_assumed.
- [ ] skeleton_rebuild_publishes_definition_before_next_pose_revision.
- [ ] skeleton_rejects_invalid_recorded_topology_nan_and_bad_parent.
- [ ] skeleton_replay_restores_exact_recorded_metric_tails_and_offsets.
- [ ] skeleton_old_recording_without_new_provenance_still_replays.
- [ ] skeleton_fk_joint_angles_remain_consistent_for_existing_model.

### 10.5 Active athlete, persistence, integration

- [ ] body_active_athlete_A_never_gets_athlete_B_profile.
- [ ] body_switch_to_unknown_athlete_uses_height_assumed.
- [ ] body_height_change_marks_lengths_and_grip_offsets_stale.
- [ ] body_room_switch_keeps_valid_physical_body_lengths.
- [ ] body_camera_recalibration_does_not_invalidate_past_measured_length.
- [ ] wrist_offset_camera_change_does_invalidate_current_offset.
- [ ] wrist_offset_new_controller_without_stable_identity_invalidates_offset.
- [ ] body_calibration_rejected_during_recording_training_and_replay.
- [ ] body_athlete_switch_rejected_mid_training_or_recording.
- [ ] body_atomic_snapshot_failure_rolls_back_profile_and_skeleton.
- [ ] body_calibration_retries_after_device_unplug_without_persisting_bad_data.
- [ ] body_profile_survives_engine_restart_for_same_athlete_and_height.
- [ ] body_profile_delete_restores_height_only_assumed_skeleton.
- [ ] body_recording_replay_after_athlete_profile_deleted_is_bitwise_semantic_equivalent.
- [ ] body_imu_mounting_reference_pose_endpoint_unchanged.
- [ ] body_camera_only_and_imu_only_existing_fixtures_unchanged.

### 10.6 Commands

Run at project root after prerequisite phases are actually implemented:

~~~bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
BANSAYO_BLESS=1 cargo test -p bansayo-contracts
cargo test --workspace
cargo test -p bansayo-engine --test body_calibration
cargo test -p bansayo-engine --test grip_wrist_calibration
node scripts/verify-gate1.mjs
node scripts/verify-gate2.mjs

# Desktop: use the package manager/scripts pinned in the checked-in workspace.
# Confirm the exact package script names against current package.json before calling.
npm --prefix apps/desktop run typecheck
npm --prefix apps/desktop test

godot --headless --path apps/vr --editor --quit
godot --headless --path apps/vr --xr-mode off --script tests/contract_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/spatial_ui_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/xr_observation_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/xr_alignment_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/body_calibration_smoke.gd
~~~

**Command caveat:** npm scripts above are illustrative workflow names pending verification of current desktop package.json; the implementing AI MUST inspect that file and use the real scripts/package manager. Do not claim a failed or nonexistent command passed. Run contracts tests again **without BANSAYO_BLESS** and verify no generated changes. Mark any unavailable real hardware or tool environment **BLOCKED**, not green.

## 11 — Physical validation protocol

Save **docs/validation/YYYY-MM-DD-phase03-body-proportions-wrist.md** once measured, with commit SHA and distinct PASS/FAIL/BLOCKED for each item.

1. Select an active athlete with verified entered height; record only the anonymized AthleteId in shared QA reports.
2. Verify two or more calibrated real cameras, measured FPS, current room camera fingerprint and confirmed camera identities.
3. Stand in still T-pose for 5s, arms near horizontal, facing camera world +Z. Observe *actual* measured successful frames and reject reasons.
4. Compare camera-derived shoulder→elbow and elbow→wrist lengths with an independent tape/physical landmark measurement on each arm where feasible. Record **absolute signed error per segment**. Do not claim anatomical accuracy if the independent physical landmark is imprecise.
5. Repeat three captures. Measure repeatability per side; aim **<=2cm median absolute deviation among repeated estimates** as a provisional target; report observed variation and any detector-induced systematic bias.
6. Occlude right arm, keep left visible; confirm explicit partial success (right assumed), no fake mirror.
7. Induce a failed T-pose (bent elbow, fast motion, wrong body orientation). Confirm no new valid profile accepted.
8. With Phase 02 XR alignment valid, connect left/right grips and run slow controller motion capture. Report per-side paired sample count, clock skew p95, offset vector, residual RMS/p95.
9. Hold a known stable wrist pose, move controller slightly without changing the grip where physically possible; predicted camera wrist reconstructed from local offset should agree within **5cm provisional test threshold** on samples **not used for calibration**.
10. Change active athlete, room, camera calibration, XR stage and/or controller physical attachment separately. Verify the exact invalidation table.
11. Edit athlete height; ensure profile is stale and model fallback is safely assumed, not silently stretched while still labeled measured.
12. Record a short body/calibrated session; replay after changing active athlete and restarting engine. Verify identical recorded segment lengths, joint outputs and frame/skeleton revision consistency.
13. Confirm old mounting calibration is still functional; confirm Godot desktop and OpenXR camera controls remain unchanged.
14. Report camera exposure/timestamp uncertainty and any inability to prove physical anatomical landmark location. **No clinical accuracy claim**.

**Live numerical targets are provisional until measured:** >=45 unique accepted camera frames in 5s; length MAD <=12mm; repeated independent length estimate MAD <=20mm; wrist offset RMS <=30mm and p95 <=50mm; validation wrist reconstruction <=50mm on held-out motion; no stale/foreign-athlete reuse. If physical results fail, keep output assumed/stale or lower confidence, do not loosen thresholds silently.

## 12 — Exact implementation sequence: seven commits directly to main

### Commit 01 — Baseline and contracts

- [ ] Refresh main and verify **actual Phase 01 and Phase 02 code**, not only their docs.
- [ ] Record before-SHA and re-map target files and upstream line links.
- [ ] Add optional subject.athlete_id, XrGripWristOffset kind, typed profiles/quality/request/result and optional segment source metadata.
- [ ] Preserve old profile schema 1 and protocol 1, regenerate TS/golden.
- [ ] Commit: **contracts: define athlete-scoped body and grip-wrist calibration data**.
- **Gate:** all old JSON/wire tests pass, measured keys finite and strictly scoped.

### Commit 02 — Camera-only T-pose solver

- [ ] Add calibration/body_stats.rs and body_capture.rs.
- [ ] Sample accepted triangulated camera landmarks for five seconds under strict quality gates.
- [ ] Compute per-side median/MAD, point QA, partial status and invalid-sample reasons.
- [ ] Run deterministic synthetic COCO17 tests and existing camera regression tests.
- [ ] Commit: **calibration: measure body arm lengths from five-second camera T-pose**.
- **Gate:** no fake proportions from controller span or single-camera normalized landmarks.

### Commit 03 — Selective skeleton geometry and replay safety

- [ ] Add Skeleton::with_proportions and from_definition_validated.
- [ ] Apply new upper-arm/forearm tails **and matching child offsets**; keep other anatomy unchanged.
- [ ] Add per-segment measured/assumed state and skeleton revision tests.
- [ ] Update recording restore to use exact serialized geometry.
- [ ] Commit: **skeleton: apply measured arm dimensions with revision-safe replay**.
- **Gate:** old recordings load, all internal/world FK geometry matches published segment definitions.

### Commit 04 — Engine-owned capture and per-athlete persistence

- [ ] Extend Store per-kind validation and per-athlete lookup; add training-status height/revision.
- [ ] Add capture commands/routes through current single-writer runtime; serialize with IMU and XR captures.
- [ ] Add atomic save/rollback, athlete switch/height invalidation and active training conflict guard.
- [ ] Execute full athlete isolation, profile persistence, stale and camera-only engine tests.
- [ ] Commit: **engine: persist athlete body calibration and activate measured skeleton**.
- **Gate:** no cross-athlete contamination; no training/recording mid-session geometry changes.

### Commit 05 — Camera/XR grip-to-wrist translation calibration

- [ ] Confirm measured Phase 02 XR alignment, StageInfo and clock mapping on latest main.
- [ ] Implement calibration/grip_wrist.rs per §4 with correct world/stage algebra and paired timestamps.
- [ ] Add per-side typed XrGripWristOffset profile, strict provenance and status.
- [ ] Implement POST wrist capture via bounded engine runtime, no server fusion changes.
- [ ] Commit: **calibration: derive controller-local grip-to-wrist offsets from camera and XR evidence**.
- **Gate:** known synthetic offset recovers, no use when alignment/clock/landmark data unavailable.

### Commit 06 — Compact desktop and VR UI

- [ ] Keep old mounting flow intact, distinguish Body measurements and Controller wrist calibration.
- [ ] Add per-athlete measured/partial/assumed reports and per-side QA, no false percentages.
- [ ] Preserve existing Godot XR status, aim/trigger interactions and camera.
- [ ] Add desktop/VR smoke tests and regression tests.
- [ ] Commit: **ui: guide body T-pose and controller wrist calibration**.
- **Gate:** user sees measured sources and blockers, no action silently reuses prior athlete data.

### Commit 07 — Documentation, CI and hardware verification

- [ ] Run §10.6 exact supported commands and check generated files clean.
- [ ] Update docs/05-calibration-and-fusion.md, docs/13-implementation-status.md, app READMEs and CI.
- [ ] Capture physical arm measurement and XR wrist held-out QA or explicitly BLOCKED.
- [ ] Check git diff for unrelated changes and current main before final commit.
- [ ] Commit: **docs: validate Phase 03 body proportions and controller wrist calibration**.
- **Gate:** tests green, metrics actually measured, no unsupported accuracy claims.

## 13 — Explicit non-goals and downstream contract for Phase 04

- **Do not** run elbow IK or phantom elbow projection. Phase 04 uses calibrated shoulder/wrist positions, valid segment lengths and bend-plane confidence.
- **Do not** blend XR controller wrist into canonical skeleton in Phase 03; the live solver remains camera/IMU arbitration.
- **Do not** create full-body segment measurement from unobserved leg/torso lengths. Those remain assumed until independently calibrated.
- **Do not** calibrate head anatomical center from HMD elevation; do not assume grip span is shoulders.
- **Do not** introduce new YOLO/RTMPose models, 33-point foot landmarks, SteamVR driver, OSC/VMT, message broker, analytics database or GPL Unity package.
- **Do not** change physical camera calibration solver, OpenXR stage alignment or device clock estimation, except using their trusted outputs and invalidating dependent profiles.
- **Do not** introduce a new user profile database. AthleteProfile and existing RuntimeSnapshot are authoritative.

**Phase 04 consumer guarantees** after implementation: active athlete identity; height provenance; four arm lengths with per-segment measured/assumed states and quality; calibrated controller-local wrist offsets where available and valid; mapping into engine world via Phase 02; timestamp quality; explicit errors on missing/stale input; deterministic replay. Phase 04 must not use the controller offset when its physical controller/athlete/camera/Stage evidence does not match.

## 14 — Mandatory implementation AI completion report

Provide all of:

1. Phase 01 and Phase 02 prerequisite **implemented code SHAs**, then before/after Phase 03 main SHA.
2. Seven direct main commit links and exact source file inventory (additions/modifications).
3. Each cargo/Godot/desktop command actually run with PASS/FAIL/SKIP/BLOCKED, including generated-contract repeat check.
4. Sample camera triangulated shoulder/elbow/wrist QA frame and the resulting robust length per side; synthetic values labeled synthetic.
5. Active athlete per-profile IDs, height source, per-segment measured/assumed map, and proof that athlete switch cannot reuse other athlete's geometry.
6. Wrist offset equation/results per controller with camera/wrist quality, Stage/clock status and residuals.
7. SkeletonDefinition revision sequence and replay proof with measured limb geometry preserved after new athlete selection.
8. Profile invalidation/rollback tests (camera change, height edit, controller change, XR recenter, recording guard).
9. Independent physical validation report with actual error measurements, or **BLOCKED: real hardware not tested**.
10. Open TODOs, each with owner, file path, precise blocker and measurable acceptance criterion.

**Definition of done:** Bansayo can make a camera-supported, reproducible 5s T-pose measurement for an identified athlete; selectively apply trustworthy upper-arm/forearm dimensions while keeping remaining anatomy assumed; optionally estimate each grip-to-wrist local translation from synchronized calibrated camera+XR evidence; persist only valid typed calibration records; update skeleton revisions safely; replay exact recorded geometry; and leave current camera/IMU solver arbitration, desktop/VR controls and protocol 1 unaffected. **Phase 04 IK is explicitly NOT implemented here.**
