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
# Cross-section profiles from segment origin to tip. Torso and limb surfaces
# overlap at the joint without changing the engine's segment transforms.
const PROFILES := {
	"pelvis": [0.30, 0.76, 0.97, 1.0, 0.93, 0.76, 0.30],
	"spine": [0.47, 0.79, 0.90, 0.87, 0.94, 0.83, 0.40],
	"chest": [0.34, 0.69, 0.86, 0.97, 1.0, 0.82, 0.24],
	"neck": [0.38, 0.81, 0.84, 0.69, 0.73, 0.72, 0.34],
	"head": [0.20, 0.52, 0.72, 0.90, 1.0, 0.84, 0.28],
	"clavicle": [0.43, 0.75, 0.79, 0.84, 0.91, 0.71, 0.35],
	"upper_arm": [0.36, 0.85, 1.0, 0.91, 0.72, 0.61, 0.31],
	"forearm": [0.32, 0.68, 0.96, 0.81, 0.63, 0.50, 0.27],
	"hand": [0.33, 0.74, 0.99, 1.0, 0.83, 0.64, 0.24],
	"thigh": [0.35, 0.86, 1.0, 0.96, 0.78, 0.59, 0.26],
	"shin": [0.38, 0.66, 1.0, 0.94, 0.66, 0.46, 0.24],
	"foot": [0.30, 0.66, 0.84, 0.83, 0.98, 1.0, 0.37],
}
var _anatomy := {}
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
	_materials["measured"] = _material(Color(0.84, 0.87, 0.91), false)
	_materials["derived"] = _material(Color(0.38, 0.67, 0.72), false)
	_materials["inferred"] = _material(Color(0.69, 0.49, 0.90, 0.35), true)
	_materials["unavailable"] = _material(Color(0.3, 0.33, 0.36, 0.12), true)

func _material(color: Color, transparent: bool) -> StandardMaterial3D:
	var material := StandardMaterial3D.new()
	material.albedo_color = color
	material.roughness = 0.66
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
			bone.mesh = _anatomical_mesh(kind)
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

func _anatomical_mesh(kind: String) -> Mesh:
	if _anatomy.has(kind):
		return _anatomy[kind]
	var profile: Array = PROFILES.get(kind, PROFILES["forearm"]).duplicate()
	profile.push_front(0.001)
	profile.push_back(0.001)
	const SIDES := 16
	const RINGS := 25
	var surface := SurfaceTool.new()
	surface.begin(Mesh.PRIMITIVE_TRIANGLES)
	for ring in range(RINGS):
		var t := float(ring) / (RINGS - 1)
		var sample := t * (profile.size() - 1)
		var at := mini(floori(sample), profile.size() - 2)
		var radius := lerpf(profile[at], profile[at + 1], smoothstep(0.0, 1.0, sample - at)) * 0.5
		for i in range(SIDES):
			var angle := TAU * i / SIDES
			var depth := sin(angle) * radius
			# A shallower anterior curve gives torso pieces a broad chest and back.
			if kind in ["chest", "spine", "pelvis"]:
				depth *= 0.86 + 0.14 * absf(sin(angle))
			surface.add_vertex(Vector3(cos(angle) * radius, t - 0.5, depth))
	for ring in range(RINGS - 1):
		for i in range(SIDES):
			var a := ring * SIDES + i
			var b := ring * SIDES + (i + 1) % SIDES
			for index in [a, b + SIDES, a + SIDES, a, b, b + SIDES]:
				surface.add_index(index)
	surface.generate_normals()
	var mesh := surface.commit()
	_anatomy[kind] = mesh
	return mesh

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
			mesh.mesh = _sphere if unoriented else _anatomical_mesh(kind)
			mesh.position = Vector3.ZERO if unoriented else tail * 0.5
			mesh.scale = Vector3.ONE * 0.06 if unoriented else Vector3(dimensions.x, dimensions.z, dimensions.y) * maxf(0.02, tail.length())
		bone.show()
