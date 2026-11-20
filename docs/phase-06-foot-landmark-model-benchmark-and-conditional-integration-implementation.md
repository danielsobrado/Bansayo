# Bansayo Phase 06 — Foot landmark model benchmark, licensing gates and conditional integration

**Status:** Detailed implementation specification, not implemented code.  
**Date:** 2026-11-20.  
**Target:** [danielsobrado/Bansayo](https://github.com/danielsobrado/Bansayo), direct commits to `main`.  
**Verified target source snapshot:** [b49e5038dec884474822ea7d102dad20c8543e71](https://github.com/danielsobrado/Bansayo/commit/b49e5038dec884474822ea7d102dad20c8543e71).  
**Reference source snapshot:** [maceq687/FullBodyPoseEstimation@be895709](https://github.com/maceq687/FullBodyPoseEstimation/commit/be895709c0bf86d35f64eb2cd5d2ffb4c6838593).  
**Prerequisites:** Phases [01](phase-01-openxr-observation-pipeline-implementation.md), [02](phase-02-openxr-room-alignment-and-time-synchronization-implementation.md), [03](phase-03-body-proportions-t-pose-and-controller-wrist-calibration-implementation.md), [04](phase-04-confidence-aware-xr-camera-imu-fusion-and-elbow-ik-implementation.md), and [05](phase-05-godot-humanoid-avatar-retargeting-and-vr-presentation-implementation.md) are designed; at this inspected commit they are **documents, not implemented XR/retargeting features**.  
**Following phase:** Phase 07 end-to-end physical camera/XR/IMU validation.  
**Primary goal:** Determine whether an actually licensed and deployable detector with left/right heels and toes measurably improves martial-arts foot observables over the current COCO-17 baseline, **without** damaging body tracking, replay, inference budgets or provenance.

> **No automatic model replacement.** The implementation consists of an always-valid **benchmark/contract/fixture path** and an explicitly **conditional, disabled-by-default** live integration. If no model artifact meets the independent license, accuracy and performance gates, Phase 06 can finish as **NO-GO** with an evidence report. The original COCO-17 detector remains Bansayo's fully functioning default and foot-dependent values remain unavailable. Model weights and evaluation videos are never checked into GitHub.

## 0. Current actual baseline, key correction and success criterion

As of target commit b49e503, Bansayo **already uses**:

- YOLOX-m person detector followed by RTMPose-m SimCC **COCO-17** (17 landmarks) through native Rust and ONNX Runtime with an optional DirectML provider;
- raw `Landmarks2d` `SensorObservation` events, bounded per-camera capture slots, multi-camera calibration and metric triangulation;
- a canonical **26-keypoint movement schema** where indices **22–25** are left_toe, right_toe, left_heel, right_heel, although the default COCO-17 detector provides none;
- foot-yaw/support-foot-pivot metric definitions that cannot be claimed measured without both toe and heel evidence.

The repository's [ADR-021](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/docs/architecture/martial-arts/adr/ADR-021-pose-estimation-integration.md#L19-L46) **already rejected deploying Halpe-26 weights** as of November 18 because their usage terms/training-data rights were not established. **MMPose's Apache-2.0 code license does not automatically license pretrained weights or upstream training images.** The plan must not override that decision via a casually downloaded model. Existing `models/README.md` forbids shipping current user-supplied weights and the existing RTMPose model itself has unresolved dataset-license risk.

### 0.1 Why this isn't simply “33 is better than 17”

The number of landmarks is **not** a validated accuracy metric. A 33-point model can have worse ankles, higher latency or worse body stability than the COCO-17 model. MediaPipe Pose supplies normalized 2D coordinates, model-relative `z`, and separate hip-origin model world landmarks; those **must not** be treated as Bansayo's calibrated world metres. The existing camera calibration/triangulation remains the sole route to metric foot points. Camera shutter blur and 30 FPS sample spacing remain fundamental error sources regardless of keypoint count.

**Non-negotiable acceptance:** Gain useful, calibrated **heel/toe support and foot yaw** with independent physical reference labels, preserve non-foot tracking and meet measured real throughput; otherwise stay COCO-17 and report *not ready*.

### 0.2 Concrete result and status states

Each evaluated candidate ends in exactly one of:

| Status | Meaning | Allowed production action |
|---|---|---|
| BLOCKED_LICENSE | Model weights/data distribution/use rights not verified | **No download, no runtime integration**, schemas may be tested synthetically |
| BLOCKED_MODEL | Model artifact/output/preprocessing/runtime unsupported or unavailable | No runtime integration; keep offline fixtures |
| FAIL_ACCURACY | Held-out heel/toe orientation or location gates fail | No runtime integration |
| FAIL_LATENCY | One/four-camera inference budgets fail | No runtime integration |
| PASS_EVALUATION | Licensed model beats acceptance gates with recorded measurements | Implementation may proceed behind opt-in feature flag |
| PASS_INTEGRATION | New path integrated, backwards compatible, full regression passing | Can offer optional detector; COCO-17 still default |
| BLOCKED_HARDWARE | Real video/multicamera reference missing | No claim of physical model superiority |

Never output a numeric improvement percentage or accuracy measurement unless actually obtained from the frozen evaluation dataset, with measurement method and confidence interval.

## 1. Exact upstream-to-target code reference matrix

### 1.1 Upstream reference: FullBodyPoseEstimation

| Source code in reference repository (immutable commit) | Verified concept | Bansayo interpretation |
|---|---|---|
| [VNectModel.cs L11–64](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L11-L64) | `PositionIndex` enumeration includes ankle, heel, toe/foot-index and separate hands | Study semantic distinction; do **not** copy its enum or Unity class. |
| [PoseVisualizer3D.cs L51–77](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/PoseVisualizer3D.cs#L51-L77) | MediaPipe/BlazePose 33-point observations, mapped into joint points | Retain detector-native 33 schema, not an incorrectly relabeled COCO17 output. |
| [PoseVisualizer3D.cs L79–123](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/PoseVisualizer3D.cs#L79-L123) | Derives body landmarks such as head/neck/hip from model outputs | Do not treat synthetic midpoints as directly measured; use canonical provenance. |
| [PoseVisualizer3D.cs L152–173](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/PoseVisualizer3D.cs#L152-L173) | Has body-dimension scaling/calibration helper | Never copy model-scale manipulation to calibrated multi-camera world. |
| [VNectModel.cs L169–214](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L169-L214) | Headset/controller fusion and phantom elbows | Not relevant to heel/toe model comparison; Phase 04 handles arms. |
| [LICENSE](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/LICENSE) | GPL-3.0 source/project | Independent Rust and evaluation scripts only. No source, Unity assets or models copied. |

### 1.2 Bansayo exact targets, code and current behavior

All target links below are **pinned** to b49e5038, not speculative future implementation.

| Bansayo source | Verified code fact | Phase 06 change |
|---|---|---|
| [sources/camera/pose.rs L1–28](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/sources/camera/pose.rs#L1-L28) | Hard-coded YOLOX/RTMPose COCO-17 names, input 192×256, `KEYPOINTS=17` | Split model profile from detector backend; keep current 17 defaults and add optional `K` from validated model metadata. |
| [sources/camera/pose.rs L45–90](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/sources/camera/pose.rs#L45-L90) | `session`, `Detection.points` and `Detector` tied to current fixed-model backend | Make model selector typed. No new whole-framework runtime without measured gain. |
| [sources/camera/pose.rs L192–250](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/sources/camera/pose.rs#L192-L250) | Preprocess 192×256 crop, RGB mean/std, ONNX outputs SimCC x/y; decoding divides by 17 | Reuse only for compatible RTMPose-SimCC candidates whose actual tensor output has verified `K` and preprocessing; reject outputs that aren't SimCC. |
| [sources/camera/pose.rs L255–300](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/sources/camera/pose.rs#L255-L300) | Per-camera subject-following boxes and redetect cadence, no confidence smoothing | Preserve subject selection/temporal tracking rules for fair comparison. |
| [sources/camera/capture.rs L251–269, L331–374](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/sources/camera/capture.rs#L251-L374) | Image-folder `landmarks.jsonl` demands exactly 17 triples or nulls | Add **versioned** multi-schema replay metadata without breaking legacy 17-line JSONL. |
| [sources/camera/mod.rs L132–173](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/sources/camera/mod.rs#L132-L173) | DeviceDescriptor.camera.landmark_schema always `coco17_schema()` | Return correct selected schema for each camera. |
| [sources/camera/mod.rs L319–477](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/sources/camera/mod.rs#L319-L477) | Capture threads timestamp frames; single inference loop serves all camera slots | Keep latest-frame/drop-oldest strategy, measure 1/2/4-camera delivered-rate/lag. |
| [sources/camera/mod.rs L483–608](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/sources/camera/mod.rs#L483-L608) | `landmarks_observation` defaults missing result to 17 nulls and stamps schema `coco17.body` and detector name | Produce the actual K-length and schema/detector identity, while keeping raw events untouched. |
| [pipeline/skeleton_fit.rs L15–66](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/pipeline/skeleton_fit.rs#L15-L66) | Constant COCO17 name/edges and canonical 17 schema | Keep COCO17 intact; add tested schemas for candidates. |
| [pipeline/skeleton_fit.rs L96–165](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/pipeline/skeleton_fit.rs#L96-L165) | `Keypoints::get()` understands canonical or COCO17 only; unsupported schemas do not fit | Extend with single shared schema map; no silently untracked torso/limbs on candidate model. |
| [pipeline/schema_map.rs L1–72](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/pipeline/schema_map.rs#L1-L72) | Closed mapping from COCO17 to `bansayo.body.v1.points`; no toe/heel mapping | Add candidate lookup tables, preserve old COCO17 and `None` for unsupported. |
| [pipeline/schema_map.rs L76–101](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/pipeline/schema_map.rs#L76-L101) | Tests prove 19–25 absent under COCO17 and weakest confidence on midpoints | Add mirror/left-right/foot index test matrix. |
| [pipeline/triangulate.rs L89–181](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/pipeline/triangulate.rs#L89-L181) | Weighted DLT, >=2 camera views, reprojection/geometry checks, sigma propagation | Do **not** replace solver. Give it correct same-schema 2D points and preserve sample-quality thresholds. |
| [pipeline/triangulate.rs L202–315](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/pipeline/triangulate.rs#L202-L315) | Matches camera samples with same schema and timestamp, returns native-schema world points | Require homogeneous active schema during triangulation, never silently mix 17/26/33 across cameras. |
| [contracts/src/landmarks.rs L8–16, L48–70](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-contracts/src/landmarks.rs#L8-L70) | LandmarkSchema and raw PoseLandmarks2D already have `schema_id`, detector and variable-length points | Reuse existing contract: no version bump, only publish validated schema descriptors. |
| [contracts/src/landmarks.rs L79–123](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-contracts/src/landmarks.rs#L79-L123) | ModelRelative and Triangulated world poses explicitly distinct, quality carries used_views and reprojection | Use **Triangulated / FrameRef::World only** for measured foot_yaw/heel height. |
| [contracts/src/motion.rs L1–36](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-contracts/src/motion.rs#L1-L36) | Canonical names array has left_toe=22, right_toe=23, left_heel=24, right_heel=25 | No 26-keypoint model confusion: this is the movement **output** canonical schema, not Halpe-26 input. |
| [pipeline/motion.rs L28–77](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/pipeline/motion.rs#L28-L77) | COCO17 or canonical world landmark→canonical measured keypoints; only COCO17 creates pelvis midpoint | Map candidate **world-triangulated heel/toe** through common tested table, preserving measured vs derived provenance. |
| [pipeline/motion.rs L79–254](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/pipeline/motion.rs#L79-L254) | Creates canonical BodyState with 26 slots, currently foot extras inferred | Change 22–25 only for validated observed world landmarks. Never mark an inferred foot roll measured. |
| [pipeline/capability.rs L256–299](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/pipeline/capability.rs#L256-L299) | Tracking identity includes detector name/schema and processing method; currently fixed camera model | Include model artifact SHA, output schema, preprocessing/profile revision and provider, independent from source ID. |
| [pipeline/features.rs L1–151](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/pipeline/features.rs#L1-L151) | Reports general capability and landmark status | Distinguish missing heel/toe detector, insufficient triangulation, stale samples and failed physical validation. |
| [config/schema.rs L455–495](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/config/schema.rs#L455-L495) | `CameraSourceConfig` has models_dir, execution, min scores and cameras; no model selector | Add typed `pose_model` config and optional model-specific profile. Default stays COCO17. |
| [config/validate.rs L585–690, L770–955](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/crates/bansayo-engine/src/config/validate.rs#L585-L955) | Validated CameraSourceSpec/current model checks and current camera count rules | Validate path, K, tensor metadata, hash and active schema consistency with path-qualified errors. |
| [models/README.md L1–57](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/models/README.md#L1-L57) | Local ONNX model SHA256/source and unresolved weight/dataset rights; no redistribution | Add optional tested candidate provenance **only after license pass**, no binary. |
| [scripts/setup.py L44–64, L174–182](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/scripts/setup.py#L44-L64) | Explicit pinned model downloads and checksums | No unknown-license model downloads. Add optional verified entry only after license approval, no auto-install. |
| [content/metrics/kick.yaml L66–133](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/content/metrics/kick.yaml#L66-L133) | Kick ankle height, ankle speed and support-foot pivot conditions | Keep names and scoring policy; new world feet may unlock foot_yaw only if real evidence passes. |
| [ADR-021 L19–46](https://github.com/danielsobrado/Bansayo/blob/b49e5038dec884474822ea7d102dad20c8543e71/docs/architecture/martial-arts/adr/ADR-021-pose-estimation-integration.md#L19-L46) | Specifically blocks Halpe-26 on license uncertainty; measured COCO17 p95 inference 22ms when YOLOX redetects | Preserve this decision and treat listed one-camera numbers as historical, not four-camera benchmarks. |

### 1.3 Official external source anchors

| Source | Grounded fact | Implementation implication |
|---|---|---|
| [Google MediaPipe Pose Landmarker](https://developers.google.com/edge/mediapipe/solutions/vision/pose_landmarker) | Pose Landmarker exposes **33** landmarks, normalized image x/y, and separate relative model "world landmarks" | Reconstruct image pixels correctly for offline comparison; don't use hip-origin world landmarks as calibrated room positions. |
| [Google MediaPipe pose landmark enumeration](https://github.com/google-ai-edge/mediapipe/blob/master/mediapipe/python/solutions/pose.py) | 29/30 left/right heel; 31/32 left/right foot-index | Semantic adapter must preserve exact left/right indices. |
| [MMPose Halpe26 dataset metadata](https://github.com/open-mmlab/mmpose/blob/main/configs/_base_/datasets/halpe26.py) | Halpe26 first 17 COCO joints; 20 left big toe, 21 right big toe, 22 left small toe, 23 right small toe, 24 left heel, 25 right heel | It uses **interleaved left/right toe types**, unlike COCO-WholeBody. License remains blocked unless new primary evidence changes it. |
| [MMPose COCO-WholeBody dataset metadata](https://github.com/open-mmlab/mmpose/blob/main/configs/_base_/datasets/coco_wholebody.py) | 133 points: first 17 COCO body; 17/18 left big/small toe, 19 left heel, 20/21 right big/small toe, 22 right heel | Optional future candidate; not selected by default due cost and irrelevant face/hand points. |
| [MMPose Apache-2.0 LICENSE](https://github.com/open-mmlab/mmpose/blob/main/LICENSE) | Library **code** licensed Apache-2.0 | Does **not** automatically establish model-weight rights. |
| [RTMPose whole-body model information](https://github.com/open-mmlab/mmpose/blob/main/configs/wholebody_2d_keypoint/rtmpose/README.md) | Candidate architecture/models for richer keypoints | Evaluate only where usage/license and actual ONNX output verified. |

The reference Unity code and Google/MMPose resources inform **semantics and research** only. Do not copy GPL code or assume a hosted model is legally redistributable or commercially usable merely because inference is technically possible.

## 2. Fixed model decision tree; no arbitrary engineering choice

### 2.1 Mandatory baseline

**Baseline B0**: Existing `yolox-m-humanart+rtmpose-m/raw`, `coco17.body`, unchanged ONNX/DirectML settings and matched input sequence. Benchmark with **exact same clips** and controlled tracked/untracked person-detector cadence; record separate body-17 accuracy and per-stage timing.

**Candidate M33**: MediaPipe Pose Landmarker **33** (Lite and Full profiles only where actual approved weight/runtime terms permit). Treat as **offline-only reference first** because its runtime/preprocessing/output are not the current RTMPose-SimCC ONNX interface. An ONNX export/runtime claim is invalid until an actual licensed, shape-correct export is tested with the frozen image set. Its "world" coordinates are diagnostic model-relative, not Bansayo world coordinates.

**Candidate H26**: RTMPose Halpe-26 output, potentially compatible with the existing SimCC path, **BLOCKED_LICENSE at the currently verified ADR-021 baseline**. Do not download or ship weights on an unsupported permission assumption. Perform mapping, synthetic tests and static config planning even if blocked. Reconsider only after a documented, primary rights grant clarifies model checkpoint use/distribution and training-data restrictions.

**Optional candidate W133**: COCO-WholeBody 133 for feet. Evaluate only if M33 fails deployment/accuracy/performance and a separately licensed 133-point artifact is verifiable. Do not commit a 133-point runtime until its face/hand cost is justified by measured foot gains. Its 17–22 indices differ from H26.

**Candidate selection:** First passing **deployable** model wins in this fixed order **H26 then M33 then W133**, because H26 is the smallest likely change to Bansayo's current ONNX SimCC path, not because its accuracy is assumed better. Compare quality of all licensed measured options; if a later model reduces held-out foot yaw MAE by **>=20% relative** to an earlier passing model without worse throughput, prefer the later model. If none passes, **keep B0**. Output a documented recommendation without pushing an unlicensed model.

**Important distinction:** M33 can be benchmarked with a separately installed development tool where rights permit, even when production deployment cannot pass; a research result is not a shipping permission.

### 2.2 License approval checklist (hard gate before artifact download/usage)

For each specific checkpoint or `.task` asset record:

- Original publisher and immutable model URL, release/tag/revision, checksum, format, purpose and training dataset/model card.
- License of **runtime code**, **model architecture**, **checkpoint weights**, **training annotations/images**, separately.
- Whether rights permit download, local use, internal commercial evaluation, packaging, redistribution, and derivatives. Use explicit evidence, no extrapolation from code license.
- Any copyright notices, attribution obligations, field-of-use limits or export restrictions.
- A project-approved conclusion and reviewer in `docs/validation/phase06-model-license-ledger.md`.
- If cannot verify: `BLOCKED_LICENSE`; no speculative binary in `models/`, no `scripts/setup.py` URL, no CI download.
- Never collect/share identifiable training videos without appropriate consent; use anonymized IDs and private dataset path.
- Treat imported source code separately; GPL upstream cannot be embedded in Bansayo under a permissive application claim without resolving compatibility.

No model license proof, no integration. This gate is independent of technical performance.

## 3. Exact keypoint crosswalks

### 3.1 Canonical output is 26 points, irrespective of detector count

Current `BODY_KEYPOINT_NAMES` in `contracts/src/motion.rs`:

```text
 0 pelvis       1 spine      2 chest      3 neck        4 head
 5 left_clavicle 6 left_shoulder 7 left_elbow 8 left_wrist
 9 right_clavicle 10 right_shoulder 11 right_elbow 12 right_wrist
 13 left_hip     14 left_knee     15 left_ankle
 16 right_hip    17 right_knee    18 right_ankle
 19 head_top     20 left_hand_tip 21 right_hand_tip
 22 left_toe     23 right_toe     24 left_heel 25 right_heel
```

The **body skeleton** remains the existing 19 segments. Adding heel/toe **landmarks** never adds heel/toe bones to the 19-segment Godot Skeleton3D and never modifies measured body proportions.

### 3.2 MediaPipe33: exact detector indices

```text
0 nose
11 left_shoulder, 12 right_shoulder
13 left_elbow, 14 right_elbow
15 left_wrist, 16 right_wrist
23 left_hip, 24 right_hip
25 left_knee, 26 right_knee
27 left_ankle, 28 right_ankle
29 left_heel, 30 right_heel
31 left_foot_index, 32 right_foot_index
```

Prescribed `mediapipe33.pose` → canonical 26 mapping:

| Canonical index | Source detector index | Rule |
|---|---|---|
| 0 pelvis | (23,24) | Hip midpoint, derived; min confidence |
| 3 neck | (11,12) | Shoulder midpoint, derived; min confidence |
| 6/10 shoulders | 11/12 | Direct detector keypoints |
| 7/11 elbows | 13/14 | Direct |
| 8/12 wrists | 15/16 | Direct |
| 13/16 hips | 23/24 | Direct |
| 14/17 knees | 25/26 | Direct |
| 15/18 ankles | 27/28 | Direct |
| 22/23 toes | 31/32 | Direct **foot-index approximation**, not guaranteed terminal toe tip |
| 24/25 heels | 29/30 | Direct |
| Others | None | Missing, not guessed from head/hand indices |

**No** using MediaPipe model-relative z as world z, no translating its hip-centred world coordinates by a camera pose. Convert **normalized 2D x/y** to original image pixels with exact image size, crop/rotation/mirroring rules from the specific inference adapter. `presence` and `visibility` are not detector confidence interchangeable with RTMPose SimCC; calibrate/gate per model and record the model semantics.

### 3.3 Halpe26: exact detector indices if a licensed checkpoint becomes available

- 0–16 = standard COCO17 body order.
- 17 head, 18 neck, 19 hip (model estimates, not anatomical authority).
- 20 left big toe, 21 right big toe.
- 22 left small toe, 23 right small toe.
- 24 left heel, 25 right heel.

Map canonical 22 left_toe as the **midpoint of indices 20+22** if both valid, or `None` if either missing; 23 right_toe from 21+23; 24 left_heel=24; 25 right_heel=25; COCO body 0–16 maps exactly as the current COCO17 source. **Never** use H26 indices 17–22 as the COCO-WholeBody foot set — that would interpret head/neck/hip as feet.

### 3.4 COCO-WholeBody133 optional candidate

- 0–16 COCO17 body.
- 17 left_big_toe, 18 left_small_toe, 19 left_heel.
- 20 right_big_toe, 21 right_small_toe, 22 right_heel.
- Canonical 22 left_toe=midpoint(17,18) requiring both; 23 right_toe=midpoint(20,21), 24 left_heel=19, 25 right_heel=22.
- Face and finger keypoints **do not** enter Phase 06 canonical movement or change the hand/face pose.

**Quality for derived toe midpoints:** min input confidence, explicit derived provenance; in 3D, both 3D source points must pass the same room/calibration/time gates and have verified compatible used_views (prefer at least two shared calibrated cameras). Propagate sigma conservatively; missing uncertainty never becomes a falsely validated sigma.

### 3.5 Output contracts and schema identity

Use exactly these detector-native `LandmarkSchema.id` values:

```text
coco17.body             # legacy; do not rename
mediapipe33.pose        # offline candidate, live only after gate
halpe26.bodyfeet        # schema testable with synthetic input
coco_wholebody133.pose  # optional, may remain test-only
```

All `LandmarkSchema.names` and `edges` are declared as fixed tested tables with their **exact number of names**. No dynamic guessing "K=26 therefore Halpe". The `detector` string includes model **family/revision/preprocessing mode**, while tracking/calibration comparability identity includes **model artifact SHA256**.

**No need to change protocol v1**, `Landmark2D`, `LandmarkSchema`, `PoseLandmarks3D` or `BodyState.keypoints` length. Raw 2D observations still carry native 17/26/33/133 points; canonical conversion is a **downstream derived view**, not a rewrite of recorded source events.

## 4. Fixed benchmark corpus and acquisition

### 4.1 Physical input requirements

Capture a reproducible, **private, consented** evaluation corpus with camera IDs, calibration references, exposure and measured frame rates, rather than choosing hand-picked internet videos with inconsistent annotation.

**Minimum corpus** (a gate, not a claim of existing data):
- **24 clips**, **6 movement classes ×4 repetitions**; each 10–20s, two or more cameras when available.
- Classes: stationary T-pose/feet planted; slow front kicks; fast front kicks; roundhouse kicks; support-foot pivot; occlusion/turning/step-through.
- **12 repetitions left-dominant and 12 right-dominant** distributed across classes where mechanically relevant.
- At least **3 participant identifiers** if permitted and practical. If fewer subjects, mark generalization **BLOCKED**, not PASS. Split by **participant**, not adjacent frames, to avoid leakage.
- Record camera resolution, intrinsic/extrinsic calibration fingerprint, actual delivered FPS, exposure mode, lighting and blur, model runtime hardware/provider and device timestamps when available.
- For 3D reference, use independently measured **rigid foot markers or surveyed markers** on shoe toe/heel **or** an external reference tracking system with documented accuracy. 2D annotation alone **cannot** certify centimetre-scale 3D error.
- For yaw/support-pivot reference, mark shoe forward axis plus foot-ground contact periods and turn angle with a defensible reference method.
- No smoothing or extra temporal prediction on recorded raw detector landmarks; scoring can analyze temporal stability separately.
- Never infer 60 FPS footage from resampled 30 FPS videos; assess real frame-to-frame capture limits.

**Privacy/ownership:** Save raw videos externally to GitHub; record dataset id/manifest checksums and paths only, never faces or identifiable names in repository. Benchmark code must accept a manifest path supplied by operator, not hardcoded individual data paths.

### 4.2 Benchmark manifest (new versioned format)

New `benchmarks/foot_pose/manifest.example.yaml`:

```yaml
schema: bansayo.foot-benchmark/v1
dataset_id: phase06_reference_v1
root: /data/private/bansayo-foot-benchmark
room_calibration_id: room-floor-v1
clips:
  - id: subject-a-front-kick-left-01
    subject_group: group-a
    movement: front_kick
    side: left
    cameras:
      - camera_id: CAM-A
        file: clips/a-front-01-cam-a.mp4
        calibration_profile_id: cam-a-extrinsics
        resolution_px: [1280, 720]
        capture_rate_hz: 30.0
      - camera_id: CAM-B
        file: clips/a-front-01-cam-b.mp4
        calibration_profile_id: cam-b-extrinsics
        resolution_px: [1280, 720]
        capture_rate_hz: 30.0
    labels_2d: labels/a-front-01-2d.jsonl
    reference_3d: labels/a-front-01-reference-3d.jsonl
    contact_labels: labels/a-front-01-contact.jsonl
```

The example is **documentation only**, not a claim those files exist. Validate exact schema, unique clip IDs, frame timestamps increasing, camera references, point units and mutually consistent image sizes; never auto-download datasets. User paths in a YAML example are placeholders resolved only at runtime.

### 4.3 Two separate benchmark tiers

**Tier A — local synthetic/contract proof, mandatory and fully CI runnable:**
- COCO17, M33, H26, W133 **mapping** from fixed fixture arrays with unambiguous left/right values, nulls and confidences.
- Synthetic multi-camera projections with known camera K/extrinsics, 3D heel/toe ground truth, known yaw and occlusion.
- Bounded raw JSONL fixture for 17 and v2 variable-point count.
- No pretrained model files or internet needed. Proves mechanics, **not detector accuracy**.

**Tier B — real learned-model comparison, mandatory before GO:**
- Rights-approved candidate weights.
- Same private image frames/ROIs fed to baseline and candidate with reproducible preprocessing and identical ground-truth labels.
- Head-to-head 2D plus calibrated 3D evaluations, time synchronization, burst/occlusion and 1/2/4 camera throughput.
- Report each model's actual confidence convention and calibration; no apples-to-oranges confidence thresholds.

**If Tier B blocked:** finish the Phase 06 report as `BLOCKED_HARDWARE` or `BLOCKED_LICENSE`; keep COCO17 default, and do not label Phase 06 detector improvement implemented.

## 5. Metrics and exact GO/NO-GO thresholds

### 5.1 Independent test metrics

Compute on a fixed held-out participant/clip split, no tuning to the holdout:

| Metric | Definition | Threshold for model GO |
|---|---|---|
| 2D foot landmark error | Euclidean pixel error normalized by image diagonal, scored per visible annotated left/right toe/heel | Median <=0.015 and p90 <=0.030 (provisional) |
| 2D foot usable coverage | Proportion of **labeled-visible** frames with both toe and heel detected, passing confidence | >=0.80 for slow/static, >=0.65 for fast/pivot, each side |
| 3D heel/toe error | Distance to independent world-metres reference from **>=2 calibrated cameras**, metric evidence | Median <=0.040m, p90 <=0.080m |
| 3D direction yaw error | Wrapped angle difference vs reference of ground-plane heel→toe forward vector | Median abs <=10°, p90 <=20° |
| Support pivot error | Abs difference of unwrapped yaw delta vs independent support foot rotation | Median abs <=12° per tested pivot |
| Left/right foot identity swaps | Wrong limb assigned across a clearly labeled event | 0 on holdout where identity is unambiguous |
| Non-foot body regression | Error on common 17 COCO joint semantics vs B0 at same images | Median errors no worse by >5% relative; no new severe tracking loss |
| 3D foot time availability | Percentage of time all required world landmarks pass measured evidence quality | >=0.70 across full applicable maneuver segments |
| Per-camera tracked inference latency | End-to-end pose stage with subject already tracked, actual reference workstation | p95 <=12ms |
| Redetect inference latency | Including YOLOX every 10th/subject-reacquire frame | p95 <=25ms; no unbounded backlog |
| 4-camera throughput | Four genuine feeds nominal 30 FPS simultaneously, current GPU/provider | Each delivered >=90% requested rate, with p95 frame age <=100ms |
| Memory growth | 10-minute continuous run, warmed model | No unbounded growth; quantified MB delta and peak |
| Compatibility | Golden protocol-v1/old recordings, camera-only, IMU-only | 100% existing regressions pass |

These thresholds are **engineering gates to be tested**, not experimentally proven performance. On 60 FPS, target 4×60 inference may be impossible with one shared thread: **do not assume throughput**. Benchmark at real 30 FPS and optionally 60 FPS; report 60 as BLOCKED/FAIL when not feasible.

### 5.2 Why no meaningless “20% better than COCO17 feet” metric

B0 has **no heel/toe labels**; it is invalid to calculate a relative heel/toe error improvement against its missing output. Use **absolute accuracy/coverage** for new feet; compare **shared body-17** error/latency to baseline.

When comparing **two feet-capable candidates**, prefer a model with **>=20% relative reduction in held-out median foot yaw error**, with no violation of any other hard gate. Report numerator/denominator sample counts, side, movement and dataset split. If coverage differs, also report errors on the **same jointly visible frames**, not just easiest predicted subset.

### 5.3 Statistical reporting

- Bootstrap **by clip or participant**, not individual video frames (which are temporally correlated), 1000 resamples with fixed seed **20261120**.
- Report 95% confidence intervals for median 2D/3D/yaw error and coverage; if cohort too small, state wide uncertainty.
- Report p50/p95/p99 latency as actual observed time distributions and release-build GPU/CPU provider, full YOLOX cadence and cache warmup.
- Pre-register primary outcome = held-out support-foot yaw error under real kicks, secondary = 3D foot error/availability, tertiary = body-17 regression and runtime cost.
- Never select a candidate solely because it tracks more landmarks on static indoor video.
- If physical ground-truth accuracy is unavailable, mark 3D and foot yaw **BLOCKED**, no production GO even if 2D/latency look good.

## 6. Geometry and provenance for new foot observables

### 6.1 Foot yaw from calibrated metric heel/toe

The world frame is +Y up; define measured foot forward horizontal vector:

```text
heel_world = TriangulatedLandmark3D.m(canonical left/right heel)
toe_world  = TriangulatedLandmark3D.m(canonical left/right toe)
d = toe_world - heel_world
forward_xz = (d.x, 0, d.z)
l_xz = sqrt(d.x*d.x + d.z*d.z)

if l_xz < 0.08 m:
    foot_yaw = UNAVAILABLE; reason foot_direction_degenerate
else:
    yaw_rad = atan2(forward_xz.x, forward_xz.z)  // relative to world +Z
```

Because foot toe/heel are detector-specific **landmarks**, not anatomically certified vectors, label orientation `Derived` from **measured triangulated** foot points. A 2D-only observation may produce a **planar diagnostic** only with a known camera view; it must **not** populate absolute 3D world foot yaw or support pivot.

**Strict quality to calculate yaw:**
- Two current room-calibrated camera views contributing to each world point; matching active room/calibration fingerprint, accepted temporal skew.
- Each point confidence >=0.60 (tune only on training split), reprojection RMS <=5.0px, sigma if present <=0.050m (not validated accuracy bound).
- Toe/heel timestamps from same accepted triangulation result; no stitching across different poses without declared time interpolation.
- Horizontal projected forward length >=0.08m and actual 3D foot length within **0.10–0.40m**; otherwise unavailable.
- No world/model-relative frame confusion.
- If airborne, foot yaw may still be meaningful but **support-foot pivot** is disabled unless support ground-contact evidence independently qualifies.

**Do not** derive foot yaw from `ankle→toe` when heel is unavailable and label it measured heel/toe orientation. Such a calculation can be a lower-confidence separate method in a later phase but not Phase 06.

### 6.2 Support-foot pivot and heel lift

- Determine support side from existing training/tracking context (exercise stance, active vs supporting foot), not automatically from a single camera's "lower ankle".
- Require toe, heel and ankle world observations with independent quality. For planted-foot pivot, require toe/heel vertical proximity to verified world floor within **0.06m** (specific shoe/platform offsets require measured calibration; otherwise support contact unknown).
- For an accepted contiguous planted period, unwrap yaw and compute `delta_yaw_rad = yaw(t_end)-yaw(t_start)` with standard wrap correction in [-pi,pi].
- A lifted heel is a different physical event; existing `content/metrics/kick.yaml` has a `heel_height.support >0.08m` gate. Use *observed world heel y* only when world floor and shoe offset are validated. Do not simultaneously assume planted heel and lift >=0.08m. The existing scoring rules decide applicability.
- If support contact is uncertain, emit unavailable reason `foot_contact_unverified`; do not infer balance or pivot skill.
- Use at least **3 contiguous valid frames** for a pivot segment and no gap >100ms; do not interpolate long missing periods into a measured rotation.
- Measured ankle-height/kick-height metrics remain unchanged; do not silently promote all foot metrics from "not implemented" to "measured" because heel/toe appear.

### 6.3 Data provenance and measurement interpretation

Keep existing `MethodId::Triangulated`, `Evidence::provenance`, `sigma_basis=Propagated`, `t_obs_us` and `sources` for new canonical foot keypoints. Midpoints of big/small toes are **Derived** from two triangulated model detections; direct MediaPipe foot-index/heel points may be `Measured` as detected physical-image landmarks **only after triangulation**, not anatomical ground truth. The foot **orientation is Derived**, not a direct sensor observation.

Do not report a confidence probability of 100%. Confidence from MediaPipe is different from SimCC score. Treat unknown uncertainty as None; no guessed sigma.

## 7. Source adapter and compatibility architecture

### 7.1 Minimal source restructuring (once a candidate passes)

Split the large `sources/camera/pose.rs` only if needed for the new output backend:

```text
sources/camera/
  pose.rs                    # existing high-level Detector facade and shared Detection API
  pose_models.rs             # NEW model profile/schema/expected tensor metadata
  pose_simcc.rs              # NEW compatible RTMPose SimCC decoder, K typed
  pose_mediapipe.rs          # NEW ONLY IF a rights-approved supported runtime export passes GO
  capture.rs                # existing, versioned image-folder landmarks
  mod.rs                    # existing one shared inference worker and device descriptors
```

**Keep `Detection {bbox,score,points}` and `track()`** semantics unless a change is essential. The active model profile supplies `model_path`, `schema_id`, expected keypoint count, model hash, runtime adapter, input shape, preprocessing/decoding profile and detector version. **No polymorphic plugin loader or database** in this phase. A simple sealed enum suffices:

```rust
enum PoseModelId {
    RtmposeCoco17,
    RtmposeHalpe26,
    MediapipePose33,
}
enum PoseRuntime {
    SimccOnnx,
    ApprovedMediapipe,
}
struct PoseModelProfile {
    id: PoseModelId,
    schema_id: &'static str,
    keypoints: usize,
    runtime: PoseRuntime,
    model_sha256: String,
    preprocessing_id: &'static str,
}
```

The `MediapipePose33` runtime enum variant can remain offline-only and must not be selectable in production unless a verified binary/runtime implementation exists and benchmark passed. Do not fabricate an `ort` Session capable of reading a `.task` directly.

### 7.2 Detector output shape checks

For SimCC candidate models inspect actual ONNX session metadata:
- Input tensor: batch×3×height×width with expected dtype/layout, no dynamic alternate shape accepted silently.
- Two outputs named `simcc_x` and `simcc_y`, shapes `[batch,K,x_bins]` and `[batch,K,y_bins]`; same `K` equal profile keypoints and reasonable split ratio.
- Reject K mismatch with `pose_model_output_mismatch` before decoding, not panicking on integer division.
- Keypoint coordinate offset from crop affine transform must exactly invert preprocessing; test with a fixed landmark-centre synthetic model output and original frame corners.
- Candidate's confidence convention must be documented; never automatically inherit COCO17's score cutoff as calibrated probability.
- Unavailable detection emits `vec![None; K]`, with proper schema. No stale last 17 points leaking through `Track`.
- Detector name/status and camera descriptor schema match exactly; changed model invalidates previous cached `Track`, but should not alter camera intrinsics/extrinsics.

### 7.3 Raw observation and recording backward compatibility

Keep the existing raw `SensorObservation::Landmarks2d` event schema and MCAP recording. The `LandmarkSchema` descriptor travels with camera device status and `PoseLandmarks2D.schema_id`; every length must match the corresponding descriptor.

Extend **image-folder recorded landmarks** as follows:
- Legacy `landmarks.jsonl`: exactly 17 entries, no manifest, implicitly `coco17.body` and existing fixture semantics; all old tests pass unchanged.
- New optional `landmarks.meta.json` in a folder: exact `{ "schema_version":2, "schema_id":"mediapipe33.pose", "detector":"...", "count":33, "source_model_sha256":"...", "preprocessing_id":"..." }`, strictly typed and validated; 26/33/133 entries allowed only with a matching recognized schema. Keep array JSONL format but point count now from metadata.
- If manifest absent and row count !=17, reject with actionable message; **never guess schema by length**, especially 26 canonical vs Halpe26.
- Record raw events **before** any canonical interpolation/mapping/filter; replay should restore exactly the previous schema/model and produce identical canonical foot observables.
- If model changes while a recording or active training session is in progress, reject the switch; do not mix comparability classes in a single scored session.
- Store all model ids/hashes in existing `tracking_config_id` comparability identity and recording context. No protocol-version bump.

### 7.4 Camera mixing rule

The current triangulation code aligns samples with the **same schema**; preserve this behavior. For Phase 06 each **active room's calibrated multicamera reconstruction set must use one detector schema**. A partly migrated room (one COCO17, one 33) may still show per-camera 2D previews but is **not eligible** for mixed-schema 3D or foot-yaw scores.

Do not silently map COCO17 and M33 2D to 26 canonical positions before triangulation to artificially make them "compatible": detector biases differ, and the mapping may create false cross-model geometry. Mixed-detector 3D triangulation is a future separately validated algorithm. UI must report `camera_pose_schema_mismatch` and the configured camera IDs.

## 8. Canonical adapter and solver wiring

### 8.1 Single semantic source mapping

`pipeline/schema_map.rs` currently maps COCO17 2D into canonical points, while `skeleton_fit.rs` and `motion.rs` have separate 3D mapping. Avoid a third source of index truth.

Prescribe one **schema definition table per model** in a new `pipeline/pose_schema.rs` (pure, small), with:
- Model schema id.
- Native landmark names + edges, fixed expected count.
- Canonical mapping entries (direct or 2-point midpoint), with semantic label and expected confidence rule.
- Left/right pairs and foot extras.
- A helper `canonical_sources(schema_id, canonical_index)` that returns a static source list for **both 2D and 3D** adapters.

During rollout, keep existing COCO17 hot path unchanged and prove the new static mapping yields identical outputs before refactoring to the shared helper. `schema_map.rs::to_canonical` uses it for 2D; `motion.rs::measured_keypoints` uses it for triangulated 3D; `skeleton_fit.rs::Keypoints::get` uses same direct/derived references for body axes. This removes duplicated unsafe index logic without replacing triangulation.

**3D midpoint semantics**:
- Require both detector points available and **same camera-calibration/time origin**.
- Use mean world metre position; confidence=min confidences, used_views **intersection** for requiring shared supporting cameras (not naive union that can create fictitious multiview validity), and sigma conservatively >= both source estimates unless validated propagation is available.
- Method `Derived` and confidence not higher than source; never call derived anatomical neck/head measured.
- If used_views intersection <2, suppress metric derived point; keep raw metric source points available for diagnostics.
- The old COCO17 pelvis midpoint behavior is a compatibility fixture; if its legacy used_views union differs, preserve byte-for-byte old results with an explicit legacy branch and flag the improvement separately for later, not as a silent Phase 06 regression.

### 8.2 Skeleton fit and foot geometry

The 19-segment skeleton has no heel/toe bone. `skeleton_fit.rs` should use new candidate body joints (shoulder/elbow/wrist/hips/knees/ankles) exactly as current COCO17 does, but may use **metric heel-to-toe vector** to derive **foot segment swing** if confidence and joint frames support it.

**Chosen Phase 06 boundary:**
- **Do not** modify current skeletal foot rotation in Phase 06: the body avatar stays driven by existing camera/IMU/FK, while heel/toe world observations flow to movement observable quality/metrics. This prevents a detector experiment from destabilizing feet in VR.
- Foot-bone rotation from heel/toe belongs to a later gated source arbitration extension after a held-out angle benchmark; current work provides world-derived foot yaw independent of rest pose.
- Check calf/ankle/kick existing angles unchanged for same COCO17 data.
- No avatar skinning or Phase 04 arm fusion changes.
- Existing `MotionSample` 26 slots are sufficient; only 22–25 become present with verified model and calibration.

## 9. Exact YAML setup and central constants

Add **opt-in** model selection inside the existing camera source, not a second camera service. Existing camera YAML with no `pose_model` behaves exactly as today.

```yaml
schema_version: 2
sources:
  - kind: camera
    id: cameras
    enabled: true
    models_dir: ./models
    execution: direct_ml
    min_person_score: 0.5
    min_keypoint_score: 0.3
    pose_model:
      id: rtmpose_halpe26
      path: ./models/rtmpose-halpe26.onnx
      expected_sha256: "<verified-model-sha256>"
      schema_id: halpe26.bodyfeet
      preprocessing_id: rtmpose-simcc-256x192-v1
    cameras:
      # TODO: insert real camera entries using the existing validated format.
      []
```

**Important:** This is a **section-level illustration**, not a complete runnable camera configuration; empty `cameras` is not operational, and the angle-bracket checksum is not a real artifact hash. The actual checked-in `config/feet-model.example.yaml` must contain a fully valid documented **image-folder fixture** or explicitly state it is a template and not runnable. Do not commit nonexistent model hashes or real hardware identifiers.

Proposed typed config:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PoseModelConfig {
    #[serde(default)]
    id: PoseModelId,        // default: RtmposeCoco17
    path: Option<PathBuf>,
    expected_sha256: Option<String>,
    preprocessing_id: Option<String>,
}
```

The `schema_id` must be **derived from the model ID**, not freely user-set, to prevent a 17-point checkpoint being misidentified as 33. Explicit user supplied `schema_id` in examples should be validated against the profile or removed in final code; prescribed final approach **derive**, remove it from runnable YAML. All model IDs and default filenames belong to a central models module, no scattered strings across source and analytics.

**Evaluation config** (new `benchmarks/foot_pose/evaluation.yaml`):

```yaml
schema: bansayo.foot-benchmark-config/v1
seed: 20261120
bootstrap_repetitions: 1000
minimum_clips: 24
minimum_subjects: 3
minimum_2d_foot_coverage_slow: 0.80
minimum_2d_foot_coverage_fast: 0.65
minimum_3d_foot_coverage: 0.70
max_2d_median_diagonal_fraction: 0.015
max_2d_p90_diagonal_fraction: 0.030
max_3d_median_error_m: 0.040
max_3d_p90_error_m: 0.080
max_yaw_median_error_deg: 10.0
max_yaw_p90_error_deg: 20.0
max_pivot_median_error_deg: 12.0
max_body17_relative_regression: 0.05
tracked_inference_p95_budget_ms: 12.0
redetect_inference_p95_budget_ms: 25.0
four_camera_min_delivered_fps_ratio: 0.90
four_camera_max_frame_age_p95_ms: 100.0
yaw_min_ground_projection_m: 0.08
foot_length_range_m: [0.10, 0.40]
required_metric_views: 2
required_point_confidence: 0.60
max_foot_reprojection_error_px: 5.0
max_foot_sigma_m: 0.050
```

**Code validation** uses strict nonfinite/range checks and enforces `min <= max`. Never put thresholds only in report text or hardcode them into hot-path code. Default model file path must remain the existing `models/rtmpose-m.onnx` unless explicitly changed.

## 10. File-level implementation manifest

All new files should be small and SOLID-focused (roughly <=250 lines each where reasonable). Do not create a huge new inference framework. This manifest is prescriptive **subject to refreshing newest main**.

| Operation | Exact path | Responsibility / change |
|---|---|---|
| **New** | `benchmarks/foot_pose/README.md` | Consent, label definitions, dataset acquisition, reproducibility, model status |
| **New** | `benchmarks/foot_pose/manifest.example.yaml` | Versioned private corpus manifest, no actual private videos |
| **New** | `benchmarks/foot_pose/evaluation.yaml` | Fixed thresholds, seed, metrics and bootstrap settings |
| **New** | `benchmarks/foot_pose/validate_manifest.py` | Offline YAML/JSONL manifest and timestamp label validation, no model inference |
| **New** | `benchmarks/foot_pose/score.py` | Pure offline 2D/3D/yaw/pivot score and grouping statistics |
| **New** | `benchmarks/foot_pose/report.py` | Machine-generated JSON and human-readable Markdown evidence report |
| **New** | `benchmarks/foot_pose/mediapipe_reference.py` | **Optional offline-only** 33-point inference with model/license gate; never called by production Rust |
| **New** | `benchmarks/foot_pose/tests/test_metrics.py` | Deterministic annotated fixture, error, yaw wrap and bootstrap tests |
| **New** | `docs/validation/phase06-model-license-ledger.md` | Evidence for each artifact, blocked permission explicit |
| **New** | `crates/bansayo-engine/src/pipeline/pose_schema.rs` | One closed model-specific raw/26-canonical map and per-schema table |
| **Edit** | `crates/bansayo-engine/src/pipeline/schema_map.rs` | Use common map for 2D while preserving exact COCO17 output |
| **Edit** | `crates/bansayo-engine/src/pipeline/motion.rs` | Map 3D world heel/toe into existing canonical 22–25, correct provenance |
| **Edit** | `crates/bansayo-engine/src/pipeline/skeleton_fit.rs` | Accept candidate body anchors without changing default COCO17 fit; no foot rotation change |
| **New** | `crates/bansayo-engine/src/pipeline/foot_observables.rs` | Pure 3D heel/toe vector, wrapped yaw and support pivot evidence guards |
| **Edit** | `crates/bansayo-engine/src/pipeline/mod.rs` | Export shared pure modules |
| **Edit** | `crates/bansayo-engine/src/pipeline/capability.rs` | Model hash/schema/preprocess identity, foot observables capability |
| **Edit** | `crates/bansayo-engine/src/pipeline/features.rs` | Honest no-feet/no-views/no-floor/degraded reasons |
| **Edit if necessary** | `crates/bansayo-engine/src/training/observables.rs` | Wire measured support-foot yaw/heel height into existing catalogue (verify actual path on main first) |
| **Edit** | `crates/bansayo-engine/src/sources/camera/capture.rs` | New optional metadata for variable K fixture; legacy 17 unchanged |
| **New, conditional GO** | `crates/bansayo-engine/src/sources/camera/pose_models.rs` | Typed candidate id/path/schema/hash/model metadata and file validation |
| **New, conditional GO** | `crates/bansayo-engine/src/sources/camera/pose_simcc.rs` | Variable-K RTMPose decoder with input/output tensor shape checks |
| **New, conditional GO** | `crates/bansayo-engine/src/sources/camera/pose_mediapipe.rs` | **Only** if approved export/runtime passes; otherwise do not create speculative adapter |
| **Edit, conditional GO** | `crates/bansayo-engine/src/sources/camera/pose.rs` | Delegate compatible backend; keep Detection/Track and legacy defaults |
| **Edit, conditional GO** | `crates/bansayo-engine/src/sources/camera/mod.rs` | Correct 26/33 descriptor and no-detection vector, keep single inference thread |
| **Edit, conditional GO** | `crates/bansayo-engine/src/config/schema.rs` | Typed `pose_model` optional, default COCO17 |
| **Edit, conditional GO** | `crates/bansayo-engine/src/config/validate.rs` | Model files/sha/output/permission/config validation, no unknown paths |
| **Edit, conditional GO** | `scripts/setup.py` | Optional pinned approved weights URL+SHA (never default auto-download) |
| **Edit, conditional GO** | `models/README.md` | Exact validated optional model provenance without binaries |
| **New** | `crates/bansayo-engine/tests/foot_schema_map.rs` | Synthetic indices, left/right, nulls, privacy/shape and no-feet regression |
| **New** | `crates/bansayo-engine/tests/foot_observables.rs` | Synthetic calibrated camera toe/heel positions, yaw/ground/contact provenance |
| **New, conditional GO** | `crates/bansayo-engine/tests/foot_pose_inference.rs` | Ignored real-weight image/inference/latency tests; CI uses fixtures |
| **Edit** | `crates/bansayo-engine/tests/camera_source.rs` | Raw source variable-K/legacy recorded schema, reconnect and multi-camera |
| **Edit** | `crates/bansayo-engine/tests/phase2_camera.rs` | New native-schema triangulation fixtures, same-camera gating |
| **Edit** | `crates/bansayo-contracts/tests/contracts.rs` | Explicit schema/pose-v1 backward compatibility (no fields added) |
| **Edit** | `docs/architecture/martial-arts/adr/ADR-021-pose-estimation-integration.md` | Append measured outcome, do not rewrite history of rejected Halpe checkpoint |
| **Edit** | `docs/13-implementation-status.md` | Real status, conditional integration vs blocked |
| **Edit** | `.github/workflows/ci.yml` | Offline fixture tests only; no unlicensed model binary download |
| **New after actual measurement** | `docs/validation/YYYY-MM-DD-phase06-foot-pose-report.md` | Exact hardware/dataset/metrics, PASS/FAIL/BLOCKED |

**Leave unchanged:** `crates/bansayo-contracts/src/pose.rs`, `crates/bansayo-contracts/src/motion.rs` canonical order, `triangulate.rs` mathematics, calibrated camera intrinsics/extrinsics, 19-segment skeleton, XR/IMU fusion/IK, Godot animation retargeting and martial arts training scoring definitions unless a specific test proves a narrow bug.

## 11. Deterministic test matrix (must execute, not just describe)

### 11.1 Schema and raw observation

- [ ] `coco17_native_schema_and_golden_unchanged`.
- [ ] `mediapipe33_names_count_33_and_left_heel29_right_heel30`.
- [ ] `mediapipe33_left_foot31_right_foot32`.
- [ ] `halpe26_left_toes20_22_right_toes21_23`.
- [ ] `halpe26_left_heel24_right_heel25`.
- [ ] `wholebody133_left_17_18_19_right_20_21_22`.
- [ ] `halpe26_indexes_17_18_19_are_not_feet`.
- [ ] `unknown_schema_returns_none_without_guess_by_point_count`.
- [ ] `native_to_canonical_26_slots_exact_names_and_missing_points`.
- [ ] `schema_midpoint_requires_both_points_and_min_confidence`.
- [ ] `schema_left_right_mirror_property_no_swapped_foot`.
- [ ] `native_landmarks_jsonl_legacy_17_loads_unchanged`.
- [ ] `native_landmarks_jsonl_33_requires_explicit_meta`.
- [ ] `native_landmarks_meta_rejects_wrong_model_sha_and_count`.
- [ ] `native_observation_raw_points_not_modified_by_downstream_mapping`.
- [ ] `descriptor_landmark_schema_matches_every_recorded_frame`.
- [ ] `no_detection_produces_K_nulls_not_17_stale_values`.

### 11.2 Metric triangulation and foot math

- [ ] `two_valid_calibrated_cameras_known_toe_heel_3d_recovered` with finite epsilon.
- [ ] `one_camera_only_never_claims_world_metric_foot`.
- [ ] `model_relative_33_world_landmarks_never_claim_frame_world`.
- [ ] `triangulation_mixed_17_33_schema_refused`.
- [ ] `stale_room_calibration_or_camera_moved_no_foot_scores`.
- [ ] `foot_yaw_identity_along_plus_z_is_zero`.
- [ ] `foot_yaw_right_90deg_about_y_is_positive_pi_over_2`.
- [ ] `foot_yaw_minus_179_to_plus_179_unwraps_small_rotation`.
- [ ] `foot_yaw_left_and_right_independent`.
- [ ] `foot_yaw_near_vertical_toe_heel_vector_unavailable`.
- [ ] `foot_yaw_horizontal_separation_below_0p08m_unavailable`.
- [ ] `foot_yaw_bad_reprojection_views_confidence_rejected`.
- [ ] `foot_yaw_observation_timestamps_and_source_ids_preserved`.
- [ ] `foot_toe_midpoint_3d_has_derived_not_measured_source`.
- [ ] `toe_midpoint_nonoverlapping_camera_views_does_not_gain_false_views`.
- [ ] `foot_yaw_in_air_not_support_pivot_without_contact_evidence`.
- [ ] `foot_pivot_ground_contact_unknown_is_unavailable`.
- [ ] `foot_pivot_skew_or_gap_over_100ms_does_not_interpolate_measured`.
- [ ] `foot_height_world_y_and_floor_offset_no_double_subtract`.
- [ ] `coco17_detector_no_feet_reason_remains_and_scores_unavailable`.
- [ ] `added_feet_do_not_change_19_bone_topology_or_fk_results`.

### 11.3 Inference and identity (conditional GO)

- [ ] `pose_model_expected_sha_verified_before_load`.
- [ ] `pose_model_unknown_license_is_not_downloaded_by_setup`.
- [ ] `pose_model_rtpose_simcc_K_26_tensor_shapes_validated`.
- [ ] `pose_model_fake_K_17_as_26_rejected_before_decode`.
- [ ] `pose_model_mediapipe_task_is_not_passed_to_ort_session`.
- [ ] `pose_model_normalized_pixels_correct_after_rotation_crop_and_mirror`.
- [ ] `pose_model_image_folder_roundtrip_17_26_33`.
- [ ] `pose_model_no_replay_model_selection_from_current_yaml`.
- [ ] `pose_model_identity_hash_changes_if_weights_change_same_filename`.
- [ ] `pose_model_switch_forbidden_while_recording_or_training_active`.
- [ ] `pose_model_reconnect_discards_old_track_and_schema`.
- [ ] `pose_model_selected_not_found_fails_clearly_coco17_not_faked`.
- [ ] `pose_model_default_config_is_byte_equivalent_to_existing_B0`.
- [ ] `pose_model_multi_camera_overload_drops_frames_not_queues_forever`.

### 11.4 Benchmark truthfulness

- [ ] `benchmark_manifest_requires_consent_private_paths_and_checksums`.
- [ ] `benchmark_rejects_mixed_resolution_without_pixel_transform`.
- [ ] `benchmark_rejects_gaps_unknown_reference_and_out_of_order_frame`.
- [ ] `benchmark_heldout_split_by_subject_no_leakage`.
- [ ] `benchmark_yaw_wrap_179_to_minus179_is_2degrees`.
- [ ] `benchmark_no_baseline_heel_values_yields_NA_not_infinite_improvement`.
- [ ] `benchmark_common17_comparison_same_frames`.
- [ ] `benchmark_bootstrap_1000_seed_20261120_deterministic`.
- [ ] `benchmark_candidate_missing_weights_is_BLOCKED_not_zero_error`.
- [ ] `benchmark_performance_reports_tracked_and_redetect_separately`.
- [ ] `benchmark_failure_cannot_mark_candidate_pass_via_missing_frames`.
- [ ] `benchmark_license_fail_always_overrides_good_accuracy`.

### 11.5 Commands and CI

Use the **actual** current root-level project toolchain. Required when appropriate files exist:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
BANSAYO_BLESS=1 cargo test -p bansayo-contracts
cargo test -p bansayo-contracts
cargo test -p bansayo-engine --test foot_schema_map
cargo test -p bansayo-engine --test foot_observables
cargo test -p bansayo-engine --test camera_source
cargo test -p bansayo-engine --test phase2_camera
cargo test --workspace
node scripts/verify-gate1.mjs
node scripts/verify-gate2.mjs
python -m unittest discover -s benchmarks/foot_pose/tests
python benchmarks/foot_pose/validate_manifest.py --manifest benchmarks/foot_pose/manifest.example.yaml --check-shape-only
```

For Tier B, use an **explicitly separate** command with private corpus and local approved model(s), for example:

```bash
# Replace placeholder with real private manifest; never put footage in git.
python benchmarks/foot_pose/score.py \
  --manifest /path/to/private/manifest.yaml \
  --config benchmarks/foot_pose/evaluation.yaml \
  --predictions /path/to/private/model_outputs/ \
  --out /path/to/private/phase06_evaluation.json
```

Actual CLI must implement these documented flags. Do not ship a script whose flags differ from docs. A missing test binary, model asset, GPU, camera, reference-label set or licensed checkpoint is **BLOCKED**, not PASS. After BANSAYO_BLESS run contracts again without it and verify no generated files changed.

## 12. Physical multi-camera validation matrix

Write actual results under `docs/validation/YYYY-MM-DD-phase06-foot-pose-report.md`.

1. Record current main SHA, OS, GPU, CPU, ONNX Runtime version/provider, per-model SHA, permission ledger and target model artifacts.
2. Verify physically calibrated ≥2 current cameras, identity confirmed, same active room, floor-board up sign and camera intrinsics/extrinsics fingerprint.
3. Acquire frozen 24-clip corpus (§4) with documented frame-time/exposure limits and valid independent foot marks/reference.
4. Run B0, then every **licensed** candidate on identical frame/crop sets; preserve 2D native output and detector confidence convention.
5. Score 2D visible heel/toe coverage per model, side, movement speed, camera view angle and occlusion class.
6. Triangulate heel and toe using **Bansayo's actual DLT** and record world m, used_views, reprojection, sigma and detection dropout.
7. Score held-out foot yaw against independent floor/foot reference. If only 2D labels, report **3D/yaw not validated**, no production GO.
8. Evaluate support-foot pivot separately from lifted-heel condition; test left/right and heel-lift event rules.
9. Compare shared body17 shoulder/elbow/hip/knee/ankle to B0, requiring no >5% error degradation.
10. Measure one real camera idle/redetect/tracked, then two and four real simultaneous feeds for at least **10 minutes**, with true capture FPS, inference throughput, frame-age p50/p95, skipped frames and GPU/CPU memory.
11. Test faster punches/kicks where blur and 30 FPS aliasing matter; do not attribute every error to model architecture.
12. Switch from candidate back to B0, reconnect cameras, restart engine and replay old sessions; raw observations must keep their recorded detector schema.
13. Confirm Godot avatar stays unchanged (19 bones), and only supported analytics/diagnostics gain measured foot evidence.
14. Run default COCO17 camera/IMU/VR/clinic smoke suites and ensure the existing "detector_no_feet" reason persists when candidate disabled.
15. Report actual GO/NO-GO/BLOCKED decision per checkpoint, with threshold failures visible. If no candidate meets gates, leave **B0 as default**.
16. Record independent source evidence: model license, evaluation corpus identity, participant holdout split, QA screenshots, durations and command outputs. No personally identifying images committed to GitHub.

**Known limitation:** The camera source currently relies on arrival timestamps when true camera exposure time isn't available. This uncertainty is particularly important for fast heel/toe motion; report it explicitly. A lower neural landmark error cannot fix motion blur or temporal offset automatically.

## 13. Seven exact implementation work packages / direct-to-main commits

These packages are **conditional**. Baseline preparatory work can be committed when it passes. Do not publish a runnable optional detector backend until rights/performance gates have passed.

### Commit 01 — Research/license ledger and benchmark contract

- [ ] Fetch latest main and verify actual files/methods have not changed since b49e5038.
- [ ] Record B0 model+weight SHA and existing risk R8.
- [ ] Add detailed rights ledger for M33, H26 and optional W133; explicitly mark unverified weights as BLOCKED_LICENSE.
- [ ] Add manifest+evaluation YAML and fixture validation.
- [ ] Commit: `docs,bench: define licensed foot-pose evaluation gates and reproducible corpus`.
- **Gate:** No new third-party weight downloaded without primary rights; no fake existing video files.

### Commit 02 — Pure canonical schema tables and synthetic fixtures

- [ ] Add `pipeline/pose_schema.rs` with exact 17/26/33/133 tables.
- [ ] Wire `schema_map.rs` for 2D and extend source-type tests; preserve B0.
- [ ] Add fixed synthetic left/right foot points and test all indices, known paired midpoint semantics and unknown schemas.
- [ ] Regenerate contracts only if a real contract change required (none expected).
- [ ] Commit: `engine: map candidate foot landmark schemas without changing COCO17`.
- **Gate:** B0 serialized events and golden outputs unchanged.

### Commit 03 — Triangulated world foot observables

- [ ] Add shared 3D canonical mapper and `foot_observables.rs` pure yaw/quality functions.
- [ ] Map native foot points into `MotionSample.BodyState.keypoints[22..25]` with independent measurement quality.
- [ ] Implement suppression for bad views, model-relative frames, floor unknown, out-of-plane degeneracy and unreliable pivot.
- [ ] Add synthetic camera calibration world point tests and model-relative rejection.
- [ ] Commit: `engine: derive measured-world foot yaw and support pivot evidence`.
- **Gate:** No new foot yaw without current metric heel+toe triangulation; no change to 19-bone rig.

### Commit 04 — Offline reference and scoring harness

- [ ] Implement offline dataset validation, fixed annotation IO, scoring, clip/subject bootstrap and timing reports.
- [ ] Add optional **offline-only M33 adapter** only if its model/runtime license is verified; otherwise keep candidate as BLOCKED_LICENSE and score synthetic fixtures only.
- [ ] Compare B0/common-17 and candidate feet using same images, no prediction smoothing or cherry-picking.
- [ ] Generate candidate decision file with exact thresholds, missing reference marked BLOCKED.
- [ ] Commit: `bench: evaluate heel toe and foot yaw accuracy against frozen heldout clips`.
- **Gate:** Real reported numbers have actual labeled dataset/model hashes; no model weights in git.

### Commit 05 — Optional native inference backend (only after PASS_EVALUATION)

- [ ] If H26 meets license+accuracy+latency, add `pose_models.rs`/`pose_simcc.rs` and typed YAML with validated tensor K=26.
- [ ] Else, if M33 meets all gates **and has a licensed supported production runtime**, integrate that adapter separately without faking ORT compatibility.
- [ ] Else, do not add runtime code; record **NO-GO** and skip this optional commit rather than create a dead implementation.
- [ ] Keep default B0; correct descriptor schema/K and raw recording manifest; no schema mixing across cameras.
- [ ] Commit only if qualified: `engine: optionally ingest licensed foot-capable pose model`.
- **Gate:** Actual approved model loads, decodes and processes without changing B0 run paths.

### Commit 06 — Full integration and scenario regression (only if GO; otherwise tests/report)

- [ ] Validate 1/2/4-camera real throughput and conservative metric sourcing, delayed/lost/reconnected camera behavior.
- [ ] Check replay deterministic with variable K and historical 17 metadata absence.
- [ ] Extend diagnostics with selected model ID, schema, latency, foot availability/suppression reason.
- [ ] Validate no workout/score semantics were changed without specific test.
- [ ] Commit: `engine,ui: preserve pose comparability and foot-quality diagnostics`.
- **Gate:** All default/golden/regression tests green; candidate integration still feature-off by default.

### Commit 07 — CI, documentation and physical verdict

- [ ] Run all §11.5 commands, collect exact PASS/FAIL/BLOCKED, verify no generated changes.
- [ ] Update ADR-021 append-only decision with measured model status, license evidence and quality/latency.
- [ ] Update `models/README.md` only for approved artifacts. Document model rights and no redistribution.
- [ ] Update implementation status and add CI fixture-only tests.
- [ ] Add `docs/validation/YYYY-MM-DD-phase06-foot-pose-report.md` with real reference numbers or explicit BLOCKED reasons.
- [ ] Check main for new commits before writing and do not overwrite concurrent features.
- [ ] Commit: `docs,ci: record Phase 06 foot model GO NO-GO and validation evidence`.
- **Gate:** Unlicensed/unsuccessful model remains disabled and B0 is intact; successful optional candidate has reproducible approval and regression evidence.

## 14. Required implementing-AI completion report

Return:

1. Latest main before/after SHA, actual prerequisite implementation status, and exact source map differences from this pinned plan.
2. Each direct-to-main commit with URL. **Do not invent all seven** when GO-gated packages were skipped; report skipped packages by gate.
3. B0 and every candidate: specific model ID/SHA, permission verdict, runtime and preprocessing, Native K/schema, inference available yes/no.
4. License ledger with distinct code/weights/dataset conclusions and links to primary evidence.
5. Benchmark corpus manifest checksum, camera calibration fingerprint, participant split, movement coverage and image timestamps.
6. Foot index mapping tests for M33/H26/W133, proper orientation/left-right handling, 2D world conversion boundaries.
7. Actual per-class 2D coverage, 2D normalized error, world 3D metric heel/toe error, yaw/pivot error, intervals and reasons missing.
8. Real 1/2/4 camera tracked/redetect p50/p95 inference, each camera delivered rate, frame-age and dropout. Include 60 FPS only if measured.
9. Proof B0 common17 landmark and camera/IMU/VR outputs are unchanged; historical 17 JSONL and MCAP replay preserved.
10. Actual selected candidate or **COCO17 retained** with concrete failed gate(s), not unspecified recommendation.
11. No binaries or private video/identifiers in GitHub; exact `git diff`/CI results.
12. TODOs with owner, file, accurate blocker and measured acceptance condition.

## 15. Phase 06 completion criterion and follow-on work

**Phase 06 is considered technically documented** with:
- strict candidate model/license research,
- a reproducible benchmark harness and synthetic schema/foot geometry tests,
- an auditable PASS/FAIL/BLOCKED report.

**Phase 06 is considered successfully integrated** only if an actual licensed model passes frozen independent heel/toe accuracy, non-foot regression, multi-camera throughput, schema and replay gates and is enabled as an optional detector without changing the COCO17 default.

If none qualifies, explicitly **NO-GO**. This is a valid engineering conclusion, not a prompt to invent numbers or silently weaken thresholds.

**Phase 07:** End-to-end hardware testing with 2–4 synchronized cameras, OpenXR HMD/controller, SlimeVR, precise stage/camera/floor alignment, latency and fast martial-arts motion. Phase 06 does **not** claim hardware tracking success because a 33-point pose model was described or a code schema was added.

**Explicitly defer:** cross-model 3D triangulation, detector ensemble/voting, YOLO tracking framework replacement, foot-ground physics correction, custom model retraining, hands/face animation, object interactions, learned 3D lifting and new per-frame Python backend. Use Rust in production; Python may exist only for offline evaluation or model conversion if approved.
