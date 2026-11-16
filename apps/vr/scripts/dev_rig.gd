extends Node3D
const ORBIT_SENSITIVITY := 0.006
const ZOOM_STEP_M := 0.3
var camera := Camera3D.new()
var _yaw := 0.45
var _pitch := 0.12
var _distance := 4.5
var _target := Vector3(0, 0.9, 0)
var enabled := true

func _ready() -> void:
	add_child(camera)
	camera.fov = 45
	camera.current = true
	_apply_view()

func _unhandled_input(event: InputEvent) -> void:
	if not enabled:
		return
	if event is InputEventMouseMotion and Input.is_mouse_button_pressed(MOUSE_BUTTON_LEFT):
		_yaw -= event.relative.x * ORBIT_SENSITIVITY
		_pitch = clampf(_pitch + event.relative.y * ORBIT_SENSITIVITY, -1.2, 1.2)
		_apply_view()
	elif event is InputEventMouseButton and event.pressed:
		if event.button_index == MOUSE_BUTTON_WHEEL_UP:
			_distance = maxf(1.0, _distance - ZOOM_STEP_M)
		elif event.button_index == MOUSE_BUTTON_WHEEL_DOWN:
			_distance = minf(12.0, _distance + ZOOM_STEP_M)
		_apply_view()

func _apply_view() -> void:
	camera.position = _target + Vector3(sin(_yaw) * cos(_pitch), sin(_pitch), cos(_yaw) * cos(_pitch)) * _distance
	camera.look_at(_target)
