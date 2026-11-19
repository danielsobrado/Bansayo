extends Node3D
## Setup and on-demand tracking diagnostics.
const SpatialPanel = preload("res://scripts/spatial_panel.gd")
const MovementCounter = preload("res://scripts/movement_counter.gd")
const READOUTS := ["left_knee_flexion", "right_knee_flexion", "left_hip_flexion", "right_hip_flexion", "trunk_forward_lean"]
var counter := MovementCounter.new()
var alignment_text := "XR alignment: assumed identity"
var action_text := ""
var panel := SpatialPanel.new()
var diagnostics := false
var preferences: Array[String] = []
var _tracking_state := ""
var _simulated := false
var _connected := false
var _connection_message := "Engine not connected"
var _state: Dictionary = {}
var _frame: Dictionary = {}
var _joint_ids: Array = []
var _joint_labels: Array = []
var _segment_ids: Array = []
var _gap := true
var _next_refresh_ms := 0

func _ready() -> void:
	add_child(panel)
	_refresh()

func connection(connected: bool, message: String) -> void:
	_connected = connected
	_connection_message = message
	if not connected:
		_frame.clear()
	_refresh()

func skeleton_changed(definition: Dictionary) -> void:
	_joint_ids = definition.get("joints", []).map(func(j): return str(j.get("id", "")))
	_joint_labels = definition.get("joints", []).map(func(j): return str(j.get("label", "")))
	_segment_ids = definition.get("segments", []).map(func(s): return str(s.get("id", "")))

func state_changed(state: Dictionary) -> void:
	_state = state
	_refresh()

func frame_received(frame: Dictionary) -> void:
	_frame = frame
	_tracking_state = str(frame.get("quality", {}).get("state", "unavailable"))
	_simulated = "simulated" in frame.get("origins", [])
	var angles := {}
	var joints: Array = frame.get("joints", [])
	for i in range(mini(joints.size(), _joint_ids.size())):
		var value: Variant = joints[i].get("value")
		if value != null:
			angles[_joint_ids[i]] = rad_to_deg(float(value))
	var heights := {}
	var pose: Variant = frame.get("pose")
	if pose is Dictionary:
		var segments: Array = pose.get("segments", [])
		# A segment's origin follows its parent, so the ankle (foot origin) is as good as the shin.
		# [reported name, segment whose origin it is, segment that must not be inferred]
		for item in [["pelvis", "pelvis", "pelvis"], ["left_ankle", "left_foot", "left_shin"], ["right_ankle", "right_foot", "right_shin"]]:
			var at := _segment_ids.find(item[1])
			var by := _segment_ids.find(item[2])
			if at < 0 or by < 0 or maxi(at, by) >= segments.size():
				continue
			if segments[at].get("position_m") is Array and str(segments[by].get("provenance", "")) != "inferred":
				heights[item[0]] = float(segments[at]["position_m"][1])
	counter.update(angles, heights)
	# Text rebuilds are throttled; the stream arrives at up to 60 Hz.
	if Time.get_ticks_msec() >= _next_refresh_ms:
		_next_refresh_ms = Time.get_ticks_msec() + 100
		_refresh()

func set_gap(gap: bool) -> void:
	if _gap != gap:
		_gap = gap
		_refresh()

func set_action(text: String) -> void:
	action_text = text
	_refresh()

func tracking_label() -> String:
	if not _connected:
		return "Engine disconnected"
	if _gap:
		return "Tracking unavailable"
	if _tracking_state == "tracking":
		return "Simulated tracking" if _simulated else "Tracking ready"
	if _tracking_state == "degraded":
		return "Tracking limited"
	return "Tracking " + _tracking_state if not _tracking_state.is_empty() else "Waiting for tracking"

func set_preferences(options: Array[String]) -> void:
	preferences = options
	_refresh()

func set_diagnostics(enabled: bool) -> void:
	diagnostics = enabled
	_refresh()

