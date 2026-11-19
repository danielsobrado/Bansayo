extends Node3D
## Routes the same spatial controls from desktop and OpenXR aim poses.
const SpatialPanel = preload("res://scripts/spatial_panel.gd")
const ClinicTarget = preload("res://scripts/clinic_target.gd")
var panel: SpatialPanel
var focused := false
var audio_enabled := true
var _hands: Array[XRController3D] = []
var _beams: Array[MeshInstance3D] = []
var _drag_hand: XRController3D
var _drag_offset := Transform3D.IDENTITY
var _drag_panel: SpatialPanel
var _sound := AudioStreamPlayer.new()

func _ready() -> void:
	_sound.stream = ClinicTarget.chime()
	_sound.volume_db = -23.0
	add_child(_sound)

func add_hand(hand: XRController3D) -> void:
	var beam := MeshInstance3D.new()
	var mesh := BoxMesh.new()
	mesh.size = Vector3(0.003, 0.003, 1.0)
	beam.mesh = mesh
	var material := StandardMaterial3D.new()
	material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	material.albedo_color = SpatialPanel.ACCENT
	beam.material_override = material
	beam.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	beam.hide()
	hand.add_child(beam)
	_hands.append(hand)
	_beams.append(beam)
	hand.button_released.connect(func(action: String):
		if action == "grip_click" and _drag_hand == hand:
			_drag_hand = null
	)

func set_panel(next: SpatialPanel) -> void:
	if next == panel:
		return
	if panel:
		panel.hover_at(SpatialPanel.NO_HIT)
	panel = next
	_drag_hand = null

func _tracked(hand: XRController3D) -> bool:
	return focused and hand.get_has_tracking_data()

func _hand_pixel(hand: XRController3D) -> Vector2:
	if panel and _tracked(hand):
		return panel.intersect_ray(hand.global_position, -hand.global_basis.z)
	return SpatialPanel.NO_HIT

func handle_button(action: String, hand: XRController3D, blocked_label := "") -> void:
	var pixel := _hand_pixel(hand)
	if pixel == SpatialPanel.NO_HIT:
		return
	match action:
		"grip_click":
			_drag_hand = hand
			_drag_panel = panel
			_drag_offset = hand.global_transform.affine_inverse() * panel.global_transform
		"trigger_click":
			if _drag_hand or (not blocked_label.is_empty() and panel.label_at(pixel) == blocked_label):
				return
			if panel.press_at(pixel):
				hand.trigger_haptic_pulse("haptic", 0.0, 0.25, 0.04, 0.0)
				if audio_enabled:
					_sound.play()

func desktop_input(event: InputEvent, camera: Camera3D) -> bool:
	if not panel or not event is InputEventMouse:
		return false
	var pixel := panel.intersect_ray(camera.project_ray_origin(event.position), camera.project_ray_normal(event.position))
	panel.hover_at(pixel)
	if pixel == SpatialPanel.NO_HIT or not event is InputEventMouseButton or not event.pressed:
		return false
	match event.button_index:
		MOUSE_BUTTON_LEFT: return panel.press_at(pixel)
		MOUSE_BUTTON_WHEEL_UP: panel.scroll_at(pixel, -96)
		MOUSE_BUTTON_WHEEL_DOWN: panel.scroll_at(pixel, 96)
		_: return false
	return true

func _process(delta: float) -> void:
	if _drag_hand:
		if _drag_panel == panel and panel.is_visible_in_tree() and _tracked(_drag_hand):
			panel.global_transform = _drag_hand.global_transform * _drag_offset
		else:
			_drag_hand = null
	var hover := SpatialPanel.NO_HIT
	for i in range(_hands.size()):
		var hand := _hands[i]
		var beam := _beams[i]
		var pixel := _hand_pixel(hand)
		beam.visible = pixel != SpatialPanel.NO_HIT
		if beam.visible:
			var distance := hand.global_position.distance_to(panel.world_point(pixel))
			beam.position.z = -distance * 0.5
			beam.scale.z = distance
			hover = pixel
			var stick := hand.get_vector2("primary").y
			if absf(stick) > 0.25:
				panel.scroll_at(pixel, roundi(-stick * delta * 720))
	if panel and not _hands.is_empty() and focused:
		panel.hover_at(hover)
