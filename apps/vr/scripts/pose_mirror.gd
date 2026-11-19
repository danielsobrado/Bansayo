extends Node3D

signal closed

const SpatialPanel = preload("res://scripts/spatial_panel.gd")
const MirrorView = preload("res://scripts/pose_mirror_view.gd")
const Guides = preload("res://scripts/pose_guides.gd")
const HOLD_MS := 500
const CONFIDENCE := 0.30
const JOINTS := [
	"head", "left_shoulder", "right_shoulder", "left_elbow", "right_elbow",
	"left_wrist", "right_wrist", "left_hip", "right_hip", "left_knee", "right_knee",
	"left_ankle", "right_ankle",
]
const ALIASES := {
	"nose": "head",
	"left_upper_arm": "left_shoulder", "right_upper_arm": "right_shoulder",
	"left_forearm": "left_elbow", "right_forearm": "right_elbow",
	"left_hand": "left_wrist", "right_hand": "right_wrist",
	"left_thigh": "left_hip", "right_thigh": "right_hip",
	"left_shin": "left_knee", "right_shin": "right_knee",
	"left_foot": "left_ankle", "right_foot": "right_ankle",
}

var panel := SpatialPanel.new()
var view := MirrorView.new()
var _schemas: Dictionary = {}
var _frames: Dictionary = {}
var _training: Dictionary = {}
var _camera: Camera3D
var _xr := false
var _camera_id := ""
var _mode := "session"
var _guide_index := 0
var _phase_index := 0
var _left := true
var _connected := false

func _ready() -> void:
	add_child(panel)
	panel.set_visual(view)
	panel.selected.connect(_choose)
	hide()

func open(camera: Camera3D, xr: bool) -> void:
	_camera = camera
	_xr = xr
	_recenter()
	show()
	_refresh()

func _recenter() -> void:
	panel.recenter(_camera, 1.4 if _xr else 2.0)

func _process(_delta: float) -> void:
	if visible and not view.live_pose.is_empty() and _live_frame().is_empty():
		_refresh()

func connection(connected: bool) -> void:
	_connected = connected
	if not connected:
		_frames.clear()
		_training = {}
	if visible:
		_refresh()

func engine_state(state: Dictionary) -> void:
	var schemas := {}
	for entry in state.get("devices", []):
		var descriptor: Dictionary = entry.get("descriptor", {})
		var camera: Variant = descriptor.get("camera")
		if camera is Dictionary and camera.get("landmark_schema") is Dictionary:
			schemas[str(descriptor.get("id", ""))] = camera["landmark_schema"]
	for id in _frames.keys():
		if schemas.get(id) != _schemas.get(id):
			_frames.erase(id)
	_schemas = schemas
	if not _schemas.has(_camera_id):
		_camera_id = ""
	if visible:
		_refresh()

func training_changed(state: Dictionary) -> void:
	_training = state
	if visible and _mode == "session":
		_refresh()

func landmarks_received(frame: Dictionary) -> void:
	var meta: Dictionary = frame.get("frame_meta", {})
	var id := str(meta.get("device_id", ""))
	var schema: Dictionary = _schemas.get(id, {})
	var dimensions: Array = meta.get("image_size_px", [])
	var samples: Array = frame.get("points", [])
	if schema.is_empty() or schema.get("id") != frame.get("schema_id") or dimensions.size() != 2:
		return
	var width := float(dimensions[0])
	var height := float(dimensions[1])
	if width <= 0.0 or height <= 0.0:
		return
	var names: Array = schema.get("names", [])
	var points := {}
	for i in range(mini(samples.size(), names.size())):
		var point: Variant = samples[i]
		if not point is Dictionary or float(point.get("confidence", 0.0)) < CONFIDENCE:
			continue
		var name := str(names[i])
		name = str(ALIASES.get(name, name))
		var x: Variant = point.get("x_px")
		var y: Variant = point.get("y_px")
		if name not in JOINTS or not (x is float or x is int) or not (y is float or y is int):
			continue
		var position := Vector2(float(x), float(y))
		if position.is_finite() and Rect2(0, 0, width, height).has_point(position):
			points[name] = Vector2(width - position.x, position.y)
	_frames[id] = {"points": points, "received_ms": Time.get_ticks_msec(), "origin": frame.get("origin")}
	if _camera_id.is_empty() and not points.is_empty():
		_camera_id = id
	if visible and id == _camera_id:
		_refresh()