func _refresh() -> void:
	var lines: Array[String] = [_connection_message]
	if not preferences.is_empty():
		lines = ["Set your panel size and height, choose an avatar and room, and control feedback sounds."]
		if not action_text.is_empty():
			lines.append(action_text)
		panel.present("Preferences", "", lines, preferences, -1, false)
		return
	if not diagnostics:
		lines.append(alignment_text)
		lines.append_array(_camera_lines())
		if not action_text.is_empty():
			lines.append(action_text)
		var options: Array[String] = ["Taekwondo clinic", "Pose mirror", "Tracking diagnostics", "Preferences", "Recenter", "Dismiss"]
		var status := tracking_label()
		panel.present("Bansayo", status, lines, options, -1, false, status not in ["Tracking ready", "Simulated tracking"], not _connected)
		return
	if _connected and not _frame.is_empty():
		var origins: Array = _frame.get("origins", [])
		var origin_text := ", ".join(PackedStringArray(origins)).to_upper()
		if str(_state.get("engine", {}).get("mode", "")) == "replay":
			origin_text = "REPLAY · " + origin_text
		if _state.get("recording") is Dictionary:
			origin_text = "● REC · " + origin_text
		var quality: Dictionary = _frame.get("quality", {})
		lines.append(origin_text)
		lines.append("%s · %d/%d segments" % [str(quality.get("state", "unavailable")), int(quality.get("observed_segments", 0)), int(quality.get("total_segments", 0))])
		lines.append_array(_camera_lines())
		if _gap:
			lines.append("No pose data · stream gap or pose unavailable")
		lines.append("")
		lines.append_array(_joint_lines())
		lines.append("Avatar · white measured, cyan derived, violet inferred. Missing segments are hidden.")
		lines.append("Squats %d · Kicks L %d / R %d" % [counter.squats, counter.kicks["left"], counter.kicks["right"]])
		var ankle := "—" if counter.max_ankle_height_m <= 0.0 else "%.2f m" % counter.max_ankle_height_m
		var pelvis := "—" if counter.min_pelvis_height_m == INF else "%.2f m" % counter.min_pelvis_height_m
		lines.append("Max ankle height %s · Min pelvis height %s" % [ankle, pelvis])
	lines.append("")
	lines.append(alignment_text)
	lines.append("Left stick or T: clinic · Right stick: align")
	lines.append("A/X or R: record · B/Y or P: replay · C: reset counts")
	if not action_text.is_empty():
		lines.append(action_text)
	lines.append("50 ms interpolation buffer · joint angles not validated")
	var options: Array[String] = ["Back to setup", "Record / stop", "Replay / live", "Reset diagnostic counts", "Recenter"]
	var status := tracking_label()
	panel.present("Tracking diagnostics", status, lines, options, -1, false, status not in ["Tracking ready", "Simulated tracking"], not _connected)

func _camera_lines() -> Array[String]:
	var out: Array[String] = []
	var cameras: Array[String] = []
	for device in _state.get("devices", []):
		var descriptor: Dictionary = device.get("descriptor", {})
		if str(descriptor.get("kind", "")) != "camera":
			continue
		var rate := float(device.get("stream", {}).get("measured_rate_hz", 0.0))
		cameras.append("%s %s %.0f Hz" % [str(descriptor.get("id", "?")), str(device.get("state", "?")), rate])
	if not cameras.is_empty():
		out.append("Cameras: " + " · ".join(PackedStringArray(cameras)))
	for feature in _state.get("features", []):
		var id := str(feature.get("id", ""))
		if id in ["landmarks_3d_triangulated", "skeleton_pose"] and not cameras.is_empty():
			var reasons: Array = feature.get("reasons", [])
			var why := "" if reasons.is_empty() else " (" + str(reasons[0].get("code", "")) + ")"
			out.append("%s: %s%s" % [id.replace("_", " "), str(feature.get("state", "?")), why])
	return out

func _joint_lines() -> Array[String]:
	var out: Array[String] = []
	var joints: Array = _frame.get("joints", [])
	for id in READOUTS:
		var i := _joint_ids.find(id)
		if i < 0 or i >= joints.size():
			continue
		var value: Variant = joints[i].get("value")
		var now := "—" if value == null else "%5.1f°" % rad_to_deg(float(value))
		var peak := "—" if not counter.peaks.has(id) else "%5.1f°" % counter.peaks[id]
		out.append("%s  %s   peak %s" % [_joint_labels[i], now, peak])
	return out
