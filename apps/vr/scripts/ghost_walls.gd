extends Node3D
signal controls_changed

const GLASS = preload("res://scripts/ghost_wall.gdshader")
const ClinicTarget = preload("res://scripts/clinic_target.gd")
const PATTERN := [["left", "right", "duck", "right", "left"], ["duck", "left", "right", "duck", "left"]]
const CUES := {"left": "←   LEAN LEFT", "right": "LEAN RIGHT   →", "duck": "↓   DUCK"}
const TINT := {"left": Color(0.31, 0.92, 1.0), "right": Color(0.70, 0.57, 1.0), "duck": Color(1.0, 0.72, 0.35)}
const TRAVEL_S := 2.35
const SLOT_S := 3.25
const REST_S := 12.0
const ALLOWED_SHIFT := 0.32
const SIDE_EDGE := 0.08
const DUCK_EDGE := -0.055
const CLEARANCE := 0.08

var audio_enabled := true
var opened := false
var controls_requested := false
var _control_status := ""
var _control_details: Array[String] = []
var phase := "ready"
var cleared := 0
var assessed := 0
var _camera: Camera3D
var native := false
var _round := 0
var _wall_index := 0
var _choice := 0
var _clock := 0.0
var _rest_left := 0.0
var _wall_clock := 0.0
var _old_depth := INF
var _old_head := Vector3.ZERO
var _judged := false
var _after_countdown := "running"
var _resume_phase := "running"
var _notice := ""
var _result := ""
var _wall := Node3D.new()
var _panels: Array[MeshInstance3D] = []
var _materials: Array[ShaderMaterial] = []
var _cue := Label3D.new()
var _dashboard := Label3D.new()
var _sound := AudioStreamPlayer3D.new()

func _ready() -> void:
	add_child(_wall)
	for offset in [0.0, -0.065]:
		var panel := MeshInstance3D.new()
		panel.position.z = offset
		panel.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
		panel.mesh = QuadMesh.new()
		var material := ShaderMaterial.new()
		material.shader = GLASS
		material.set_shader_parameter("layer", 1.0 if offset == 0.0 else 0.25)
		panel.material_override = material
		_wall.add_child(panel)
		_panels.append(panel)
		_materials.append(material)
	_sound.stream = ClinicTarget.chime()
	_sound.volume_db = -18.0
	_wall.add_child(_sound)
	_configure_label(_cue, Vector3(0.0, 0.48, -2.55), 46, 0.0020)
	_configure_label(_dashboard, Vector3(0.0, -0.02, -2.55), 30, 0.0016)
	hide()

func _configure_label(label: Label3D, at: Vector3, font: int, pixel: float) -> void:
	label.position = at
	label.font_size = font
	label.pixel_size = pixel
	label.outline_size = 6
	label.no_depth_test = true
	add_child(label)

func open(camera: Camera3D, in_headset: bool) -> void:
	_camera = camera
	native = in_headset
	opened = true
	show()
	_reset()
	_anchor()
	_refresh()

func _anchor() -> void:
	var forward := -_camera.global_basis.z
	forward.y = 0.0
	if forward.length_squared() < 0.01:
		forward = Vector3.FORWARD
	global_transform = Transform3D(Basis.looking_at(forward.normalized(), Vector3.UP), _camera.global_position)

func _reset() -> void:
	controls_requested = false
	phase = "ready"
	cleared = 0
	assessed = 0
	_round = 0
	_wall_index = 0
	_choice = 0
	_notice = ""
	_result = ""
	_judged = false
	_wall.hide()

func _menu() -> Array[String]:
	match phase:
		"ready": return ["Confirm clear area & start", "Back to clinic"]
		"rest": return ["Start next round", "Pause", "Back to clinic"]
		"paused": return ["Confirm clear area & resume", "Back to clinic"]
		"complete": return ["Practise again", "Back to clinic"]
	return ["Pause", "Back to clinic"]

func next() -> void:
	_choice = (_choice + 1) % _menu().size()
	_refresh()

func select(space_ok: bool) -> void:
	match _menu()[_choice]:
		"Back to clinic":
			opened = false
			_sound.stop()
			hide()
		"Pause":
			interrupt("Paused")
		"Practise again":
			_reset()
			_anchor()
		"Confirm clear area & start":
			if space_ok:
				_anchor()
				_countdown("running")
			else:
				_notice = "A clear area and headset tracking are required."
		"Confirm clear area & resume", "Start next round":
			if space_ok and _inside(to_local(_camera.global_position)):
				_countdown(_resume_phase if phase == "paused" else "running")
			else:
				_notice = "Return to the starting spot and confirm the clear area."
	_refresh()

func _countdown(next_phase: String) -> void:
	controls_requested = false
	phase = "countdown"
	_clock = 3.0
	_choice = 0
	_after_countdown = next_phase
	_notice = ""

func interrupt(message: String) -> void:
	if not opened or phase not in ["countdown", "running", "rest"]:
		return
	_resume_phase = _after_countdown if phase == "countdown" else phase
	_wall.hide()
	_sound.stop()
	_notice = message
	phase = "paused"
	_choice = 0
	_refresh()

func _inside(head: Vector3) -> bool:
	return head.is_finite() and Vector2(head.x, head.z).length() <= ALLOWED_SHIFT and head.y >= -0.45 and head.y <= 0.24

