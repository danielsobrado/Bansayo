extends Node3D
const PoseBuffer = preload("res://scripts/pose_buffer.gd")
const RADIUS_M := 0.022
var _segments: Array[Node3D] = []
var _meshes: Array[MeshInstance3D] = []
var _materials: Dictionary = {}
var revision := -1

func _ready() -> void:
	_materials["measured"] = _material(Color(0.94, 0.95, 0.96), false)
	_materials["derived"] = _material(Color(0.30, 0.79, 0.83), false)
	_materials["inferred"] = _material(Color(0.69, 0.49, 0.90, 0.35), true)
	_materials["unavailable"] = _material(Color(0.3, 0.33, 0.36, 0.12), true)

func _material(color: Color, transparent: bool) -> StandardMaterial3D:
	var material := StandardMaterial3D.new()
	material.albedo_color = color
	material.roughness = 0.9
	if transparent:
		material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	return material

func build(definition: Dictionary) -> void:
	for segment in _segments:
		segment.queue_free()
	_segments.clear()
	_meshes.clear()
	revision = int(definition.get("rev", -1))
	for item in definition.get("segments", []):
		var segment := Node3D.new()
		segment.name = str(item.get("id", "segment"))
		add_child(segment)
		_segments.append(segment)
		var tail := PoseBuffer.vector(item.get("tail_m", [0, 0, 0]))
		var length := tail.length()
		var bone := MeshInstance3D.new()
		var capsule := CapsuleMesh.new()
		capsule.radius = RADIUS_M
		capsule.height = maxf(length + 2 * RADIUS_M, 2 * RADIUS_M)
		bone.mesh = capsule
		bone.position = tail * 0.5
		if length > 0.0001:
			bone.quaternion = Quaternion(Vector3.UP, tail.normalized())
		bone.material_override = _materials["inferred"]
		segment.add_child(bone)
		_meshes.append(bone)
	visible = false

func apply_pose(pose: Dictionary) -> void:
	if pose.is_empty() or int(pose.get("skeleton_rev", -1)) != revision:
		visible = false
		return
	visible = true
	var segments: Array = pose.get("segments", [])
	for i in range(mini(_segments.size(), segments.size())):
		var item: Dictionary = segments[i]
		if not item.get("position_m") is Array or not item.get("rotation") is Array:
			continue
		_segments[i].position = PoseBuffer.vector(item["position_m"])
		_segments[i].quaternion = PoseBuffer.quaternion(item["rotation"])
		var provenance := str(item.get("provenance", "inferred"))
		_meshes[i].material_override = _materials.get(provenance, _materials["inferred"])
		_segments[i].visible = provenance != "unavailable"
