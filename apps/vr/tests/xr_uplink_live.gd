extends SceneTree
## Manual check, not CI: the real uplink node against a running engine with xr.enabled, using
## synthetic nodes instead of a headset. Proves the Godot sender and the engine endpoint agree;
## it is not headset validation. Usage (engine already running):
##   godot --headless --path apps/vr --xr-mode off --script tests/xr_uplink_live.gd -- http://127.0.0.1:7700 3
## Streams for N seconds (default 3) with the right grip "occluded" for the middle third.
const XrPoseCapture = preload("res://scripts/xr_pose_capture.gd")
const XrPoseUplink = preload("res://scripts/xr_pose_uplink.gd")

var _origin := Node3D.new()
var _head := Node3D.new()
var _left := Node3D.new()
var _right := Node3D.new()
var _start_us := 0
var _seconds := 3.0

func _initialize() -> void:
	var args := OS.get_cmdline_user_args()
	var url := args[0] if args.size() > 0 else "http://127.0.0.1:7700"
	_seconds = float(args[1]) if args.size() > 1 else 3.0
	root.add_child(_origin)
	for n in [_head, _left, _right]:
		_origin.add_child(n)
	_origin.position = Vector3(0, 0, 3) # Same presentation offset as the training room.
	var uplink := XrPoseUplink.new()
	uplink.configure(url)
	uplink.capture = _capture
	uplink.focused = true
	uplink.status_changed.connect(func(t: String): print("status: ", t))
	root.add_child(uplink)
	_start_us = Time.get_ticks_usec()

func _capture() -> Array:
	var t := (Time.get_ticks_usec() - _start_us) / 1e6
	_head.transform = Transform3D(Basis(Vector3.UP, 0.3 * sin(t)), Vector3(0, 1.7, 0))
	_left.transform = Transform3D(Basis(), Vector3(-0.3, 1.2 + 0.1 * sin(3 * t), -0.2))
	_right.transform = Transform3D(Basis(), Vector3(0.3, 1.2, -0.2))
	var occluded := t > _seconds / 3.0 and t < 2.0 * _seconds / 3.0
	var tracked: Array[bool] = [true, true, not occluded]
	return XrPoseCapture.samples(_origin, _head, _left, _right, tracked)

func _process(_delta: float) -> bool:
	return Time.get_ticks_usec() - _start_us > int(_seconds * 1e6)
