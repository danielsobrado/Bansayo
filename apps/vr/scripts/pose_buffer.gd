extends RefCounted

# A presentation delay, not a measured end-to-end latency.
const BUFFER_DELAY_US := 50000
const STALE_AFTER_US := 500000
const MAX_FRAMES := 16
const OFFSET_WINDOW := 120
var _frames: Array[Dictionary] = []
var _offsets: Array[int] = []
var _last_arrival_us := 0
var _last_session_us := -1
var _latest_has_pose := false

func clear() -> void:
	_frames.clear()
	_offsets.clear()
	_latest_has_pose = false
	_last_session_us = -1

func push(frame: Dictionary, arrival_us: int = -1) -> void:
	if arrival_us < 0:
		arrival_us = Time.get_ticks_usec()
	var session_us := int(frame.get("t_us", 0))
	if session_us < _last_session_us:
		clear() # Seek or replay restart remaps the session clock.
	_last_session_us = session_us
	_last_arrival_us = arrival_us
	_latest_has_pose = frame.get("pose") is Dictionary
	if not _latest_has_pose:
		_frames.clear()
		return
	_offsets.append(arrival_us - session_us)
	if _offsets.size() > OFFSET_WINDOW:
		_offsets.pop_front()
	_frames.append(frame)
	if _frames.size() > MAX_FRAMES:
		_frames.pop_front()

func sample(now_us: int = -1) -> Dictionary:
	if now_us < 0:
		now_us = Time.get_ticks_usec()
	if not _latest_has_pose or _frames.is_empty() or now_us - _last_arrival_us > STALE_AFTER_US:
		return {}
	var offset: int = _offsets.min()
	var render_us := now_us - offset - BUFFER_DELAY_US
	if render_us < int(_frames.front()["t_us"]):
		return _frames.front()["pose"]
	for i in range(1, _frames.size()):
		var a := _frames[i - 1]
		var b := _frames[i]
		if int(a["t_us"]) <= render_us and render_us <= int(b["t_us"]):
			var span := maxi(1, int(b["t_us"]) - int(a["t_us"]))
			return interpolate(a["pose"], b["pose"], clampf(float(render_us - int(a["t_us"])) / span, 0.0, 1.0))
	return _frames.back()["pose"] # Brief hold; no extrapolation.

static func vector(value: Array) -> Vector3:
	return Vector3(float(value[0]), float(value[1]), float(value[2]))

static func quaternion(value: Array) -> Quaternion:
	return Quaternion(float(value[0]), float(value[1]), float(value[2]), float(value[3])).normalized()

static func interpolate(a: Dictionary, b: Dictionary, fraction: float) -> Dictionary:
	if a.get("skeleton_rev") != b.get("skeleton_rev"):
		return b
	var result: Dictionary = b.duplicate(true)
	var segments_a: Array = a.get("segments", [])
	var segments_b: Array = result.get("segments", [])
	for i in range(mini(segments_a.size(), segments_b.size())):
		var previous: Dictionary = segments_a[i]
		var current: Dictionary = segments_b[i]
		if previous.get("provenance") != current.get("provenance"):
			continue
		var position := vector(previous["position_m"]).lerp(vector(current["position_m"]), fraction)
		var rotation := quaternion(previous["rotation"]).slerp(quaternion(current["rotation"]), fraction)
		current["position_m"] = [position.x, position.y, position.z]
		current["rotation"] = [rotation.x, rotation.y, rotation.z, rotation.w]
	return result
