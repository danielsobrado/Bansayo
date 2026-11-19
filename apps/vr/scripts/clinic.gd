extends Node3D

const Ghost = preload("res://scripts/clinic_ghost.gd")
const ClinicTarget = preload("res://scripts/clinic_target.gd")
const GhostWalls = preload("res://scripts/ghost_walls.gd")
const PoseBuffer = preload("res://scripts/pose_buffer.gd")
const EngineActions = preload("res://scripts/engine_actions.gd")
const KICKS := ["taekwondo.ap_chagi", "taekwondo.dollyeo_chagi"]
const TITLES := ["Front kick", "Roundhouse kick"]
const METRICS := {"kick.retraction_time": "Retraction", "kick.peak_ankle_height": "Foot height", "kick.support_pivot": "Support pivot"}
var actions: EngineActions
var opened := false
var connected := false
var xr: XRInterface
var xr_focused := false
var camera: Camera3D
var world_frame: Node3D
var right_hand: XRController3D
var aligned := false
var pose_available := false
var _label := Label3D.new()
var _ghost := Ghost.new()
var _target := ClinicTarget.new()
var _walls := GhostWalls.new()
var _engine_busy := false
var _restore_ghost := false
var _athletes: Array = []
var _athlete := 0
var _kick := 0
var _side := "right"
var _sets := 2
var _rest := 30
var _choice := 0
var _pending := false
var _session := ""
var _stream_session := ""
var _needs_resync := true
var _state: Dictionary = {}
var _texts: Dictionary = {}
var _definition: Dictionary = {}
var _review: Dictionary = {}
var _notice := ""
var _cue := ""
var _reference := ""
var _center := Vector2.ZERO
var _space_confirmed := false
var _native_session := false
var _use_targets := false
var _target_reaches := 0
var _last_reach_seq := 0
var _next_safety_ms := 0

func _ready() -> void:
	_label.font_size = 40
	_label.pixel_size = 0.00135
	_label.outline_size = 5
	_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_LEFT
	_label.position = Vector3(-1.0, 1.55, 0)
	add_child(_label)
	_ghost.position = Vector3(0.95, 0, 0)
	add_child(_ghost)
	add_child(_target)
	add_child(_walls)
	hide()

func build(skeleton: Dictionary) -> void:
	_ghost.build(skeleton)
	_reference = ""

func connection(active: bool) -> void:
	connected = active
	if not active:
		_needs_resync = true
		_space_confirmed = false
		_ghost.hide()
		_target.clear()
		_restore_ghost = false
	_refresh()

func engine_state(state: Dictionary) -> void:
	var session: Variant = state.get("training")
	_engine_busy = session is Dictionary and session.get("phase") not in ["completed", "aborted"]

func set_notice(message: String) -> void:
	_notice = message
	_refresh()

func show_menu() -> void:
	if opened or not is_instance_valid(camera):
		return
	var forward := -camera.global_basis.z
	forward.y = 0.0
	if forward.length_squared() < 0.01:
		forward = Vector3.FORWARD
	forward = forward.normalized()
	var ground := camera.global_position + forward * 2.8
	ground.y = 0
	global_transform = Transform3D(Basis.looking_at(forward, Vector3.UP), ground)
	opened = true
	show()
	_refresh()
	await _load_menu()

func _load_menu() -> void:
	_pending = true
	var people: Variant = await actions.request(HTTPClient.METHOD_GET, "/athletes")
	if people is Array:
		_athletes = people
		_athlete = mini(_athlete, maxi(0, _athletes.size() - 1))
	var texts: Variant = await actions.request(HTTPClient.METHOD_GET, "/content/i18n/en")
	if texts is Dictionary:
		_texts = texts
	_pending = false
	_refresh()

func stream_state_changed(state: Dictionary) -> void:
	var id := str(state.get("session_id", ""))
	if not _session.is_empty() and id != _session and (_needs_resync or _stream_session == _session):
		if _phase() not in ["completed", "aborted"]:
			_session = ""
			_state = {}
			_choice = 0
			_space_confirmed = false
			_target_reaches = 0
			_cue = ""
			_target.clear()
			_clear_reference()
			set_notice("The previous clinic is no longer the current session.")
	_stream_session = id
	_needs_resync = false
	state_changed(state)