func _live_frame() -> Dictionary:
	var frame: Dictionary = _frames.get(_camera_id, {})
	if frame.is_empty() or Time.get_ticks_msec() - int(frame["received_ms"]) > HOLD_MS:
		return {}
	return frame

func choose_flexibility() -> void:
	_set_mode("flexibility")

func _set_mode(mode: String) -> void:
	_mode = mode
	_guide_index = 0
	_phase_index = 0
	_refresh()

func _guide() -> Dictionary:
	match _mode:
		"workout":
			return Guides.workout(Guides.WORKOUT_ITEMS.keys()[_guide_index])
		"technique":
			return Guides.technique(Guides.TECHNIQUES.keys()[_guide_index],
				["ready", "chamber", "extension", "retraction", "recovery"][_phase_index], _left)
		"flexibility":
			return Guides.flexibility(Guides.FLEXIBILITY.keys()[_guide_index])
	return Guides.for_session(_training)

func _choose(index: int) -> void:
	match index:
		0: closed.emit()
		1: _set_mode("session")
		2: _set_mode("workout")
		3: _set_mode("technique")
		4: choose_flexibility()
		5:
			match _mode:
				"workout": _guide_index = (_guide_index + 1) % Guides.WORKOUT_ITEMS.size()
				"technique": _guide_index = (_guide_index + 1) % Guides.TECHNIQUES.size()
				"flexibility": _guide_index = (_guide_index + 1) % Guides.FLEXIBILITY.size()
			_refresh()
		6:
			var cameras := _schemas.keys()
			cameras.sort()
			if not cameras.is_empty():
				_camera_id = cameras[(cameras.find(_camera_id) + 1) % cameras.size()]
			_refresh()
		7: _recenter()
		8:
			_phase_index = (_phase_index + 1) % 5
			_refresh()
		9:
			_left = not _left
			_refresh()

func _refresh() -> void:
	# Keep the selected camera locked through short occlusions and never mix viewpoints.
	var frame := _live_frame()
	var points: Dictionary = frame.get("points", {})
	var source := _camera_id if not _camera_id.is_empty() else "No camera pose"
	if frame.get("origin") == "simulated":
		source = "Simulated · " + _camera_id
	elif frame.get("origin") == "fixture":
		source = "Recorded · " + _camera_id
	var guide := _guide()
	var title := str(guide.get("title", "Pose mirror"))
	var status := "Camera pose unavailable"
	if not points.is_empty():
		status = "Partial camera pose" if points.size() < JOINTS.size() else "Camera pose available"
	elif not _connected:
		status = "Engine disconnected"
	var current: Variant = _training.get("current")
	var details: Array[String] = ["Illustrated guide · 2D, unscored"]
	if _mode == "session":
		var phase: String = str(_training.get("phase", "")).capitalize()
		if guide.is_empty() and current is Dictionary and not phase in ["Completed", "Aborted"]:
			title = str(current.get("title", "Pose mirror"))
			details = ["%s · No guide for this exercise" % phase]
		elif phase.is_empty() or phase in ["Completed", "Aborted"]:
			details = ["Choose a guide or start an exercise in Bansayo."]
		else:
			details[0] = phase + " · " + details[0]
	view.present(points, guide.get("points", {}), source, str(guide.get("caption", "")))
	var options: Array[String] = ["Back", "Follow session", "Workout guides", "Technique guides", "Flexibility guides",
		"Next guide", "Switch camera", "Recenter"]
	if _mode == "technique":
		options.append_array(["Next phase", "Switch guide side"])
	panel.present(title, status, details, options, -1, false, points.size() < JOINTS.size())
