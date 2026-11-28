extends RefCounted
## Raw OpenXR head and controller-grip poses in XR stage space for the engine uplink.
## Pure: transforms in, wire dictionaries out (contracts/examples/xr_client_frame.json).
## Stage = XROrigin3D's local space, so the room's presentation offset of the origin is removed.
## The grip pose is a controller point, never the anatomical wrist; nothing here aligns poses to
## the engine world (BodyRoot) — that is a later, calibrated step.

## Fixed wire order; the engine refuses any other.
const ROLES: Array[String] = ["head", "left_grip", "right_grip"]
## Engine defaults (xr.max_position_norm_m, xr.max_quaternion_norm_error); a pose outside is sent
## as untracked rather than as a value the engine would refuse.
const MAX_POSITION_NORM_M := 20.0
const MAX_QUATERNION_NORM_ERROR := 0.005

static func untracked(role: String) -> Dictionary:
	return {"role": role, "tracked": false, "position_m": null, "rotation_xyzw": null}

## One device's sample. `stage_from_world` is the inverse of the XR origin's global transform.
static func sample(role: String, stage_from_world: Transform3D, device_global: Transform3D, tracked: bool) -> Dictionary:
	if not tracked:
		return untracked(role)
	var stage_from_device := stage_from_world * device_global
	var p := stage_from_device.origin
	if not stage_from_device.basis.is_finite() or stage_from_device.basis.determinant() <= 0.000001:
		return untracked(role)
	var q := stage_from_device.basis.orthonormalized().get_rotation_quaternion()
	var values := [p.x, p.y, p.z, q.x, q.y, q.z, q.w]
	for v in values:
		if not is_finite(v):
			return untracked(role)
	if p.length() > MAX_POSITION_NORM_M or absf(q.length() - 1.0) > MAX_QUATERNION_NORM_ERROR:
		return untracked(role)
	return {"role": role, "tracked": true, "position_m": [p.x, p.y, p.z], "rotation_xyzw": [q.x, q.y, q.z, q.w]}

## All three devices from one tick. Nodes may be null (then untracked).
static func samples(origin: Node3D, head: Node3D, left: Node3D, right: Node3D, tracked: Array[bool]) -> Array:
	var nodes := [head, left, right]
	var out := []
	var valid_origin := origin != null and origin.global_transform.basis.is_finite() and origin.global_transform.basis.determinant() > 0.000001
	var stage_from_world := origin.global_transform.affine_inverse() if valid_origin else Transform3D.IDENTITY
	for i in range(ROLES.size()):
		var node: Node3D = nodes[i]
		var ok: bool = valid_origin and node != null and i < tracked.size() and tracked[i]
		out.append(sample(ROLES[i], stage_from_world, node.global_transform if node else Transform3D.IDENTITY, ok))
	return out

static func frame(seq: int, client_time_us: int, device_samples: Array) -> Dictionary:
	return {"type": "frame", "seq": seq, "client_time_us": client_time_us, "samples": device_samples}

## Head validity from the XR server's head tracker. XRCamera3D is a Camera3D, not an XRNode3D:
## it has no get_has_tracking_data(), and OpenXR does not report XRInterface tracking status.
static func head_tracked() -> bool:
	var tracker := XRServer.get_tracker(&"head") as XRPositionalTracker
	if tracker == null:
		return false
	var pose := tracker.get_pose(&"default")
	return pose != null and pose.has_tracking_data and pose.tracking_confidence != XRPose.XR_TRACKING_CONFIDENCE_NONE