func state_changed(state: Dictionary) -> void:
	if _session.is_empty() or str(state.get("session_id", "")) != _session:
		return
	if _state.get("session_id") == _session and int(state.get("rev", 0)) < int(_state.get("rev", 0)):
		return
	var changed := _phase() != str(state.get("phase", "setup"))
	_state = state
	if changed:
		_choice = 0
		if _phase() == "active":
			_cue = ""
	_refresh()

func event_received(session_id: String, event: Dictionary) -> void:
	if session_id != _session:
		return
	if event.get("type") == "target_reached":
		var seq := int(event.get("event_seq", 0))
		if seq > _last_reach_seq:
			_last_reach_seq = seq
			_target_reaches += 1
			_target.reach(str(event.get("data", {}).get("id", "")))
			_refresh()
		return
	if event.get("type") != "set_completed":
		return
	_cue = ""
	for cue in event.get("data", {}).get("cues", []):
		if not bool(cue.get("resolved", false)) and not str(cue.get("id", "")).begins_with("improved."):
			_cue = _text(str(cue.get("suggestion", {}).get("key", "")))
			break
	_refresh()

func _phase() -> String:
	return str(_state.get("phase", "setup"))

func _menu() -> Array[String]:
	match _phase():
		"preparing":
			for gate in _state.get("readiness", []):
				if gate.get("gate") == "safety_ack" and gate.get("state") == "fail" and not _definition.is_empty():
					return ["Acknowledge safety notes", "End clinic"]
			return ["End clinic"]
		"ready":
			return ["Place target & start", "Start without target", "End clinic"] if _use_targets else ["Confirm clear area & start", "End clinic"]
		"countdown": return ["End clinic"]
		"active": return ["Pause", "End clinic"]
		"resting": return ["Skip rest", "Pause", "End clinic"]
		"paused": return ["Confirm clear area & resume", "End clinic"]
		"completed", "aborted":
			return ["Review sets & replay ghost", "Practise again", "Change settings", "Ghost walls", "Close clinic"]
	var choices: Array[String] = [
		"Athlete · " + (str(_athletes[_athlete].get("display_name", "")) if not _athletes.is_empty() else "None"),
		"Kick · " + TITLES[_kick], "Side · " + _side,
		"Sets · %d × 4" % _sets, "Rest · %d s" % _rest,
		"Load previous practice ghost", "Prepare clinic", "Ghost walls", "Close clinic",
	]
	if xr != null:
		choices.insert(5, "Targets · " + ("On" if _use_targets else "Off"))
	return choices

func next() -> void:
	if _walls.opened:
		_walls.next()
		return
	if opened:
		_choice = (_choice + 1) % _menu().size()
		_refresh()

func select() -> void:
	if _walls.opened:
		_walls.select(_wall_space_ok())
		if not _walls.opened:
			_label.show()
			if _restore_ghost:
				_ghost.show()
			_refresh()
		return
	if not opened or _pending:
		return
	var menu := _menu()
	var action := menu[mini(_choice, menu.size() - 1)]
	if action == "Ghost walls":
		if _engine_busy:
			set_notice("Finish the active training session before starting Ghost Walls.")
			return
		_restore_ghost = _ghost.visible
		_ghost.hide()
		_label.hide()
		_walls.open(camera, xr != null)
		return
	if not connected and action != "Close clinic":
		return
	_pending = true
	match action:
		"Prepare clinic", "Practise again":
			await _prepare()
		"Load previous practice ghost":
			await _prior_reference()
		"Review sets & replay ghost":
			await _load_review(_session)
		"Acknowledge safety notes":
			var current: Dictionary = _state.get("current", {})
			await _command("ack_safety", {"content_refs": [current.get("exercise", {}).get("ref", "")]})
		"Confirm clear area & start", "Confirm clear area & resume":
			if _confirm_space():
				await _command("start" if _phase() == "ready" else "resume")
		"Place target & start":
			if _confirm_space():
				var position: Variant = _target_position()
				if position is Vector3:
					await _command("start", {"target_position_m": [position.x, position.y, position.z]})
		"Start without target":
			if _confirm_space():
				await _command("start")
		"Pause": await _command("pause")
		"Skip rest": await _command("skip_rest")
		"End clinic": await _command("end")
		"Change settings":
			_state = {}
			_session = ""
			_choice = 0
		"Close clinic":
			opened = false
			hide()
		_:
			if action.begins_with("Targets · "):
				_use_targets = not _use_targets
			match _choice:
				0:
					if not _athletes.is_empty():
						_athlete = (_athlete + 1) % _athletes.size()
					_clear_reference()
				1:
					_kick = 1 - _kick
					_clear_reference()
				2:
					_side = "left" if _side == "right" else "right"
					_clear_reference()
				3: _sets = 2 if _sets == 4 else _sets + 1
				4: _rest = 15 if _rest == 60 else _rest + 15
	_pending = false
	_refresh()

