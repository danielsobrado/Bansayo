# Bansayo Phase 05 — Godot humanoid avatar retargeting and VR presentation

**Status:** Implementation specification only. No Phase 05 application code is included in this document commit.  
**Date:** 2026-11-20.  
**Target:** [danielsobrado/Bansayo](https://github.com/danielsobrado/Bansayo), direct commits to `main`, no force-push.  
**Verified Bansayo source baseline:** [5b361b54b360870ed5adb0c08d530aaa7fb38daf](https://github.com/danielsobrado/Bansayo/commit/5b361b54b360870ed5adb0c08d530aaa7fb38daf).  
**Reference baseline:** [maceq687/FullBodyPoseEstimation@be895709](https://github.com/maceq687/FullBodyPoseEstimation/commit/be895709c0bf86d35f64eb2cd5d2ffb4c6838593).  
**Prerequisites:** [Phase 01 — XR observation ingress](phase-01-openxr-observation-pipeline-implementation.md), [Phase 02 — world alignment and clock mapping](phase-02-openxr-room-alignment-and-time-synchronization-implementation.md), [Phase 03 — body proportions](phase-03-body-proportions-t-pose-and-controller-wrist-calibration-implementation.md), [Phase 04 — camera/XR/IMU constrained fusion](phase-04-confidence-aware-xr-camera-imu-fusion-and-elbow-ik-implementation.md).  
**Next:** Phase 06 — benchmark additional foot/heel/toe landmark models; Phase 07 — hardware validation.  
**Implementation ownership:** All tracking, calibrated lengths, IK, source selection, observation times and provenance stay in Rust; Godot does **rendering, skin deformation, bone retargeting and UX only**.  
**License rule:** Reference Unity project is GPL-3.0. Cite architecture, not source code; no copying/translating C#, Unity assets, prefabs, bundled commercial assets or shaders.

> **Implementation prerequisite, not a suggestion:** At the verified target commit, Phases 01–04 are Markdown plans, not implemented XR/body-fusion code. Before writing Phase 05 application code, inspect fresh `main`, verify each prerequisite has actually been implemented and tested, and refresh the code map. A procedural humanoid can be developed against the current synthetic camera/IMU skeleton fixture without XR hardware, but **never** mark full sensor-to-VR validation complete without the previous phases and physical headset tests. No speculative API calls or nonexistent paths may be treated as current code.

## 0. Purpose and exact boundary

Replace the disconnected capsule/mannequin appearance **as the default experience for a fresh installation** with a physically coherent, **GPU-skinned humanoid** driven by Bansayo's canonical 19-segment `SkeletonDefinition` and `SkeletonPose`, while retaining the current technical visualization, movement diagnostics, clinic, mirror and ghost.

Deliver one self-contained Godot 4.4+ implementation that runs in desktop fallback **without importing a copyrighted humanoid**. The first shipped default avatar is a deliberately stylized, anatomically tapered procedural humanoid, not a claim of photorealism. A commercial/realistic imported glTF body is a **future independent asset track** requiring licensing, rest-pose/bone mapping and QA, not a blocker for the Phase 05 functional milestone.

**Required outcomes:**

1. One Godot `Skeleton3D` with **exactly 19 bones**, one per canonical engine segment, parents in the same order.
2. One `MeshInstance3D` containing bounded procedural humanoid surfaces and a valid `Skin`, GPU-deformed by those 19 bones. Body scales from the current skeleton definition, **including measured Phase 03 arm lengths** when present.
3. One deterministic `pose_world_to_bone_local` transform pipeline; no per-frame Unity-like avatar bone guessing and no Godot-side elbow, wrist or knee IK.
4. **Correct bone positions** when pelvis translates, spine yaws, shoulders rotate and left/right elbows or knees bend. The whole body stays together under one forward-kinematic bone tree.
5. Revision change immediately rebuilds rest geometry/bind poses, clears incompatible buffered poses and prevents old-revision crossfade. No wrong-proportion frame leaks.
6. Distinct visual evidence for measured, derived, inferred and unavailable segments, not a clean mesh that hides tracking uncertainty.
7. Live third-person avatar, clinic reference ghost and desktop preview can use the new renderer without separate tracking/retargeting implementations.
8. Measured XR room alignment, if available from Phase 02, applies at **BodyRoot exactly once**. The native headset camera is completely independent.
9. No first-person body, passthrough/occlusion requirement or animation-graph dependency; these remain safety/comfort follow-ups.
10. All old contract/smoke tests pass, and new headless bone/mesh tests plus physical validation are explicitly reported.

**Minimum user-visible result:** in desktop simulation, a skinned humanoid follows standing, squat and front/side kick poses rather than separate rotating capsules. In VR, the same avatar appears in the correctly aligned room, and a ghost demonstrates the recorded movement using the same rig.

**Hard non-goals:** body pose estimation, optical detector replacement, original skin/photorealistic asset sourcing, face expressions, hair, fingers, cloth physics, mechanical joint corrections, foot locking or contact-solving in Godot, OpenXR driver changes, proprietary Unity assets, first-person embodiment and new networking.

## 1. Cross-repository code reference matrix — verified, immutable links

### 1.1 Upstream FullBodyPoseEstimation references

| Upstream reference | What exists | Specific independent Bansayo adaptation |
|---|---|---|
| [VNectModel.cs L73–120](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L73-L120) | JointPoint has a Transform, parent/child references and initial rotations; avatar Animator holds human bones | Godot `Skeleton3D` with explicit parent-index mapping and bind/rest poses, **not** Unity Transform mutation. |
| [VNectModel.cs L238–299](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L238-L299) | Maps pose joints to `HumanBodyBones` on Unity Animator and measures model dimensions | Data-driven Bansayo 19-segment-to-19-bone mapping; no hidden hand/finger joints or implicit Animator retargeting. |
| [VNectModel.cs L300–340](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L300-L340) | Initializes parent/child skeleton links and bone transforms | Build Godot hierarchy exactly from `SkeletonDefinition.segments`, verify all parents precede children and are unique. |
| [VNectModel.cs L423–505](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L423-L505) | Updates Unity avatar bone rotations and child transforms | Convert engine **world** quaternions to Godot **parent-local** bone rotations; use `set_bone_pose_rotation`, not per-bone world Node3D transforms. |
| [VNectModel.cs L188–214](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/VNectModel.cs#L188-L214) | Fixed controller Euler hand rotations and phantom elbow heuristic | **Do not port.** Phase 04 already solves arm constraints in Rust; Godot only displays resultant rotations. |
| [PoseVisualizer3D.cs L51–123](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/PoseVisualizer3D.cs#L51-L123) | Maps visual body landmarks and derived joint positions | Mirror and debug overlays can keep source-provenance markers, but no re-derivation of engine joints. |
| [BodySourceView.cs L33–64, L167–219](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/Assets/Scripts/BodySourceView.cs#L33-L219) | Kinect bone-map and line-rendered debug skeleton | Preserve Bansayo's existing technical skeleton as fallback/debug, not another data stream. |
| [FullBodyPoseEstimation LICENSE](https://github.com/maceq687/FullBodyPoseEstimation/blob/be895709c0bf86d35f64eb2cd5d2ffb4c6838593/LICENSE) | GPL-3.0 | Independent original Godot procedural mesh and scripts only. |

This project **does not** already contain Bansayo-compatible Godot skinned assets; no Unity prefab is authorized to be imported.

### 1.2 Current Bansayo code references

All links are pinned to `5b361b54`. This is the **pre-implementation** baseline, not future file availability.

| Actual target source | Observed implementation | Required Phase 05 change |
|---|---|---|
| [apps/vr/scripts/avatar_renderer.gd L1–55](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/avatar_renderer.gd#L1-L55) | One Node3D, procedural radial profiles, provenance materials, simple segment meshes; no Skeleton3D | Keep this class as **stable facade** and old technical/mannequin fallback; delegate new `humanoid` style to a SkinnedAvatar child. |
| [avatar_renderer.gd L64–105](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/avatar_renderer.gd#L64-L105) | `set_mannequin(bool)` toggles technical/mannequin; `build` creates independent Node3Ds, capsule/SurfaceTool meshes | Add `set_style(id)` without removing `set_mannequin`; only build GPU skinned rig when selected. |
| [avatar_renderer.gd L107–138](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/avatar_renderer.gd#L107-L138) | Current `_anatomical_mesh` uses radial profile, 16 sides × 25 rings, one mesh per segment | Reuse ideas/profiles for new original mesh builder; skin weights and inverse bind poses, not per-frame mesh mutation. |
| [avatar_renderer.gd L140–170](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/avatar_renderer.gd#L140-L170) | `apply_pose` accepts engine SegmentPose world transforms if `skeleton_rev` matches and colors by provenance | Preserve call signature; route matching pose to SkinnedAvatar. Reject invalid data/revisions without ghosting stale limbs. |
| [apps/vr/scripts/training_room.gd L33–87](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/training_room.gd#L33-L87) | Creates `BodyRoot`, `AvatarRenderer`, EngineClient subscriptions, clinic, mirror; skeleton_changed connects to live and clinic | Keep these hookups; no second engine client or skeleton stream. Clear pose buffer when skeleton changes. |
| [training_room.gd L95–140](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/training_room.gd#L95-L140) | Preferences index 3 toggles boolean mannequin; applies `set_mannequin` | Replace UI preference with 3 explicit styles, preserving old saved preferences. |
| [training_room.gd L270–281](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/training_room.gd#L270-L281) | `_process`: buffer.sample() → _body.apply_pose(), clinic status | Keep exactly one buffered render call; do not add frame interpolation in new skinned renderer. |
| [training_room.gd L244–268](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/training_room.gd#L244-L268) | Legacy manual XR alignment stores `user://xr_alignment.cfg` | Phase 02 will replace authority with engine alignment; do not reintroduce approximate local alignment. |
| [apps/vr/scripts/body_root.gd L1–5](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/body_root.gd#L1-L5) | Parent for engine content only, independent of native XR headset | Keep one `T_xr_stage_from_world` parent. No extra world transform in SkinnedAvatar. |
| [apps/vr/scripts/pose_buffer.gd L1–78](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/pose_buffer.gd#L1-L78) | 50ms presentation delay; 16 frame cap; 500ms stale timeout; world position lerp and xyzw slerp; version guard | Reuse interpolation as-is; add reset on skeleton_changed, style changes should not invalidate samples. |
| [apps/vr/scripts/engine_client.gd L1–110](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/engine_client.gd#L1-L110) | Protocol-v1 typed websocket messages, skeleton_changed, state/frame streams | No new messages or transport. Reuse same skeleton_changed and frame_received signals. |
| [apps/vr/scripts/comfort_settings.gd L1–38](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/comfort_settings.gd#L1-L38) | Existing `user://bansayo_vr.cfg`: `display/mannequin` bool; default true | Migrate to `display/avatar_style` enum string. Fresh default humanoid; old true maps mannequin, false technical, no data loss. |
| [apps/vr/scripts/status_panel.gd L1–180](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/status_panel.gd#L1-L180) | Compact setup and tracking/quality diagnostics | Add current renderer style and a short fallback reason, no giant options panel. |
| [apps/vr/scripts/clinic_ghost.gd L1–100](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/clinic_ghost.gd#L1-L100) | Owns another AvatarRenderer; clip provides positions/rotations and skeleton_rev; ghost-local root offset | Reuse same facade/SkinnedAvatar rig and ghost material; preserve origin relocation and clip sampling. |
| [apps/vr/scripts/clinic.gd L63–95](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/clinic.gd#L63-L95) | Ghost built from skeleton and displayed in clinic alongside targets | Select ghost style independently of live, preserve training controls/safety gates and height-dependent review. |
| [apps/vr/scripts/pose_mirror.gd L1–39, L91–134](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/scripts/pose_mirror.gd#L1-L134) | Camera-image 2D mirror with landmark overlays and confidence | Do not replace with unverified virtual mirror geometry; optional humanoid is a separate 3D figure. |
| [crates/bansayo-contracts/src/skeleton.rs L8–73](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/crates/bansayo-contracts/src/skeleton.rs#L8-L73) | `SkeletonDefinition` has model_id/rev/height/proportions, parents-before-children, offset/tail/contact points | Sole rig topology and rest geometry contract; no new JSON wire schema. |
| [crates/bansayo-contracts/src/pose.rs L7–42](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/crates/bansayo-contracts/src/pose.rs#L7-L42) | `SkeletonPose` has world-frame segment origin and `q_world_from_segment`, root position, rev, provenance | Must convert world orientations to parent-local `Skeleton3D` rotations; never treat world quaternion as bone-local. |
| [crates/bansayo-engine/src/skeleton/proportions.rs L22–87](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/crates/bansayo-engine/src/skeleton/proportions.rs#L22-L87) | 19 segment names, mirrored left/right; rest +X arms, -Y legs, T-pose facing +Z | Hard requirement for mapping; don't copy default anthropometric numbers into Godot. |
| [crates/bansayo-engine/src/skeleton/definition.rs L89–160](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/crates/bansayo-engine/src/skeleton/definition.rs#L89-L160) | Engine builds canonical segment lengths, can revise definition | Godot rig regenerates when definition/rev changes. No independent stature approximation. |
| [crates/bansayo-engine/src/skeleton/fk.rs L5–18](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/crates/bansayo-engine/src/skeleton/fk.rs#L5-L18) | Child positions are `parent_position + parent_world_rotation * offset_m` | Validate every Godot posed bone origin against the corresponding engine world pose. |
| [apps/vr/tests/contract_smoke.gd L1–81](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/tests/contract_smoke.gd#L1-L81) | Explicitly inspects AvatarRenderer `_segments`/`_meshes`, set_mannequin and visible-mesh behavior | Keep legacy tests passing; add new separate skinned-rig tests instead of deleting old assertions. |
| [apps/vr/tests/clinic_smoke.gd L1–144](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/tests/clinic_smoke.gd#L1-L144) | End-to-end simulator scene and clinic ghost review | Run unchanged; add assertions for new ghost style/revision. |
| [docs/architecture/martial-arts/25-mixed-reality-and-avatars.md L49–65](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/docs/architecture/martial-arts/25-mixed-reality-and-avatars.md#L49-L65) | Proposes bone map, provenance styling, no client IK, no first-person body; describes uniform imported-mesh scaling | Update to distinguish **built-in procedural skinned mesh follows current segment lengths** from future imported GLB rigs using uniform scale. Preserve no-client-IK rule. |
| [apps/vr/project.godot L1–25](https://github.com/danielsobrado/Bansayo/blob/5b361b54b360870ed5adb0c08d530aaa7fb38daf/apps/vr/project.godot#L1-L25) | Godot 4.4 feature tag, GL Compatibility renderer, OpenXR enabled | Implement/test APIs on Godot **4.4**; do not assume 4.6-only rendering defaults or Vulkan-specific effects. |

### 1.3 Official Godot implementation references

Use these official **4.4** specifications as API authority during implementation:

- [Skeleton3D](https://docs.godotengine.org/en/4.4/classes/class_skeleton3d.html): `add_bone`, `set_bone_parent`, `set_bone_rest`, `set_bone_pose_position`, `set_bone_pose_rotation`, `get_bone_global_pose_no_override`, `create_skin_from_rest_transforms`.
- [MeshInstance3D](https://docs.godotengine.org/en/4.4/classes/class_meshinstance3d.html): explicit skeleton NodePath, `skin`, per-surface material overrides.
- [SurfaceTool](https://docs.godotengine.org/en/4.4/classes/class_surfacetool.html): per-vertex `set_bones(PackedInt32Array)` and `set_weights(PackedFloat32Array)` **before** `add_vertex`.
- [Godot 4.4 scene skin/bind semantics](https://docs.godotengine.org/en/4.4/classes/class_resourceimporterscene.html): inverse bind poses and skin attachment.
- [Godot 4.4 animation retargeting guide](https://docs.godotengine.org/en/4.4/tutorials/assets_pipeline/retargeting_3d_skeletons.html): BoneMap/SkeletonProfileHumanoid for later imported assets. **Do not** introduce `AnimationTree` for real-time streamed pose.

**Important API detail:** `Skeleton3D` "global bone pose" is relative to the Skeleton3D node, not the actual scene/world global transform. Do not mix it with `Node3D.global_transform`. See official Skeleton3D docs.

## 2. Prescribed overall architecture

```text
Existing EngineClient
  -> skeleton_changed(definition) [canonical 19 parent-ordered segments; rev]
  -> frame_received(pose) -> existing PoseBuffer (50ms / 16 bounded)
                                     |
                              AvatarRenderer facade
                       +-------------+----------------+
                       |                              |
                 technical / mannequin           humanoid
                 existing Nodes / mesh          SkinnedAvatar
                 unchanged behavior                |
                                         Skeleton3D (19 bones)
                                         MeshInstance3D (GPU Skin)
                                         AvatarPoseAdapter
                                         AvatarSkinBuilder
                                         AvatarEvidenceMaterials
                       +------------------------------+
                       |
                   ClinicGhost same facade
                   (relative clip positions, same revision)
                       |
                Existing BodyRoot (T_xr_stage_from_world)
                       |
             Native XR camera remains independent
```

### Fixed decisions

- Native Godot 4.4 GDScript, no C# runtime, no GDExtension, no Rust renderer and no extra backend websocket.
- Default style **humanoid** only for a fresh user preference file; preserve old preferences and original rendering modes.
- Built-in avatar **procedurally generated**, deterministic, license self-contained, physically dimensioned using the received SkeletonDefinition.
- Exactly **19 segment-to-bone bindings**. No synthetic thumb/finger, face, chest-twist or toe bones.
- `Skeleton3D` rest pose in canonical T-pose, with **identity rest orientation** of all segment frames. Bone local rest origins from `offset_m` except pelvis root at zero.
- Skinning is GPU with **4 bone weights per vertex** (up to 2 used at a normal seam), not per-frame SurfaceTool regeneration.
- Per-bone pose **rotations and validated root position only**; don't set child global bone positions every frame.
- Root motion is applied once at a Node3D directly under AvatarRenderer, and XR stage alignment remains once at BodyRoot.
- All renderer variants use same `SkeletonPose` and `skeleton_rev`.
- Evidence styling mandatory in normal humanoid view; no opaque "everything tracked" appearance.
- All invalid/unknown topology, stale revisions and unavailable pose cases fall back to existing technical renderer or hide appropriately with user-visible diagnostics, never crash Godot.
- Do not add first-person head/camera coupling; headset pose should not be used to place an avatar head joint.

## 3. Canonical bone mapping — no name guessing

Use the segment ids **exactly** as already defined in the engine. For the built-in procedural avatar bone names equal segment IDs; the bone map file makes this explicit rather than attempting Unity HumanBodyBones conventions.

| Index | Bone name (= SegmentId) | Parent | Local rest offset |
|---:|---|---|---|
| 0 | pelvis | none | `Vector3.ZERO` under RigRoot |
| 1 | spine | pelvis | `spine.offset_m` |
| 2 | chest | spine | `chest.offset_m` |
| 3 | neck | chest | `neck.offset_m` |
| 4 | head | neck | `head.offset_m` |
| 5 | left_clavicle | chest | `left_clavicle.offset_m` |
| 6 | left_upper_arm | left_clavicle | `left_upper_arm.offset_m` |
| 7 | left_forearm | left_upper_arm | `left_forearm.offset_m` |
| 8 | left_hand | left_forearm | `left_hand.offset_m` |
| 9 | right_clavicle | chest | `right_clavicle.offset_m` |
| 10 | right_upper_arm | right_clavicle | `right_upper_arm.offset_m` |
| 11 | right_forearm | right_upper_arm | `right_forearm.offset_m` |
| 12 | right_hand | right_forearm | `right_hand.offset_m` |
| 13 | left_thigh | pelvis | `left_thigh.offset_m` |
| 14 | left_shin | left_thigh | `left_shin.offset_m` |
| 15 | left_foot | left_shin | `left_foot.offset_m` |
| 16 | right_thigh | pelvis | `right_thigh.offset_m` |
| 17 | right_shin | right_thigh | `right_shin.offset_m` |
| 18 | right_foot | right_shin | `right_foot.offset_m` |

**Validation:**
- Skeleton model ID exactly `bansayo.body.v1` at this phase; unsupported future models fall back gracefully with a reason, not a guessed bone map.
- SkeletonDefinition segments count==19; IDs unique and exactly expected; parent mapping as above, parent precedes child.
- `offset_m`/`tail_m` finite; arm and leg lengths nonzero; exception pelvis root rest offset (floor height) deliberately **not** applied within rig skeleton when RigRoot is at pelvis world position.
- Keep **canonical left/right sign**, T-pose facing **+Z**, arms along ±X, legs along -Y. Do not swap because it looks mirrored in a desktop viewport.
- Bone index order must be verified at runtime rather than relying on string insertion side effects.
- An invalid skeleton never tries to create an `Skin` or bake a mesh.

### 3.1 Data-only bone map and renderer config

**New** `apps/vr/config/avatar_humanoid.yaml` is a **documentation/build-time source**, not loaded by GDScript at runtime. Godot 4.4 does not ship a native YAML parser. To avoid duplicating config and breaking KISS, instead implement a native `Resource` **`apps/vr/config/avatar_humanoid.tres`** containing fixed required model ID, 19 bone names/parents and profile settings, with a small `avatar_config.gd` loader/validator. Keep application-wide camera/sensor configuration in engine YAML as in prior phases. Renderer-only defaults belong in `project.godot`/`AvatarConfig`, not Rust. **Do not check in a YAML file Godot cannot load.**

For optional future external mesh imports: the existing architectural `bansayo.avatar/v1` conceptual bone map should be kept as a versioned **data contract** in docs. Do not ship an importer or a second avatar asset registry in Phase 05.

## 4. Rest pose, bind pose and the exact pose transform

### 4.1 Build rest hierarchy

In `AvatarSkeletonBuilder.build(definition)`:

1. Validate complete definition and resolve ID→index once.
2. Create Skeleton3D, add one bone per canonical id, assign parent indices from validated parent names.
3. For the pelvis/root, use `T_parent_from_bone_rest = Transform3D.IDENTITY`; otherwise set `T_parent_from_bone_rest = Transform3D(Basis.IDENTITY, Vector3(offset_m))`. All rest bases identity because the engine's rest segment frames are world-aligned.
4. Compute `G_rest[bone]` recursively: root identity, child `G_rest[parent]*T_local_rest[bone]`. No default height term added twice.
5. Call `set_bone_rest(index,T_local_rest)`; initialize matching bone pose position/rotation to rest.
6. Construct `Skin` with a bind for every bone. Each bind references the same bone index/name used in mesh weights and stores **`inverse(G_rest[bone])`**.
7. Create one MeshInstance3D as sibling of Skeleton3D under the same rig-local Node3D; set its `skeleton` NodePath explicitly, attach the generated Skin and mesh.
8. At rest, skinning each vertex must yield **the same vertex coordinate** as submitted in rig-local bind space; test this before any live pose.

**Godot `Skin` implementation safety:** Confirm the exact 4.4 `Skin.add_bind(index,inverse_bind_pose)` signature and property names from official class docs before typing. If using `Skeleton3D.create_skin_from_rest_transforms()`, test it produces correct inverse bind transforms for every bone and use it **instead of** parallel manually computed binds. The prescribed choice here is **explicit per-bone Skin binds** to make invariants testable; do not create a duplicate Skin bound to different index order.

### 4.2 Engine world pose to local Godot bones — exact convention

Engine segment pose:
- `q_world_from_segment` = Quaternion xyzw, Hamilton;
- `p_world_segment` = proximal joint origin in **camera world metres**;
- `root_position_m` = pelvis world origin;
- `skeleton_rev` must exactly match built definition.

AvatarRenderer lives under BodyRoot, which already applies T_xr_stage_from_world only once. It owns `RigRoot` with **translation** `pose.root_position_m` and **identity rotation/scale**. The `Skeleton3D` and MeshInstance3D are children of RigRoot.

For bone i in topological parent-first order:

```text
q_target_world[i] = normalized(q_world_from_segment[i])

if i == pelvis:
    q_local[i] = q_target_world[i]
    p_local[i] = (0,0,0)
else:
    q_local[i] = inverse(q_target_world[parent]) * q_target_world[i]
    p_local[i] = definition.segments[i].offset_m

RigRoot.position = pose.root_position_m
RigRoot.rotation = identity
Skeleton3D.set_bone_pose_rotation(i, q_local[i])
Skeleton3D.set_bone_pose_position(i, p_local[i])
Skeleton3D.set_bone_pose_scale(i, (1,1,1))
```

Do **not** use `q_local = q_world_from_segment` for non-root, and do not multiply by a second guessed T-pose rest correction: all built-in rest bases are identity. On a rig with nonidentity imported rest rotations a separate data-driven correction would be needed — deferred to later asset work. Do not use `set_bone_global_pose_override` to hide mistakes in local-space math.

**Derived world transform (no extra head alignment):**

```text
G_pose_skeleton[i] = parent(G_pose_skeleton) * T_local_pose[i]

p_camera_world_bone[i] =
    RigRoot.position + G_pose_skeleton[i].origin

p_xr_stage_bone[i] =
    BodyRoot.T_xr_stage_from_world * p_camera_world_bone[i]
```

**Test:** every bone origin is within **0.002 m** of the matching engine `SegmentPose.position_m` for a consistent fixture generated by engine FK. The pelvis root itself must remain at the received camera-world root, not be raised by the `pelvis.offset_m` rest height. If an inconsistent pose violates this tolerance, reject the pose with a counted `avatar_fk_mismatch` reason; do not deform the rig by independent child position edits.

### 4.3 Quaternion and sign checks

- Normalize only after finite and norm-range validation; reject near-zero quaternion, NaN, Inf and malformed 4-element input.
- For smooth SLERP, use existing PoseBuffer quaternion; it already interpolates rotations between samples. Do not add another frame-delay/slerp stage in SkinnedAvatar.
- `q` and `-q` represent the same orientation; geometry comparisons should use rotation-angle equivalence rather than exact component equality.
- Test root yaw +90°, torso forward lean, shoulder raise, elbow flexion, knee squat, kick on each side, and replay time reversal/seek. No angle should be inverted by a left/right sign flip.

## 5. Mesh: original procedural skinned humanoid, not capsules

### 5.1 Geometry budget and aesthetic decisions

Use the current original `avatar_renderer.gd` radial profiles `PROFILES`/`VOLUMES` as **design references only** for an all-new skinned SurfaceTool mesh. Body is recognizably humanoid rather than a skeleton of separate constant-width capsules:

- Pelvis/spine/chest: broader asymmetric ellipsoid/tapered torso volume, smooth front/back profile.
- Neck/head: continuous neck transition with a simple rounded oval head; no facial detail or guessed eye tracking.
- Clavicles: subdued shoulder connectors, tapered into upper arms.
- Upper arm/forearm: tapered muscular section with joint overlap and deformation at elbows; preserve side symmetry by mirrored rest directions.
- Hands: simple mitt/palm shape with no fingers.
- Thigh/shin: anatomically tapered, with knee blending and broad calf; no hard knee snapping.
- Feet: forward-facing +Z wedge/rounded shoe shape; toe/heel model is static/assumed until Phase 06.

**Generator parameters (style defaults in AvatarConfig, not magic repeated literals):**

| Setting | Value | Meaning |
|---|---:|---|
| radial_sides | 12 | Per section circumference |
| axial_rings | 12 | Per section length |
| maximum_surfaces | 19 | Exactly one material surface per segment |
| skin_weight_slots | 4 | Godot GPU skinning layout |
| used_weights_per_vertex | <=2 | Adjacent parent/child only by default |
| proximal_blend_fraction | 0.20 | First fifth blends to parent |
| distal_blend_fraction | 0.20 | Last fifth blends to connected child |
| seam_overlap_fraction | 0.08 | Adjacent shells overlap slightly, no visible separated joint |
| default_roughness | 0.68 | Calm matte non-metallic finish |
| ghost_alpha | 0.25 | Ghost only |
| inferred_alpha | 0.38 | Evidence style |
| unavailable_alpha | 0.10 | Must look unavailable or be hidden |
| max_vertices | 5000 | Total generated mesh vertex cap |
| max_triangles | 8500 | Total generated triangle cap |
| max_build_ms (reference desktop) | 100 | Provisional rebuild target, not measured |
| max_pose_update_ms p95 | 0.30 | Provisional CPU bone update target |

19 surfaces × 12 circumference × 12 rings = **2736 main surface vertices**, excluding optional caps/connector additions; the cap of 5000 includes those additions. The 8500 triangle cap includes closed ends. Model can be CPU generated on rebuild, never per-frame.

### 5.2 Geometry generation details

**New `avatar_skin_builder.gd`**:

For each segment:
1. Read actual `tail_m` and compute `length=tail.length()`. Reject zero-length except a documented small connector where minimum visible length is used for **rendering only**, not for skeleton geometry.
2. In bind space, the bone's proximal position is `G_rest[i].origin`. Long axis is `dir=tail.normalized()`; construct deterministic orthonormal transverse axes `e1,e2` using world-facing +Z unless nearly parallel, then +X.
3. Generate `axial_rings=12` rings with `radial_sides=12` vertices each. Centre `c(t)=G_rest[i].origin + t*tail`. Radius profile from the current VOLUMES/PROFILES, scaled by **actual segment length** and a small separate anatomical width bound; never change the received bone length itself.
4. Taper profile `radius(t)` with a conservative smoothstep interpolation; avoid negative radii and collapsed rings. For torso/head use anisotropic X/Z radii to make visible chest depth and face-forward silhouette.
5. Emit positions, normals (or `generate_normals()`), UV and exactly four bone-index/weight slots per vertex using `SurfaceTool.set_bones` and `SurfaceTool.set_weights` before adding vertices. No more than two nonzero weights by default.
6. Middle rings: weight 1.0 to current bone. Proximal 20%: blend current and immediate parent (when non-root). Distal 20%: blend current and first child only when that child is an **unambiguous chain continuation** (upper_arm→forearm, forearm→hand, thigh→shin, shin→foot, spine→chest, neck→head). At branch points like chest→two clavicles and pelvis→two thighs, do not choose an arbitrary child; weights remain current.
7. Normalize every emitted weight vector (sum 1.0 ±1e-5), remaining weight entries 0. Skin indices are actual Skeleton3D indices, **not** hash order.
8. Winding is consistent with Godot 4.4 triangle orientation; supply UVs and generated normals; seal ends conservatively and overlap adjacent shells by up to 8% joint length. No negative indices, no NaN vertices.
9. Commit each segment's triangles as a separate mesh surface to permit per-segment material overrides. All surfaces share one ArrayMesh and one Skeleton3D.
10. Verify mesh vertex/triangle caps before committing a new avatar; on failure log and keep existing technical fallback, do not display half a mesh.

**No per-frame `SurfaceTool`, `ArrayMesh.add_surface_from_arrays`, creating new `StandardMaterial3D` per frame or reassigning skin.** A rescaled/recalibrated new `SkeletonDefinition.rev` triggers one rebuild and one cached material set.

### 5.3 Bind weights and joint seams

Provide explicit tests for:
- Upper-arm elbow seam: a point near upper-arm distal end has small forearm weight; a forearm proximal point has some upper-arm weight.
- No non-adjacent bone weights and no joint from left arm bound to right side.
- In rest pose, bone skinning identity retains original mesh surface vertex positions to <1e-4m.
- On 90° elbow bend, wrist/forearm surface follows forearm bone, elbow gap ≤10mm in the procedural silhouette benchmark (provisional), with no inside-out mesh.
- On 90° knee flexion and shoulder raise, no major self-inversion or flipped surface normals.
- The head/neck mesh must never be used to control user camera, and facial detail/eyes are fixed geometry.

**Visual limitation:** This is a bounded stylized skin model, not a high-detail AAA asset. The acceptance criterion is correct geometry and strong readable silhouette with stable deformation. True photorealistic anatomy requires a separately licensed/exported mesh, which this phase does not imply.

## 6. Evidence-aware materials without falsifying tracking state

The current renderer distinguishes four states with material colors. Preserve this information on the skinned surface, even when not showing debug capsules:

| Segment provenance | Visual rule | Interpretation |
|---|---|---|
| measured | Opaque cool off-white, clean surface | A directly measured input contributed to the segment orientation |
| derived | Opaque muted cyan accent | Fused/geometrically derived segment with qualified evidence |
| inferred | Violet translucent overlay / softened shell, alpha=0.38 | Tracking is inferred; orientation is not directly observed |
| unavailable | Hide surface or show sparse dark outline, alpha<=0.10 | Cannot responsibly display pose as known |

- One `MeshInstance3D` with up to 19 surfaces; use **per-surface override materials** cached by provenance state. Updating material selection when provenance changes is permitted; no shader/mesh regeneration.
- In the first version choose normal opaque + alpha StandardMaterial3D compatible with **GL Compatibility**. No Vulkan compute, custom skinning compute shader or Godot 4.6-only effects.
- Preserve original technical skeleton with its higher contrast segmentation, and add a **Debug bones** overlay option only if it can be enabled without duplicate position-solving.
- A hand/foot inferred from engine fallback must not be presented as a fully controlled hand/foot. Prefer a softly transparent mitten/shoe, optionally a small unobtrusive origin marker from the existing technical renderer.
- Optical camera measured joint markers and clinic targets use **engine measurement positions**, never visual skinned vertices as authoritative measurements.
- Material assignments are not a confidence score; do not map a missing `confidence` to 1.0.
- For the ghost, use the same mesh but a uniform translucent ghost tint, while preserving inferred/unavailable differentiation via alpha or outlines; no fake measured source on a recorded practice clip whose data is marked inferred.

## 7. Lifecycle, buffering and revision correctness

### 7.1 Style switching and migration

**Existing** `ComfortSettings.mannequin:bool` has saved key `display/mannequin` with old default true. Change to `avatar_style: String` with exactly these values:

```text
humanoid   = new procedural GPU-skinned default for fresh configs
mannequin  = existing segmented anatomical procedural mesh
technical  = existing thin capsule/bone debug renderer
```

Persistence:
- If saved `display/avatar_style` exists and valid, use it.
- Else if old key `display/mannequin` exists: true→mannequin, false→technical. The old preference must survive migration.
- Else (fresh preferences): choose humanoid. Do **not** delete the old file or old saved key automatically.
- On successful new save write `display/avatar_style`; maintain old key only if needed for older clients, but new code reads explicit style first.
- Expose a compact cycle from existing setup preference index 3, not more scrolling tabs. All other preference indices and menu actions remain stable.

`AvatarRenderer` retains:
```gdscript
func build(definition: Dictionary) -> void
func apply_pose(pose: Dictionary) -> void
func set_mannequin(enabled: bool) -> void
```

Add:
```gdscript
func set_style(style: String) -> void
func set_ghost_material(enabled: bool) -> void
func get_active_style() -> String
```

`set_mannequin(true)` remains a **legacy compatibility shim** selecting mannequin; false selects technical. Existing contract_smoke.gd stays valid. New style selection builds/skips rig only on explicit change/revision.

### 7.2 Definition and frame ordering

- `EngineClient.skeleton_changed` delivers a new SkeletonDefinition; validate and build first, then display matching `skeleton_rev` pose only.
- In `training_room.gd`, replace the direct skeleton_changed connection to `_body.build` with a small handler that **clears PoseBuffer, builds new renderer, notifies panel/clinic and only then unblocks matching incoming frames**. This is necessary because existing `PoseBuffer.interpolate` returns the newer pose on revision mismatch and may otherwise briefly show wrong topology.
- On `frame_received`, do not enqueue a pose whose `skeleton_rev` does not match current built definition. If frame arrives before definition, hold **at most one latest frame** for <=250ms or simply discard and wait for next matching frame; prescribe **discard** here for KISS. Surface a bounded diagnostic `skeleton_definition_pending`.
- `AvatarRenderer.apply_pose` must also independently reject pose whose rev mismatches built rig, or segment count differs from expected 19. Hide skinned rig until valid input.
- On new athlete proportions/body height/replay seek, `SkeletonDefinition.rev` changes (from engine). Recompute exact bone rest offsets/skin, discard prior references and cache; do not just change MeshInstance3D.scale.
- On a `ServerMessage::Frame` with null pose, clear active visual immediately. After >500ms no frames, buffer clears/hides. Do not keep a stuck translucent full-size avatar in the training area.
- `PoseBuffer` remains the **only** 50ms presentation interpolation stage; no additional lerp/slerp inside SkinnedAvatar.
- On camera/IMU tracking loss, respect pose provenance transitions, no renderer-side dead reckoning.
- On reconnect or engine hello, preserve saved style but clear buffered body and any stale in-flight definition.

### 7.3 Source-of-truth transform and XR safety

**Tree:**
```text
TrainingRoom
  BodyRoot                     # only T_xr_stage_from_world when verified
    AvatarRenderer             # stable existing facade
      SkinnedAvatar
        RigRoot                 # only pose.root_position_m in camera-world units
          Skeleton3D            # local rest+bone rotations
          MeshInstance3D        # same parent; explicit Skin
      [legacy technical/mannequin children, toggled]
  XROrigin3D                    # separate; native XR HMD+controllers
    XRCamera3D
  Clinic
    Ghost                       # on own display stage, separate rig
```

- Never parent `XRCamera3D` or XRController3D under body rig.
- Never use headset HMD pose to force avatar head rotation or eye-height because anatomical HMD→head calibration is out of scope.
- Phase 02 server-verified `T_xr_stage_from_world` is applied **once** at BodyRoot; do not apply it again to segment pose or SkinnedAvatar.
- On recenter, stale XR alignment immediately withdraws the world-aligned first-person overlay, as Phase 02 requires. Technical desktop preview can continue with identity parent; don't continue a falsely aligned VR body.
- No first-person body overlay by default. Keep avatar as an external trainer/mirror figure; physical strikes use existing play-area/clinic safety checks, not collision with skinned mesh.
- The skinned mesh is not an interaction collider or hit detection source. Existing engine keypoints/clinic target geometry remain authoritative.

## 8. Clinic ghost and mirror: one rendering implementation

### 8.1 Ghost integration

Current `clinic_ghost.gd` calls `_body.build(skeleton)` and `_body.apply_pose({...})` on recorded clip data. Its clip `positions_m` are already transformed to a **local ghost stage** by subtracting `_root`. Preserve this design:

1. Add `_body.set_style("humanoid")` for a ghost renderer by default (not a new clone of mesh-generation code).
2. `set_ghost_material(true)` changes materials only; root/rotation mapping remains identical.
3. Clip poses have `skeleton_rev`; reuse exact built definition, reject mismatch before playing.
4. The ghost's display-stage translation and path markers stay independent from live athlete root, BodyRoot alignment and the headset.
5. On paused/restarted/speed-changed clip, retarget exact sampled pose; no second internal interpolation stage. Clip may use its own indexing as currently implemented.
6. Current ghost clip sets all segment provenance `inferred`; preserve this fact or consume real recorded provenance if the clip wire actually carries it in the future. Do not relabel recorded motion as measured just because the mesh looks realistic.
7. Memory: ghost may share one read-only geometry cache with same skeleton revision if safe and tested. Its materials must be **distinct resources** from live avatar so alpha tint cannot leak between them.

### 8.2 Mirror and diagnostic overlays

The existing `pose_mirror.gd` is a **2D camera-image mirror** with landmarks, not a physical 3D mirror view. Do not replace/rename it or break live image overlay. The humanoid is shown in the existing room; a separate true virtual mirror surface is **deferred** until its physical alignment, camera depth and visual safety are tested. "Mirror" UX must still open the existing camera view.

If user switches technical/mannequin/humanoid during clinic:
- Live avatar changes immediately without changing exercise or reset counts.
- Ghost stays humanoid unless it has its own explicit style selection; no rebuild every frame.
- Labels and targets remain independent; no depth/clipping artifacts covering setup panels.

## 9. File-by-file implementation manifest

Only files that currently exist are marked **Edit**, new files marked **New**. Phase 02–04 planned modules are prerequisites but are not needed for building a synthetic avatar from already existing skeleton/pose wire data.

| Operation | Path | Precise responsibility |
|---|---|---|
| **New** | `apps/vr/scripts/avatar_definition.gd` | Validate model ID/rev/19 IDs/topological order/offsets/tails; build ID-index and rest transforms |
| **New** | `apps/vr/scripts/avatar_skeleton_builder.gd` | Create Skeleton3D, parent/rest transforms and explicit Skin inverse binds |
| **New** | `apps/vr/scripts/avatar_skin_builder.gd` | Procedural mesh surfaces, skin weights, consistent winding/normals, budget limits |
| **New** | `apps/vr/scripts/avatar_pose_adapter.gd` | Validate engine world pose and convert 19 world rotations to parent-local rotations and root position |
| **New** | `apps/vr/scripts/avatar_evidence_materials.gd` | Cached measured/derived/inferred/unavailable and ghost materials compatible with GL renderer |
| **New** | `apps/vr/scripts/skinned_avatar.gd` | Thin owner: Skeleton3D/MeshInstance3D, rebuild, apply validated pose, material updates |
| **Edit** | `apps/vr/scripts/avatar_renderer.gd` | Stable facade; retain old modes/tests, add humanoid style and ghost material API |
| **Edit** | `apps/vr/scripts/comfort_settings.gd` | Migrate mannequin bool → avatar_style, preserve existing saved choices |
| **Edit** | `apps/vr/scripts/training_room.gd` | Render style cycling, skeleton-rev buffer clear, single pose dispatch, no camera changes |
| **Edit** | `apps/vr/scripts/clinic_ghost.gd` | Use facade humanoid and independent ghost materials while retaining clip path/root |
| **Edit if needed** | `apps/vr/scripts/clinic.gd` | Pass ghost style choice without altering training safety or timer logic |
| **Edit if needed** | `apps/vr/scripts/status_panel.gd` | Small status/fallback reason and style label |
| **Edit** | `apps/vr/project.godot` | Renderer-only native settings/feature toggle; stay GL Compatibility |
| **New** | `apps/vr/tests/avatar_definition_smoke.gd` | Validate canonical hierarchy, root rest, invalid input rejection |
| **New** | `apps/vr/tests/avatar_skinning_smoke.gd` | Skin binds, weights, mesh counts, elbow/knee pose and no per-frame regeneration |
| **New** | `apps/vr/tests/avatar_retarget_smoke.gd` | World→local rotations, FK origin equality, yaw, IK fixture, revisions, persistence |
| **New** | `apps/vr/tests/avatar_ghost_smoke.gd` | Ghost uses same rig, own materials/root, clip revision and pause |
| **Edit** | `apps/vr/tests/contract_smoke.gd` | Keep all existing legacy assertions; add a minimal new facade style test only |
| **Edit** | `apps/vr/README.md` | User style settings, mesh source, FK retargeting, limitations, test commands |
| **Edit** | `docs/architecture/martial-arts/25-mixed-reality-and-avatars.md` | Clarify procedural measured-length vs future imported asset uniform-scaling policy |
| **Edit** | `docs/13-implementation-status.md` | Actual test evidence; not a claimed live validation based on synthetic tests |
| **Edit** | `.github/workflows/ci.yml` | Run Godot headless 4.4+ deterministic rig tests, preserve old tests |

**No production Rust contract or solver file changes required** for the core Phase 05 renderer. Only if a bug proves `SkeletonDefinition` or `SkeletonPose` inconsistent may the implementation AI fix a narrow contract/engine defect, with separate regression proof and explicit report. Do not introduce a new avatar-specific network or tracking schema.

## 10. Test matrix: exact scenarios and expected results

### 10.1 Definition/topology (headless no headset)

- [ ] `avatar_definition_exact_19_segment_names_and_parent_order` — only the canonical names, no duplicate/missing IDs.
- [ ] `avatar_definition_root_rest_translation_zero` — pelvis local rest origin zero; `pelvis.offset_m` not applied twice.
- [ ] `avatar_definition_child_rest_offsets_equal_wire_m` — each child rest.origin matches its definition.
- [ ] `avatar_definition_right_left_mirroring_is_correct` — +X and -X arms remain mirrored.
- [ ] `avatar_definition_rejects_invalid_parent_unknown_model_or_duplicate_id`.
- [ ] `avatar_definition_rejects_nan_zero_quaternion_and_negative_tail_length`.
- [ ] `avatar_definition_rejects_rev_or_segment_count_mismatch_without_crash`.
- [ ] `avatar_definition_handles_partial_measured_arm_lengths` — body arm rest offsets/tails use exact received values, not guessed uniform stretch.

### 10.2 Skin mesh/bind correctness

- [ ] `avatar_skin_has_19_bones_and_at_most_19_surfaces`.
- [ ] `avatar_skin_each_surface_has_four_index_weight_slots`.
- [ ] `avatar_skin_weights_sum_one_and_reference_valid_bones`.
- [ ] `avatar_skin_no_cross_side_or_nonadjacent_weights`.
- [ ] `avatar_skin_rest_vertices_unchanged_by_bind_matrices_within_0p1mm`.
- [ ] `avatar_skin_vertex_and_triangle_counts_within_budgets`.
- [ ] `avatar_skin_all_normals_finite_and_winding_consistent`.
- [ ] `avatar_skin_elbow_90_deg_no_mesh_inversion_or_gap_over_10mm`.
- [ ] `avatar_skin_knee_90_deg_no_flipped_normals`.
- [ ] `avatar_skin_head_foot_forward_facing_plus_z`.
- [ ] `avatar_skin_one_definition_rebuild_no_per_frame_arraymesh_mutation`.
- [ ] `avatar_skin_unknown_definition_falls_back_technical_with_reason`.

### 10.3 World-to-local retargeting (fixtures)

- [ ] `avatar_pose_identity_tpose_reproduces_19_world_origins`.
- [ ] `avatar_pose_pelvis_world_translation_exactly_once`.
- [ ] `avatar_pose_parent_rotation_compensated_by_child_local_inverse`.
- [ ] `avatar_pose_yaw90_no_swapped_axes_or_mirror`.
- [ ] `avatar_pose_left_and_right_elbow_90_degree_expected_bone_origins`.
- [ ] `avatar_pose_knees_bend_correctly_during_squat`.
- [ ] `avatar_pose_front_kick_support_leg_remains_on_world_floor_fixture`.
- [ ] `avatar_pose_constrained_arm_from_phase04_matches_engine_fk_2mm`.
- [ ] `avatar_pose_nonfinite_or_nonunit_wire_rotation_rejected`.
- [ ] `avatar_pose_missing_segment_hides_or_falls_back_not_stale`.
- [ ] `avatar_pose_buffer_interpolates_once_only`.
- [ ] `avatar_pose_skeleton_rev_change_clears_old_buffer_before_frame`.
- [ ] `avatar_pose_replay_seek_and_clock_rewind_do_not_mix_revision`.
- [ ] `avatar_pose_world_from_stage_alignment_not_applied_twice`.

### 10.4 Evidence, old renderer and clinic

- [ ] `avatar_style_fresh_install_defaults_humanoid`.
- [ ] `avatar_style_legacy_saved_true_maps_mannequin_false_maps_technical`.
- [ ] `avatar_style_all_three_modes_switch_without_restarting_tracking`.
- [ ] `avatar_style_legacy_set_mannequin_contract_smoke_unmodified`.
- [ ] `avatar_evidence_measured_derived_inferred_distinct`.
- [ ] `avatar_evidence_unavailable_does_not_show_opaque_limb`.
- [ ] `avatar_evidence_xr_inferred_elbow_not_mislabelled_measured`.
- [ ] `avatar_ghost_independent_material_does_not_tint_live_avatar`.
- [ ] `avatar_ghost_clip_version_mismatch_rejected_without_crash`.
- [ ] `avatar_ghost_play_pause_restart_speed_preserves_root_and_bones`.
- [ ] `avatar_pose_mirror_camera_image_and_landmark_overlay_unchanged`.
- [ ] `avatar_xr_camera_stays_independent_of_head_bone`.
- [ ] `avatar_tracking_loss_hides_and_recovers_without_nan_or_old_pose`.
- [ ] `avatar_technical_fallback_when_skin_build_fails_is_safe`.
- [ ] `avatar_existing_clinic_smoke_and_spatial_ui_smoke_pass`.

### 10.5 Commands

Run from repository root, on the actual pinned/verified toolchain. **Do not claim these were executed by this planning commit:**

```bash
cargo fmt --all -- --check
cargo test --workspace
node scripts/verify-gate1.mjs
node scripts/verify-gate2.mjs

godot --headless --path apps/vr --editor --quit
godot --headless --path apps/vr --xr-mode off --script tests/contract_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/spatial_ui_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/pose_mirror_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/avatar_definition_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/avatar_skinning_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/avatar_retarget_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/avatar_ghost_smoke.gd
```

Existing `clinic_smoke.gd` needs a running simulator engine; start the engine with an isolated data directory and `config/sim-full-body.yaml`, run the integration test, then shut it down. Follow exact checked-in test instructions and do not run it against a live production engine. If Godot is not installed, mark **BLOCKED**, not PASS; install/pin proper runtime in CI. Do not silently skip GL Compatibility tests.

## 11. Performance and frame-time acceptance

This phase does **not** add a second physics loop or any per-frame triangulation. Measure renderer CPU and GPU separately. These are **targets, not measured results**:

| Metric | Provisional target | Method |
|---|---|---|
| Live avatar pose apply (19 bones) | p95 <=0.30ms | Godot profiler, stationary and full kicks |
| Two simultaneous skinned avatars, CPU render-related work | p95 <=1.0ms added | Live + clinic ghost, same scene |
| Procedural avatar build after skeleton revision | <=100ms on reference desktop | Instrument new build only |
| Total vertices per avatar | <=5000 | Assert in headless generation test |
| Total triangles per avatar | <=8500 | Assert before mesh commit |
| Live pose render delay | Existing 50ms buffer unchanged | Compare PoseBuffer timing, not motion-to-photon |
| Headset rate | No persistent new missed-frame trend | Real headset runtime profiler |
| Build allocations | Only on style/skeleton revision | Count ArrayMesh/Skin resource creations |
| Dropout handling | No visible outdated pose >500ms | Disconnect/reconnect test |
| Root/origin correspondence | <=2mm on exact simulation | Compare Godot bone origins to engine fixture |
| Joint deformation | No major visible seam at standard bends | Review screenshots + video |

If performance fails, first profile material changes and mesh construction. Reduce radial_sides from 12 to 10 **only after measuring and updating config/test budgets**; do not prematurely add LOD streaming, worker threads or GPU compute. Keep one skinned mesh per avatar and reuse materials.

No fake frame-rate/latency numbers in docs. Headless geometry proves correctness, not physical headset comfort or visual AAA quality.

## 12. Physical headset and visual acceptance protocol

Write a verification report at `docs/validation/YYYY-MM-DD-phase05-humanoid-vr.md`.

1. Record exact engine SHA, Godot binary/version, headset, GPU, OpenXR runtime, controller device and actual sensor setup.
2. Verify Phase 01–04 implementation SHA exists and its real calibration profile status is valid; if absent, desktop can pass while **VR calibrated embodiment remains BLOCKED**.
3. Launch desktop simulated standing: confirm humanoid anatomy is clearly readable with torso, head, hands, feet and correctly sided limbs.
4. Run squat, front kick, roundhouse and punch/guard motions. Review both sides front/side/back; there must be no reversed joints, broken elbows/knees, foot orientation swap or pelvis popping.
5. Compare **19 computed Godot bone origins** to engine segment origins; record median/p95 error on simulated exact geometry. Fail if >2mm for consistent inputs.
6. Switch between fresh default humanoid, legacy mannequin and technical skeleton. The existing saved preference must restore correctly after restart.
7. Enable recorded clinic ghost beside live avatar. Exercise pause/speed/restart and verify ghost pelvis root offset, materials, clip revision and marker paths.
8. In VR, verify native headset camera continues tracking normally with no coupling to body head pose. Move the head independently while engine skeleton stationary; the avatar head must **not** move from headset input alone.
9. If calibrated Phase 02 XR alignment exists, place a physical floor mark and compare avatar foot root/trainer stage against the mark. Do not claim alignment accuracy from rig rendering.
10. Recenter XR: Phase 02 should remove any now-stale world overlay; humanoid should not remain convincingly aligned after the calibration becomes invalid.
11. Temporarily occlude camera wrist/elbow or disconnect IMU/XR controller. Confirm inferred/unavailable materials change and stale segments do not stay opaque.
12. Run live recording/replay, restart/seek and switch active athlete proportions. Validate seamless revision rebuild without one-frame mismatched limbs or duplicate body height.
13. Exercise clinical/training UI while moving arms. Ensure skinned mesh does not obstruct world-space buttons, trigger aiming or safety boundary cues.
14. Profile 10 minutes of motion on the target GPU/headset: p50/p95 CPU/GPU frame time, dropped XR frames, mesh creation count and memory trend. Report actual values, not expectations.
15. Capture screenshots: rest front/side, hands raised, both elbow bends, squat, side kick, inferred arm, ghost next to live, desktop style preference and XR-aligned view. Save to a documented validation folder **without** committing personal images inadvertently.
16. Report PASS/FAIL/BLOCKED per item, with target revision, screenshot names, test fixtures and exact reproduction steps.

**Safety/comfort:** never render a virtual body as an assured first-person physical limb in striking exercises. The avatar is an external figure; real-world collision risk, play-area boundaries and headset tracking loss remain governed by existing safety code.

## 13. Seven prescribed implementation commits to main

### Commit 01 — Validate topology and bone-map/rest poses

- [ ] Fetch latest main, record SHA and actual Phase 01–04 code status. If prerequisites missing, keep this phase in a **document/test-only preparatory branch** or block all coupled work; do not pretend sensor integration is available. Per repository instruction implementation commits go to main only when safe and passing.
- [ ] Implement `avatar_definition.gd` with model/19-parent/offset validation.
- [ ] Implement `avatar_skeleton_builder.gd` and explicit inverse bind Skin creation, no mesh yet.
- [ ] Add `avatar_definition_smoke.gd` fixtures from current `contracts/examples/skeleton.json`.
- [ ] Run Godot headless build; verify root rest origin zero and FK hierarchy.
- **Commit:** `vr: define validated canonical humanoid Skeleton3D rest rig`.
- **Gate:** 19 bones and correct parent transforms, no double pelvis offset.

### Commit 02 — Build original skinned humanoid mesh

- [ ] Implement `avatar_skin_builder.gd` with 19 tapered surfaces and bounded vertex budgets.
- [ ] Attach MeshInstance3D with explicit Skin, weight indices, normals, UVs.
- [ ] Add material cache and rest-skin vertex validation.
- [ ] Add `avatar_skinning_smoke.gd` elbow/knee geometry cases.
- [ ] Run Godot editor-import and headless tests; verify no external asset licensing concerns.
- **Commit:** `vr: generate GPU-skinned procedural humanoid with stable joint weights`.
- **Gate:** skin deforms once from bone transforms, no per-frame mesh rebuild.

### Commit 03 — World-to-local retargeting and revision handling

- [ ] Implement `avatar_pose_adapter.gd` exact §4 parent-local quaternion mapping.
- [ ] Implement `skinned_avatar.gd`, pose validation and root motion.
- [ ] Assert 19 bone-origin match on identity/yaw/bend/kick fixtures.
- [ ] Add skeleton-revision and malformed-input/reset tests.
- [ ] Keep BodyRoot alignment applied only once.
- **Commit:** `vr: retarget world skeleton poses onto parent-local humanoid bones`.
- **Gate:** <=2mm origin error on canonical FK fixtures; 0 invalid rotations.

### Commit 04 — Stable facade, preferences and provenance

- [ ] Integrate the new renderer through existing AvatarRenderer `build`/`apply_pose`.
- [ ] Preserve `set_mannequin` and existing contract smoke assertions.
- [ ] Implement three styles, migrate user settings, fresh default humanoid.
- [ ] Add cached per-segment provenance/ghost materials.
- [ ] Keep old technical view as safe fallback for invalid topology.
- **Commit:** `vr: make skinned humanoid default while retaining technical evidence modes`.
- **Gate:** old preferences and rendering smoke tests unchanged; no user-visible false tracking.

### Commit 05 — Integrate clinic ghost and pose-buffer revisions

- [ ] Keep existing EngineClient and one PoseBuffer; clear incompatible frames on skeleton update.
- [ ] Reuse same SkinnedAvatar facade for clinic ghost, own tint/cache.
- [ ] Preserve clip positions after ghost root offset and clip pause/speed/restart behavior.
- [ ] Keep camera-image pose mirror untouched.
- [ ] Add `avatar_ghost_smoke.gd` and old `clinic_smoke.gd` with simulator.
- **Commit:** `vr: share humanoid rig with clinic ghost and revision-safe playback`.
- **Gate:** live and ghost do not share mutable materials/pose state or alter training metrics.

### Commit 06 — UX, fallback and performance

- [ ] Integrate compact style selection and diagnostics with existing setup panel.
- [ ] Add malformed skeleton/pose fallback, zero-pose hide and recenter safety.
- [ ] Instrument build and pose update timing only at diagnostic level, no frame-by-frame log spam.
- [ ] Review GL Compatibility frame time and vertex/triangle caps with live+ghost.
- [ ] Capture deterministic before/after desktop screenshots for visual review.
- **Commit:** `vr: polish evidence-aware humanoid presentation and performance`.
- **Gate:** no UI overlap, no phantom tracked limbs, no regressions to XR controller input.

### Commit 07 — CI, documentation and hardware validation

- [ ] Update app README and architecture doc to distinguish procedural skinned avatar vs future imported mesh.
- [ ] Update implementation status with exact code commit links and test results.
- [ ] Add/pin Godot CI headless test matrix; run all §10.5 commands.
- [ ] Run real XR/headset validation where hardware available; otherwise mark individual steps **BLOCKED**.
- [ ] Inspect latest main again and check diff for newer concurrent functionality. Commit docs/results directly to main.
- **Commit:** `docs,ci: validate Phase 05 skinned VR avatar retargeting`.
- **Gate:** deterministic tests green, evidence archived, no claim of real hardware testing when not done.

## 14. Required implementation AI completion report

When implementing Phase 05, deliver:

1. Verified current main SHA, actual Phase 01–04 code prerequisite commits and before/after Phase 05 HEAD.
2. Seven direct-to-main commit URLs, plus changed/new file manifest with any deviations from pinned paths.
3. Exact Godot version and supported runtime APIs for Skin/Skeleton3D/MeshInstance3D.
4. Proof of 19 canonical IDs, parent order, bind pose and world-to-local math, including pelvis height not applied twice.
5. Mesh vertex/triangle count, surface list, skin weight normalization and inverse-bind correctness.
6. 19-bone FK residual table for identity, yaw, squat, elbow bend and kick fixtures.
7. Legacy mannequin/technical mode behavior and saved preference migration result.
8. Per-segment provenance visuals, unavailable pose behavior and source labels.
9. Live/ghost isolation, clip seek/pause/restart correctness, mirror/clinic regression.
10. Actual test command results with PASS/FAIL/BLOCKED; never claim unrun tests green.
11. Measured performance p50/p95, asset-build allocations and device/headset evidence where available.
12. Screenshots/videos in a validation directory with relevant timestamp/commit source.
13. Physical calibration and headset comfort PASS/FAIL/BLOCKED; do not label synthetic simulated pose as real VR validation.
14. Any outstanding work as **TODO** with owner, exact file path and acceptance gate.

## 15. Explicit next-phase boundary

**Phase 06:** Evaluate foot landmark improvements (COCO17 versus optional 33-point heels/toes) with offline speed/accuracy measurements and licensing audit. Do not import new YOLO models or alter pose detection in Phase 05.

**Phase 07:** End-to-end physical multi-camera and HMD/controller/SlimeVR validation at high-speed martial arts movement, including real timing, optical occlusion, drift, spatial alignment and reported physical error.

**Deferred:** importing licensed realistic humanoid GLBs with SkeletonProfileHumanoid/BoneMap; clothing/hair/face blendshapes, hand/finger pose, foot-ground constraints, realtime self-shadow/physically based skin shaders, passthrough and depth occlusion, first-person body, GPU-driven instancing and advanced LOD. These are not prerequisites for the self-contained skinned humanoid.

**Definition of done:** A single verified canonical 19-bone Godot Skeleton3D deforms an original bound humanoid mesh with exact engine-world-to-parent-local pose conversion; evidence provenance is preserved; the current live renderer, ghost, pose buffer, technical fallback, desktop and VR UI remain compatible; physical headset camera is independent; measured proportions are honored when present; and no tracking/IK rules are duplicated outside Rust. No downstream foot-accuracy or imported-asset claims are made.
