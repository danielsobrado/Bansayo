# 25 — Mixed reality, avatar retargeting and spatial calibration

Passthrough training beside fully immersive VR, body avatars, and the alignment of the camera
world with the headset's space. The VR architecture, experiences and safety model are in
[11](11-vr-architecture.md); this document adds what that one leaves open.

Status: **Proposed**, P1 design; nothing has run in a headset
([19 §3](19-risks-and-open-questions.md) O-10, O-14).

## 1. Capability card

| Item | Content |
|---|---|
| Use case and value | Kick and strike drills with the real room visible; a mirror and ghost standing on the real floor; a humanoid avatar instead of capsules. Value: fewer boundary pauses per session; athlete-reported comfort; no added latency on the user's own view |
| Existing support | Godot client with pose buffer and debug skeleton (runs in desktop mode); controller-based room alignment (commit `3ae126c`); `xr_alignment` calibration kind planned; engine-computed clips for any skeleton; safety model of [11 §7](11-vr-architecture.md) |
| Needed now | The avatar bone-map file of §4; the alignment-quality fields of §3. Both are data definitions |
| Dependencies | A headset and runtime; passthrough extensions per vendor; `multi_view` for a world skeleton |
| Feasibility and limits | A third-person avatar and mirror are feasible. A first-person body is rejected on latency. Passthrough availability, quality and depth occlusion differ per headset and runtime and must be verified, not assumed |
| Priority | P1 (mirror on the real floor, avatar); P2 (passthrough as default, hand interaction) |
| Smallest milestone | The mirror figure stands on the real floor within a few centimetres after alignment, checked against a physical floor mark seen through passthrough |
| Evidence before development | A headset session of the existing client; L-04 latency; the verification matrix of §6 for the owner's headset |

## 2. Presentation modes

| Mode | What the user sees | When |
|---|---|---|
| Immersive | Virtual dojo; runtime boundary | Forms review, mobility, slow practice |
| Passthrough (mixed reality) | The real room; virtual mirror, ghost, targets and HUD anchored to the real floor | Kicking and striking drills, where seeing walls and furniture matters most |
| Desktop mirror | No headset; the same scene on a monitor | Development; athletes without a headset |

The mode is a presentation setting (`bansayo.presentation.vr/v1`); the session and content are
identical. Passthrough is offered only when the capability check passes; its use as the *default*
for kicking drills stays Pending Validation ([11 §7.3](11-vr-architecture.md)).

## 3. Spatial calibration: camera world ↔ headset stage

| Step | Detail |
|---|---|
| Transform | `xr_alignment` = `T_xr_stage_from_world` (rigid, gravity-aligned: yaw + translation; pitch and roll are fixed by both systems agreeing on gravity) |
| Method A (exists as a prototype) | Place a controller on the calibration board origin and confirm; yaw from the controller's forward axis along the board arrow |
| Method B | Reference pose wearing the headset: the engine's head keypoint and the headset pose over 2 s give translation; yaw from the head forward axes; needs `multi_view` |
| Method C (refinement) | While both the engine head keypoint and the headset pose are valid, a slow complementary correction of yaw and translation, bounded to 2 cm and 1° per minute, with the residual published |
| Floor | Both systems claim `y = 0`; their difference at the alignment point is stored and shown. Above 3 cm the user is told which floor (runtime or camera calibration) to redo |
| Quality fields (additive on the profile) | `residual_m`, `yaw_residual_rad`, `method: controller \| reference_pose`, `floor_delta_m` |
| Staleness | Residual above 10 cm for 5 s, a runtime recentre event, or a boundary reconfiguration → `stale`; the mirror falls back to the unaligned presentation ([11 §4.2](11-vr-architecture.md)) |

World-anchored content (mirror plane, zone, targets, floor arrows) is authored in engine world
coordinates and drawn through this one transform; nothing is anchored to the headset.

## 4. Avatars and retargeting

