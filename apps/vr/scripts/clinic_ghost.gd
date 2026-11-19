extends Node3D
# Recorded reference on its own display stage, independent of the athlete's physical position.
const AvatarRenderer = preload("res://scripts/avatar_renderer.gd")
const PoseBuffer = preload("res://scripts/pose_buffer.gd")
var _body := AvatarRenderer.new()
var _path := MeshInstance3D.new()
var _marker := MeshInstance3D.new()
var _clip: Dictionary = {}
var _root := Vector3.ZERO
const SPEEDS := [0.5, 0.25, 1.0]
var _playhead_us := 0.0
var _speed_index := 0
var paused := false
var _index := 0

func _ready() -> void:
	add_child(_body)
	add_child(_path)
	add_child(_marker)
	var sphere := SphereMesh.new()
	sphere.radius = 0.045
	sphere.height = 0.09
	_marker.mesh = sphere
	var material := StandardMaterial3D.new()
	material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	material.albedo_color = Color(0.45, 1.0, 0.75)
	_marker.material_override = material
	_path.material_override = material
	visible = false

func build(skeleton: Dictionary) -> void:
	_body.build(skeleton)
	clear()

func clear() -> void:
	_clip = {}
	visible = false

func show_clip(clip: Dictionary, height_available: bool) -> bool:
	var samples: Array = clip.get("samples", [])
	if samples.size() < 2 or int(clip.get("skeleton_rev", -1)) != _body.revision:
		return false
	_clip = clip
	_root = PoseBuffer.vector(samples[0]["root_m"])
	_root.y = 0.0
	_playhead_us = 0.0
	_index = 0
	_speed_index = 0
	paused = false
	var points: Array = []
	for path in clip.get("paths", []):
		points = path.get("points_m", [])
		break
	_path.mesh = null
	_marker.hide()
	if height_available and points.size() > 1:
		var path_mesh := ImmediateMesh.new()
		var peak := Vector3(0, -INF, 0)
		path_mesh.surface_begin(Mesh.PRIMITIVE_LINE_STRIP)
		for point in points:
			var p := PoseBuffer.vector(point) - _root
			path_mesh.surface_add_vertex(p)
			if p.y > peak.y:
				peak = p
		path_mesh.surface_end()
		_path.mesh = path_mesh
		_marker.position = peak
		_marker.show()
	show()
	return true

func speed() -> float:
	return SPEEDS[_speed_index]

func cycle_speed() -> void:
	_speed_index = (_speed_index + 1) % SPEEDS.size()

func restart() -> void:
	_playhead_us = 0.0

func _process(delta: float) -> void:
	if _clip.is_empty() or not visible:
		return
	var frames: Array = _clip["samples"]
	var start := int(frames[0]["t_us"])
	var duration := maxi(1, int(frames.back()["t_us"]) - start)
	if not paused:
		_playhead_us = fposmod(_playhead_us + delta * speed() * 1000000.0, float(duration))
	var at := start + int(_playhead_us)
	if at < int(frames[_index]["t_us"]):
		_index = 0
	while _index + 1 < frames.size() and int(frames[_index + 1]["t_us"]) <= at:
		_index += 1
	var frame: Dictionary = frames[_index]
	var segments: Array = []
	for i in range(frame["positions_m"].size()):
		var p := PoseBuffer.vector(frame["positions_m"][i]) - _root
		segments.append({"position_m": [p.x, p.y, p.z], "rotation": frame["rotations"][i], "provenance": "inferred"})
	_body.apply_pose({"skeleton_rev": _clip["skeleton_rev"], "segments": segments})