func _prepare() -> void:
	if _athletes.is_empty():
		_notice = "Create an athlete in desktop Train or mobile Admin."
		return
	var person: Dictionary = _athletes[_athlete]
	var athlete: Variant = await actions.request(HTTPClient.METHOD_PUT, "/athletes/active", JSON.stringify({"athlete_id": person["id"]}))
	if athlete == null:
		return
	var def: Variant = await actions.request(HTTPClient.METHOD_GET, "/content/techniques/" + KICKS[_kick])
	if not def is Dictionary or not def.get("safety") is Dictionary:
		_notice = "The technique's safety notes are unavailable."
		return
	_definition = def
	_review = {}
	_cue = ""
	_space_confirmed = false
	var result: Variant = await actions.request(HTTPClient.METHOD_POST, "/training/sessions", JSON.stringify({
		"athlete_id": person["id"],
		"plan": {"kind": "technique", "technique": KICKS[_kick], "side": _side,
				"sets": _sets, "reps": 4, "rest_s": _rest, "stages": ["normal"], "coaching": "minimal"},
		"record": {"inputs": false, "motion": true, "video": false}
	}))
	if result is Dictionary:
		_native_session = xr != null
		_session = str(result.get("session_id", ""))
		_target_reaches = 0
		_last_reach_seq = 0
		_target.clear()
		state_changed(result)

func _command(command: String, args: Dictionary = {}, quiet := false) -> void:
	var result: Variant = await actions.request(HTTPClient.METHOD_POST, "/training/command",
		JSON.stringify({"command": command, "args": args, "session_id": _session}), "", quiet)
	if result is Dictionary:
		state_changed(result)

static func circle_fits(bounds: PackedVector3Array, point: Vector2, radius: float) -> bool:
	if bounds.size() < 3:
		return false
	var polygon := PackedVector2Array()
	for vertex in bounds:
		polygon.append(Vector2(vertex.x, vertex.z))
	if not Geometry2D.is_point_in_polygon(point, polygon):
		return false
	for i in range(polygon.size()):
		var edge := Geometry2D.get_closest_point_to_segment(point, polygon[i], polygon[(i + 1) % polygon.size()])
		if point.distance_to(edge) < radius:
			return false
	return true

func _head_position() -> Vector2:
	return Vector2(camera.position.x, camera.position.z)

func _confirm_space() -> bool:
	if xr == null: # Desktop preview exercises the engine flow without a headset.
		if _native_session:
			_notice = "Reconnect the headset to resume this clinic."
			return false
		_space_confirmed = true
		return true
	var radius := float(_definition.get("safety", {}).get("min_clear_radius_m", 2.0))
	if not xr_focused or xr.get_tracking_status() != XRInterface.XR_NORMAL_TRACKING or not circle_fits(xr.get_play_area(), _head_position(), radius + 0.30):
		_notice = "Clear play area needs %.1f m around you. Centre yourself within the headset boundary." % (radius + 0.30)
		return false
	_center = _head_position()
	_space_confirmed = true
	_notice = ""
	return true

func _wall_space_ok() -> bool:
	if _engine_busy:
		return false
	if not _walls.native:
		return true
	if xr == null or not xr_focused or xr.get_tracking_status() != XRInterface.XR_NORMAL_TRACKING:
		return false
	var radius := 1.0 + GhostWalls.ALLOWED_SHIFT if _walls.phase == "ready" else 1.0
	return circle_fits(xr.get_play_area(), _head_position(), radius)