static func clears(kind: String, head: Vector3) -> bool:
	match kind:
		"left": return head.x <= -SIDE_EDGE - CLEARANCE
		"right": return head.x >= SIDE_EDGE + CLEARANCE
	return head.y <= DUCK_EDGE - CLEARANCE

static func wall_depth(seconds: float) -> float:
	return lerpf(-3.9, 0.0, seconds / TRAVEL_S)

func step(delta: float, space_ok: bool, preview_offset := Vector3.ZERO) -> void:
	if not opened or phase not in ["countdown", "running", "rest"]:
		return
	var head := to_local(_camera.global_position) + preview_offset
	if not space_ok or not _inside(head):
		interrupt("Practice paused · return to the starting spot.")
		return
	if delta > 0.20:
		interrupt("Tracking paused · confirm the clear area to resume.")
		return
	match phase:
		"countdown":
			_clock -= delta
			if _clock <= 0:
				phase = _after_countdown
				_choice = 0
				if phase == "running":
					if _judged:
						_finish_wall()
					else:
						_start_wall()
		"rest":
			_rest_left -= delta
			if _rest_left <= 0:
				_countdown("running")
		"running":
			_wall_clock += delta
			_draw_wall(head)
			if _wall_clock >= SLOT_S:
				_finish_wall()
	_refresh()

func _kind() -> String:
	return PATTERN[_round][_wall_index]

func _start_wall() -> void:
	var kind := _kind()
	var side := 1.0 if kind == "left" else -1.0
	var size := Vector2(1.6, 1.75) if kind != "duck" else Vector2(2.65, 0.86)
	_wall.position = Vector3(side * (size.x * 0.5 - SIDE_EDGE), -0.18, wall_depth(0.0))
	if kind == "duck":
		_wall.position = Vector3(0, size.y * 0.5 + DUCK_EDGE, wall_depth(0.0))
	for panel in _panels:
		(panel.mesh as QuadMesh).size = size
	for material in _materials:
		material.set_shader_parameter("energy", TINT[kind])
		material.set_shader_parameter("opacity", 0.80)
	_wall_clock = 0
	_old_depth = INF
	_judged = false
	_result = ""
	_wall.show()
	_sound.pitch_scale = 0.65
	if audio_enabled:
		_sound.play()

func _draw_wall(head: Vector3) -> void:
	_wall.position.z = wall_depth(_wall_clock)
	var depth := head.z - _wall.position.z
	var fade := smoothstep(0.28, 0.85, depth)
	_wall.visible = fade > 0.0
	for material in _materials:
		material.set_shader_parameter("opacity", fade)
	if not _judged and depth <= 0.0 and _old_depth > 0.0 and is_finite(_old_depth):
		var crossing := _old_head.lerp(head, _old_depth / (_old_depth - depth))
		_judged = true
		assessed += 1
		if clears(_kind(), crossing):
			cleared += 1
			_result = "CLEAR"
			_sound.pitch_scale = 1.25
			if audio_enabled:
				_sound.play()
		else:
			_result = "CLIPPED"
	_old_head = head
	_old_depth = depth

func _finish_wall() -> void:
	_wall.hide()
	_wall_index += 1
	if _wall_index < PATTERN[_round].size():
		_start_wall()
	elif _round + 1 < PATTERN.size():
		_round += 1
		_wall_index = 0
		_judged = false
		phase = "rest"
		_rest_left = REST_S
		_choice = 0
	else:
		phase = "complete"
		_choice = 0

func refresh_controls() -> void:
	_refresh()

func _refresh() -> void:
	var cue := "GHOST WALLS"
	var color := Color(0.85, 0.95, 1.0)
	var guide := Vector2.ZERO
	var lines: Array[String] = []
	match phase:
		"ready": lines.append("Keep your feet planted. Lean or duck comfortably.")
		"countdown": cue = "READY   %d" % ceili(_clock)
		"running":
			var kind := _kind()
			cue = CUES[kind]
			color = TINT[kind]
			if not _result.is_empty():
				cue = _result
				color = Color(0.39, 1.0, 0.74) if _result == "CLEAR" else Color(1.0, 0.77, 0.44)
			match kind:
				"left": guide.x = -0.69
				"right": guide.x = 0.69
				"duck": guide.y = -0.69
			lines.append("Round %d / %d   ·   Wall %d / %d" % [_round + 1, PATTERN.size(), _wall_index + 1, PATTERN[_round].size()])
		"rest":
			cue = "REST   %d s" % ceili(_rest_left)
			lines.append("Round %d next" % (_round + 1))
		"complete": cue = "DRILL COMPLETE"
		"paused": cue = "PAUSED"
	if assessed > 0:
		lines.append("%d / %d cleared" % [cleared, assessed])
	if not _notice.is_empty():
		lines.append(_notice)
	_control_status = cue
	_control_details = lines
	controls_changed.emit()
	if _cue.text != cue:
		_cue.text = cue
	var show_cue := phase in ["running", "countdown"] and not controls_requested
	_cue.visible = show_cue
	_dashboard.visible = show_cue
	_cue.modulate = color
	_cue.position.x = guide.x
	_cue.position.y = 0.48 + guide.y
	_dashboard.position.x = guide.x
	_dashboard.position.y = -0.02 + guide.y
	var text := "\n".join(PackedStringArray(lines)) if phase in ["running", "countdown"] else ""
	if _dashboard.text != text:
		_dashboard.text = text

func control_status() -> String:
	return _control_status

func control_details() -> Array[String]:
	return _control_details