| Topic | Decision |
|---|---|
| Source of motion | Engine `BodyState`: rotations and measured keypoints for the 19-segment skeleton. The client does no kinematics beyond applying rotations to bones |
| Retargeting rule | Per bone: `q_avatar_bone = q_world_from_segment · q_rest_offset`, with the offset from the avatar's rest pose to the engine's T-pose rest (existing rule in docs/08 §6) |
| Bone map | One file per avatar, data only: `bansayo.avatar/v1 { model, bones: { <segment>: <bone name> }, rest_offsets: { <segment>: Quat }, scale_reference: "stature" \| "leg_length" }`. Avatars follow a standard humanoid profile so most maps are identical |
| Proportions | The avatar is scaled uniformly to the athlete's stature; limb proportions are **not** re-shaped. Hands and feet therefore land slightly off the measured keypoints; measured end-effector keypoints are drawn as small markers when "show measured points" is on. Trails, targets and arcs always use engine keypoints, never avatar bones |
| Inverse kinematics | **None in the client.** The engine supplies every rotation; unobserved segments arrive as `inferred` and are drawn so. IK that pulls hands or feet to keypoints would invent elbow and knee angles and is rejected; reconsidered only as an engine-side, labelled solver stage |
| Evidence styling | Carried onto the avatar: translucent for inferred segments, a hatched band where twist is inferred ([11 §3](11-vr-architecture.md)). A fully "clean" avatar that hides evidence is not offered |
| Fingers, face, hair, cloth | Static. Not tracked, not animated |
| First-person body | Not rendered ([ADR-033](adr/ADR-033-vr-runtime.md)) |
| Ghost instructor | The same avatar with the ghost material, driven by an engine-computed clip for the viewer's skeleton |
| Assets | Avatars shipped with the project need a licence compatible with the repository; user-supplied avatars are local files |

## 5. Interference and latency

| Issue | Handling |
|---|---|
| Controllers during strikes | Put down or strapped; hand tracking or camera-tracked wrists by default ([11 §6](11-vr-architecture.md)) |
| Hand tracking of fast punches | Runtime validity flags respected; invalid poses are not ingested; punches are not judged from headset hand tracking alone unless V-experiments show it adequate |
| Headset occluding the head from cameras | Head keypoint from the headset pose once `client_ingest` exists (package 6e); until then head landmarks are low-confidence and say so |
| Headset tracking disturbed by fast kicks near its cameras | Pause on tracking loss rather than drift ([11 §7.3](11-vr-architecture.md)) |
| Infra-red or reflective interference between headset tracking and tags | Unknown; checked in the lab with the owner's headset ([29](29-tracking-hardware-lab.md)) |
| Body-stream latency | Mirror and ghost only; budget in [15 §2.2](15-performance-and-latency.md) |
| Passthrough latency and distortion | Runtime property; if the user's real limbs visibly lag, kicking drills revert to immersive with the runtime boundary |

## 6. Passthrough and extension verification matrix (template)

Filled per headset and runtime before any feature depending on it is enabled by default.

| Headset / runtime | OpenXR session | Passthrough | Depth occlusion | Hand tracking | Tracker roles | Boundary geometry exposed | Notes |
|---|---|---|---|---|---|---|---|
| *(owner's headset)* | — | — | — | — | — | — | Not yet tested |

## 7. Safe interaction with virtual targets

Unchanged from [11 §4.4](11-vr-architecture.md) and [11 §7](11-vr-architecture.md): targets are
engine-placed, stationary, inside the zone, at least 1 m from the boundary, visual only, and
"reached", never "hit". In passthrough the zone outline is drawn on the real floor; a target is
never drawn in front of a real object the passthrough depth (where available) reports closer than
the target.

## 8. Validation

Headset checks of [11 §11](11-vr-architecture.md); alignment accuracy against physical floor marks
at four points of the zone; retargeting unit test (T-pose and three scripted poses reproduce the
engine's joint positions within the proportion error); the comfort checklist.