func _process(delta: float) -> void:
	if _walls.opened:
		var preview := Vector3.ZERO
		if not _walls.native:
			preview.x = 0.22 * (int(Input.is_physical_key_pressed(KEY_RIGHT)) - int(Input.is_physical_key_pressed(KEY_LEFT)))
			preview.y = -0.19 if Input.is_physical_key_pressed(KEY_DOWN) else 0.0
		_walls.step(delta, _wall_space_ok(), preview)
		return
	if (opened and connected and _phase() == "ready" and _use_targets and aligned and xr_focused
			and is_instance_valid(right_hand) and right_hand.get_has_tracking_data()):
		var point := right_hand.global_position
		var radius := float(_definition.get("target", {}).get("radius_m", 0.15))
		if _target_inside_bounds(point, radius):
			_target.present("placement-preview", point, radius, camera.global_position, world_frame.to_local(point).y)
			return
	if not opened or _phase() not in ["active", "countdown", "resting"]:
		if _target.visible:
			_target.clear()
		return
	if xr == null:
		if _native_session:
			suspend()
		else:
			_update_target()
		return
	var valid := _space_confirmed and xr_focused and xr.get_tracking_status() == XRInterface.XR_NORMAL_TRACKING
	var radius := float(_definition.get("safety", {}).get("min_clear_radius_m", 2.0))
	valid = valid and _head_position().distance_to(_center) <= 0.30 and circle_fits(xr.get_play_area(), _head_position(), radius)
	if not valid:
		suspend()
	else:
		_update_target()

func _target_position() -> Variant:
	if not aligned or not is_instance_valid(world_frame):
		_notice = "Align to the calibration board with the right stick before placing a target."
		return null
	if not is_instance_valid(right_hand) or not right_hand.get_has_tracking_data():
		_notice = "The right controller is needed to place the target."
		return null
	var point := right_hand.global_position
	var radius := float(_definition.get("target", {}).get("radius_m", 0.15))
	if not _target_inside_bounds(point, radius):
		_notice = "Place the target farther from the play-area edge."
		return null
	return world_frame.to_local(point)

func _target_inside_bounds(point: Vector3, radius: float) -> bool:
	if xr == null:
		return false
	var local: Vector3 = camera.get_parent().to_local(point)
	return circle_fits(xr.get_play_area(), Vector2(local.x, local.z), 1.0 + radius)

func _update_target() -> void:
	var current: Variant = _state.get("current")
	if connected and pose_available and _phase() == "active" and current is Dictionary:
		var placements: Array = current.get("targets", [])
		if not placements.is_empty() and current.get("sub_state") != "tracking_lost":
			var item: Dictionary = placements[0]
			var local := PoseBuffer.vector(item["position_m"])
			var point := world_frame.to_global(local)
			var radius := float(item["radius_m"])
			if xr == null or (aligned and _target_inside_bounds(point, radius)):
				_target.present(str(item["id"]), point, radius, camera.global_position, local.y)
				return
	if _target.visible:
		_target.clear()

func suspend() -> void:
	_walls.interrupt("Tracking paused · confirm the clear area to resume.")
	if _phase() not in ["active", "countdown", "resting"]:
		return
	_space_confirmed = false
	_ghost.hide()
	_target.clear()
	_notice = "Stop and return to the centre of your clear area."
	_refresh()
	var now := Time.get_ticks_msec()
	if not _pending and connected and now >= _next_safety_ms:
		_next_safety_ms = now + 500
		_command("end" if _phase() == "countdown" else "pause", {}, true)

func _clear_reference() -> void:
	_reference = ""
	_ghost.clear()
	_review = {}

func _prior_reference() -> void:
	if _athletes.is_empty():
		return
	var id := str(_athletes[_athlete]["id"]).uri_encode()
	var history: Variant = await actions.request(HTTPClient.METHOD_GET, "/training/sessions?athlete_id=" + id + "&limit=20")
	if history is Array:
		for session in history:
			if not bool(session.get("has_motion", false)) or session.get("plan_kind") != "technique":
				continue
			var sid := str(session["id"]).uri_encode()
			var review: Variant = await actions.request(HTTPClient.METHOD_GET, "/training/sessions/" + sid + "/clinic-review", "", "", true)
			if review is Dictionary and review.get("side") == _side and str(review.get("technique", "")).get_slice("@", 0) == KICKS[_kick]:
				if await _load_ghost(sid, review):
					return
	_notice = "No saved motion for this kick and side. Finish a clinic with motion consent to create a ghost."

func _load_review(id: String) -> void:
	var result: Variant = await actions.request(HTTPClient.METHOD_GET, "/training/sessions/" + id.uri_encode() + "/clinic-review", "", "", true)
	if result is Dictionary:
		_review = result
		if not await _load_ghost(id.uri_encode(), _review):
			_notice = "Review loaded. A compatible motion clip is unavailable."
	else:
		_notice = "No saved attempts to review yet. Motion consent is needed for history."

