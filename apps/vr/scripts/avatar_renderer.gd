extends Node3D
const PoseBuffer = preload("res://scripts/pose_buffer.gd")
const RADIUS_M := 0.022
# Width, depth and amount of overlap at the joints, as fractions of the segment length.
const VOLUMES := {
	"pelvis": Vector3(2.80, 1.60, 1.70),
	"spine": Vector3(1.25, 0.84, 1.55),
	"chest": Vector3(1.70, 1.02, 1.45),
	"neck": Vector3(0.95, 0.92, 1.4),
	"head": Vector3(0.74, 0.80, 1.0),
	"clavicle": Vector3(0.64, 0.70, 1.34),
	"upper_arm": Vector3(0.35, 0.36, 1.22),
	"forearm": Vector3(0.32, 0.36, 1.26),
	"hand": Vector3(0.44, 0.26, 1.06),
	"thigh": Vector3(0.39, 0.44, 1.26),
	"shin": Vector3(0.27, 0.32, 1.20),
	"foot": Vector3(0.50, 0.39, 1.16),
}
var _segments: Array[Node3D] = []
var _meshes: Array[MeshInstance3D] = []
var _materials: Dictionary = {}
var _tails: Array[Vector3] = []
var _types: Array[String] = []
var _sphere := SphereMesh.new()
var _definition: Dictionary = {}
var _mannequin := false
var revision := -1

func _ready() -> void:
	_sphere.radius = 0.5
	_sphere.height = 1.0
	_sphere.radial_segments = 16
	_sphere.rings = 8
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

func set_mannequin(enabled: bool) -> void:
	if enabled == _mannequin:
		return
	_mannequin = enabled
	if not _definition.is_empty():
		build(_definition)

func build(definition: Dictionary) -> void:
	_definition = definition
	for segment in _segments:
		remove_child(segment)
		segment.queue_free()
	_segments.clear()
	_meshes.clear()
	_tails.clear()
	_types.clear()
	revision = int(definition.get("rev", -1))
	for item in definition.get("segments", []):
		var segment := Node3D.new()
		segment.name = str(item.get("id", "segment"))
		add_child(segment)
		_segments.append(segment)
		var tail := PoseBuffer.vector(item.get("tail_m", [0, 0, 0]))
		var length := tail.length()
		var bone := MeshInstance3D.new()
		var kind := str(item.get("id", "")).trim_prefix("left_").trim_prefix("right_")
		_types.append(kind)
		_tails.append(tail)
		if _mannequin:
			bone.mesh = _sphere
		else:
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
	for i in range(_segments.size()):
		var bone := _segments[i]
		if i >= segments.size():
			bone.hide()
			continue
		var item: Dictionary = segments[i]
		var provenance := str(item.get("provenance", "inferred"))
		if not item.get("position_m") is Array or not item.get("rotation") is Array or provenance == "unavailable":
			bone.hide()
			continue
		bone.position = PoseBuffer.vector(item["position_m"])
		bone.quaternion = PoseBuffer.quaternion(item["rotation"])
		var mesh := _meshes[i]
		mesh.material_override = _materials.get(provenance, _materials["inferred"])
		if _mannequin:
			var tail := _tails[i]
			var kind := _types[i]
			# An inferred extremity can indicate its origin without implying an observed orientation.
			var unoriented := provenance not in ["measured", "derived"] and kind in ["foot", "hand"]
			var dimensions: Vector3 = VOLUMES.get(kind, Vector3(0.35, 0.35, 1.2))
			mesh.position = Vector3.ZERO if unoriented else tail * 0.5
			mesh.scale = Vector3.ONE * 0.06 if unoriented else Vector3(dimensions.x, dimensions.z, dimensions.y) * maxf(0.02, tail.length())
		bone.show()
