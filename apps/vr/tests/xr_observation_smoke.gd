extends SceneTree
## Phase 01: OpenXR pose capture without a headset — stage conversion, wire shape, tracking loss,
## role order, URL and the desktop fallback. Headless success is not headset validation.
const XrPoseCapture = preload("res://scripts/xr_pose_capture.gd")
const XrPoseUplink = preload("res://scripts/xr_pose_uplink.gd")

var _failures: Array[String] = []

func _initialize() -> void:
	_run.call_deferred()

func _check(ok: bool, name: String) -> void:
	if not ok:
		_failures.append(name)
		push_error("xr_observation_smoke: " + name)

func _sorted(a: Array) -> Array:
	var b := a.duplicate()
	b.sort()
	return b

func _pos(s: Dictionary) -> Vector3:
	return Vector3(s["position_m"][0], s["position_m"][1], s["position_m"][2])

func _quat(s: Dictionary) -> Quaternion:
	var q: Array = s["rotation_xyzw"]
	return Quaternion(q[0], q[1], q[2], q[3])

func _rig(origin_transform: Transform3D, head_local: Transform3D) -> Array:
	var origin := Node3D.new()
	var head := Node3D.new()
	var left := Node3D.new()
	var right := Node3D.new()
	root.add_child(origin)
	for n in [head, left, right]:
		origin.add_child(n)
	origin.transform = origin_transform
	head.transform = head_local
	left.transform = Transform3D(Basis(), Vector3(-0.3, 1.2, -0.2))
	right.transform = Transform3D(Basis(Vector3.UP, 0.5), Vector3(0.3, 1.2, -0.2))
	return [origin, head, left, right]

func _run() -> void:
	var all: Array[bool] = [true, true, true]
	var turn := Basis(Vector3.UP, deg_to_rad(30.0))
	var head_local := Transform3D(turn, Vector3(0.0, 1.7, 0.0))

	# identity_stage_transform
	var r := _rig(Transform3D.IDENTITY, head_local)
	var s: Array = XrPoseCapture.samples(r[0], r[1], r[2], r[3], all)
	_check(_pos(s[0]).is_equal_approx(Vector3(0, 1.7, 0)), "identity_stage_transform")

	# presentation_offset_is_removed: origin at (0,0,3), head world (0,1.7,3) -> stage (0,1.7,0)
	r = _rig(Transform3D(Basis(), Vector3(0, 0, 3)), Transform3D(Basis(), Vector3(0, 1.7, 0)))
	_check((r[1] as Node3D).global_position.is_equal_approx(Vector3(0, 1.7, 3)), "fixture head world position")
	s = XrPoseCapture.samples(r[0], r[1], r[2], r[3], all)
	_check(_pos(s[0]).is_equal_approx(Vector3(0, 1.7, 0)), "presentation_offset_is_removed")

	# origin_yaw_is_removed: rotating and moving the origin leaves stage poses unchanged
	r = _rig(Transform3D(Basis(Vector3.UP, 1.1), Vector3(2, 0, -1)), head_local)
	var yawed: Array = XrPoseCapture.samples(r[0], r[1], r[2], r[3], all)
	r = _rig(Transform3D.IDENTITY, head_local)
	var plain: Array = XrPoseCapture.samples(r[0], r[1], r[2], r[3], all)
	for i in range(3):
		_check(_pos(yawed[i]).is_equal_approx(_pos(plain[i])), "origin_yaw_is_removed position %d" % i)
		_check(_quat(yawed[i]).is_equal_approx(_quat(plain[i])), "origin_yaw_is_removed rotation %d" % i)
	_check(_quat(plain[0]).is_equal_approx(turn.get_rotation_quaternion()), "head rotation is q_stage_from_device")

	# xyzw_unit_quaternion_serialization: [x, y, z, w], finite, unit; JSON keeps the order
	var q: Array = plain[2]["rotation_xyzw"]
	var expect := Basis(Vector3.UP, 0.5).get_rotation_quaternion()
	_check(q.size() == 4 and is_equal_approx(q[1], expect.y) and is_equal_approx(q[3], expect.w), "xyzw order")
	_check(absf(_quat(plain[2]).length() - 1.0) < 1e-5, "unit quaternion")
	var wire: Variant = JSON.parse_string(JSON.stringify(XrPoseCapture.frame(7, 123456789, plain)))
	_check(wire is Dictionary and int(wire["seq"]) == 7 and int(wire["client_time_us"]) == 123456789, "frame ints survive JSON")
	# Same keys as the engine's golden example, so the wire contract is not duplicated here
	# (JSON.stringify sorts keys; order carries no meaning on the wire).
	var golden: Dictionary = JSON.parse_string(FileAccess.get_file_as_string(
		ProjectSettings.globalize_path("res://../../contracts/examples/xr_client_frame.json")))
	_check(_sorted(wire.keys()) == _sorted(golden.keys()), "frame keys match golden")
	_check(_sorted(wire["samples"][0].keys()) == _sorted(golden["samples"][0].keys()), "sample keys match golden")

	# tracking_lost_emits_null_pose: no fabricated zero or identity pose
	var lost: Array[bool] = [true, true, false]
	s = XrPoseCapture.samples(r[0], r[1], r[2], r[3], lost)
	_check(s[2] == {"role": "right_grip", "tracked": false, "position_m": null, "rotation_xyzw": null}, "tracking_lost_emits_null_pose")
	_check(s[0]["tracked"] and s[1]["tracked"], "others continue when one is lost")
	var far := XrPoseCapture.sample("head", Transform3D.IDENTITY, Transform3D(Basis(), Vector3(0, 0, 25)), true)
	_check(not far["tracked"] and far["position_m"] == null, "out-of-bound pose is not sent as tracked")
	var nan := XrPoseCapture.sample("head", Transform3D.IDENTITY, Transform3D(Basis(), Vector3(NAN, 0, 0)), true)
	_check(not nan["tracked"], "non-finite pose is not sent as tracked")
	s = XrPoseCapture.samples(null, null, null, null, all)
	_check(s.all(func(x): return not x["tracked"]), "missing nodes are untracked")

	# samples_have_fixed_role_order
	_check(plain.map(func(x): return x["role"]) == ["head", "left_grip", "right_grip"], "samples_have_fixed_role_order")

	# URL: same base as the downlink, ws/wss, fixed path, no token
	_check(XrPoseUplink.ingest_url("http://127.0.0.1:7700/") == "ws://127.0.0.1:7700/api/v1/ingest/xr", "ingest url http")
	_check(XrPoseUplink.ingest_url("https://example.test") == "wss://example.test/api/v1/ingest/xr", "ingest url https")
	_check(not XrPoseUplink.enabled(), "uplink is off by default")

	# desktop_mode_does_not_require_openxr: the room starts in desktop mode, no grips, no uplink
	var room: Node = load("res://training_room.tscn").instantiate()
	root.add_child(room)
	await process_frame
	await process_frame
	_check(room.get("_xr_interface") == null, "desktop mode without OpenXR")
	_check(room.get("_xr_uplink") == null, "desktop_mode_does_not_require_openxr: no uplink")
	_check((room.get("_grips") as Array).is_empty(), "desktop mode creates no grip nodes")
	room.queue_free()
	await process_frame

	if _failures.is_empty():
		print("Bansayo XR observation smoke passed")
		quit()
	else:
		push_error("Failures: %s" % ", ".join(_failures))
		quit(1)