func _load_ghost(id: String, review: Dictionary) -> bool:
	var rep := int(review.get("first", {}).get("rep_index", 0))
	var motion: Variant = await actions.request(HTTPClient.METHOD_GET, "/training/reps/%s/%d/motion" % [id, rep], "", "", true)
	if not motion is Dictionary:
		return false
	var height: Dictionary = {}
	for row in review.get("measurements", []):
		if row.get("metric") == "kick.peak_ankle_height" and row.get("first") is Dictionary:
			height = row["first"]
	var show_path: bool = height.get("grade") in ["A", "B"] and height.get("value") != null and height.get("unit") == "m"
	if not _ghost.show_clip(motion, show_path):
		return false
	_reference = "Recorded reference · ½ speed · practise in place\n" + ("Peak foot height  " + _metric(height) if show_path else "Spatial height unavailable")
	_notice = ""
	return true

func _text(key: String) -> String:
	return str(_texts.get(key, key))

func _metric(m: Variant) -> String:
	if not m is Dictionary or m.get("grade") == "X":
		return "Unavailable"
	var value: Variant = m.get("value")
	if value == null:
		return "%.2f body-relative [%s]" % [float(m["norm"]), m["grade"]] if m.get("norm") != null else "Unavailable"
	match m.get("unit"):
		"m": return "%.0f cm [%s]" % [float(value) * 100.0, m["grade"]]
		"rad": return "%.0f° [%s]" % [rad_to_deg(float(value)), m["grade"]]
	return "%.0f %s [%s]" % [float(value), m.get("unit", ""), m.get("grade", "")]

func _refresh() -> void:
	var lines: Array[String] = ["TAEKWONDO CLINIC"]
	if not connected:
		lines.append("Engine connection needed. Stop practice.")
	if not _state.is_empty():
		var current: Dictionary = _state.get("current", {}) if _state.get("current") is Dictionary else {}
		var counts: Dictionary = current.get("counts", {})
		var position: Dictionary = _state.get("position", {})
		lines.append("%s · %s · %s" % [TITLES[_kick], _side, _phase()])
		lines.append("Set %d / %d · Rep %d / 4" % [int(position.get("set") if position.get("set") != null else 0), _sets, int(counts.get("done", 0))])
		var timer: Variant = current.get("timer")
		if timer is Dictionary and timer.get("duration_ms") != null:
			lines.append("%d s remaining" % maxi(0, ceili(float(timer["duration_ms"] - timer["elapsed_ms"]) / 1000.0)))
		if _phase() in ["active", "resting"]:
			var rep: Variant = _state.get("last_rep")
			if rep is Dictionary:
				for measurement in rep.get("measurements", []):
					if str(measurement.get("metric", "")).get_slice("@", 0) == "kick.peak_ankle_height":
						lines.append("Last foot height · " + _metric(measurement))
						break
		if _phase() in ["preparing", "ready"]:
			for gate in _state.get("readiness", []):
				if gate.get("state") == "fail" and gate.get("gate") != "safety_ack":
					for reason in gate.get("reasons", []):
						lines.append(str(reason.get("message", "")))
			for note in _definition.get("safety", {}).get("notes", []):
				lines.append(_text(str(note)))
			lines.append("Check your floor, cables and surroundings before starting.")
			if _use_targets and _phase() == "ready":
				lines.append("Hold the right controller where you want the ring.")
				lines.append("Use left Y to confirm. Practise in clear air.")
	if not _cue.is_empty():
		lines.append("Focus · " + _cue)
	if _target_reaches > 0:
		lines.append("Targets reached · %d" % _target_reaches)
	if not _reference.is_empty() and _ghost.visible:
		lines.append(_reference)
	if _review.get("second") is Dictionary:
		lines.append("FIRST COMPLETED ATTEMPT · SET %d / SET %d" % [
			int(_review["first"]["set_index"]), int(_review["second"]["set_index"])
		])
		for row in _review.get("measurements", []):
			var metric := str(row.get("metric", ""))
			lines.append("%s  %s / %s" % [METRICS.get(metric, metric), _metric(row.get("first")), _metric(row.get("second"))])
			if not bool(row.get("comparable", false)):
				lines.append("  Comparison unavailable")
	if not _notice.is_empty():
		lines.append(_notice)
	lines.append("")
	var menu := _menu()
	for i in range(menu.size()):
		lines.append(("›  " if i == mini(_choice, menu.size() - 1) else "     ") + menu[i])
	lines.append("A/X · next    B/Y · select" if xr != null else "Tab · next    Enter · select")
	_label.text = "\n".join(lines)
