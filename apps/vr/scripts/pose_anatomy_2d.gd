extends RefCounted
## A camera-plane silhouette built around observed joint centers.
const SHOULDERS := ["left_shoulder", "right_shoulder"]
const HIPS := ["left_hip", "right_hip"]
const TORSO := ["left_shoulder", "right_shoulder", "left_hip", "right_hip"]
const UPPER_ARM := [0.137, 0.135, 0.112, 0.085]
const FOREARM := [0.087, 0.105, 0.077, 0.055]
const THIGH := [0.180, 0.183, 0.142, 0.105]
const CALF := [0.106, 0.142, 0.091, 0.062]
const PALM := [0.061, 0.080, 0.074, 0.042]
const NECK := [0.110, 0.092, 0.078, 0.083]

const BONES := [
	["left_shoulder", "right_shoulder", Color("#c29aff")],
	["left_shoulder", "left_hip", Color("#66dbe7")], ["right_shoulder", "right_hip", Color("#ffda6b")],
	["left_hip", "right_hip", Color("#d78aff")],
	["left_shoulder", "left_elbow", Color("#ff655e")], ["left_elbow", "left_wrist", Color("#8fda50")],
	["right_shoulder", "right_elbow", Color("#ffda6b")], ["right_elbow", "right_wrist", Color("#64dce9")],
	["left_hip", "left_knee", Color("#ff655e")], ["left_knee", "left_ankle", Color("#64afff")],
	["right_hip", "right_knee", Color("#64dce9")], ["right_knee", "right_ankle", Color("#93de59")],
]
const JOINT_INK := Color("#f9faff")

static func skeleton_edges(points: Dictionary) -> Array:
	var observed: Array = []
	for bone in BONES:
		if points.has_all([bone[0], bone[1]]):
			observed.append(bone)
	return observed

static func torso_length(joints: Dictionary) -> float:
	if not joints.has_all(TORSO):
		return 0.0
	var shoulders: Vector2 = (joints[SHOULDERS[0]] + joints[SHOULDERS[1]]) * 0.5
	var hips: Vector2 = (joints[HIPS[0]] + joints[HIPS[1]]) * 0.5
	return shoulders.distance_to(hips)

static func draw(canvas: Control, points: Dictionary, unit: float) -> void:
	var base := Color("#10161e")
	var light := Color("#17212b")
	var shade := Color("#090e15")
	for side in ["right_", "left_"]:
		_limb(canvas, points, side + "hip", side + "knee", THIGH, unit, light, shade)
		_limb(canvas, points, side + "knee", side + "ankle", CALF, unit, light, shade)
		if points.has(side + "ankle"):
			# An ankle supports a small rounded end shape. No toe direction is assigned.
			_ellipse(canvas, points[side + "ankle"], Vector2(0.094, 0.082) * unit, Color(base, 0.72), Color(light, 0.72))
	var head: Vector2 = points.get("head", Vector2.ZERO)
	if points.has("head") and points.has_all(SHOULDERS):
		var shoulders: Vector2 = (points[SHOULDERS[0]] + points[SHOULDERS[1]]) * 0.5
		var up := shoulders.direction_to(head)
		head += up * unit * 0.14
		var jaw := head - up * unit * 0.15
		_surface(canvas, shoulders, jaw, NECK, unit, light, shade, false)
	if points.has_all(TORSO):
		_draw_torso(canvas, points, unit, light, shade)
	for side in ["right_", "left_"]:
		var shoulder: String = side + "shoulder"
		var elbow: String = side + "elbow"
		var wrist: String = side + "wrist"
		_limb(canvas, points, shoulder, elbow, UPPER_ARM, unit, light, shade)
		_limb(canvas, points, elbow, wrist, FOREARM, unit, light, shade)
		if points.has_all([elbow, wrist]):
			var hand: Vector2 = points[wrist]
			var direction: Vector2 = (hand - points[elbow]).normalized()
			_surface(canvas, hand, hand + direction * unit * 0.13, PALM, unit, Color(light, 0.72), Color(shade, 0.72))
	if points.has("head"):
		_ellipse(canvas, head, Vector2(0.173, 0.220) * unit, base, light)

	_draw_skeleton(canvas, points, unit)

static func _draw_skeleton(canvas: Control, points: Dictionary, unit: float) -> void:
	var bone_width := clampf(unit * 0.044, 2.5, 7.0)
	var joint_radius := clampf(unit * 0.041, 3.0, 6.0)
	for edge in skeleton_edges(points):
		_draw_bone(canvas, points[edge[0]], points[edge[1]], edge[2], bone_width)
	if points.has("head") and points.has_all(SHOULDERS):
		var shoulder_center: Vector2 = (points[SHOULDERS[0]] + points[SHOULDERS[1]]) * 0.5
		_draw_bone(canvas, points["head"], shoulder_center, Color("#93de59"), bone_width)
	for position: Vector2 in points.values():
		canvas.draw_circle(position, joint_radius + 1.5, Color("#0b1016"), true, -1, true)
		canvas.draw_circle(position, joint_radius, JOINT_INK, true, -1, true)

static func _draw_bone(canvas: Control, from: Vector2, to: Vector2, color: Color, width: float) -> void:
	canvas.draw_line(from, to, Color("#0b1016"), width + 2.5, true)
	canvas.draw_line(from, to, color, width, true)

static func _draw_torso(canvas: Control, points: Dictionary, unit: float, light: Color, shade: Color) -> void:
	var left_shoulder: Vector2 = points[SHOULDERS[0]]
	var right_shoulder: Vector2 = points[SHOULDERS[1]]
	var left_hip: Vector2 = points[HIPS[0]]
	var right_hip: Vector2 = points[HIPS[1]]
	var top := (left_shoulder + right_shoulder) * 0.5
	var bottom := (left_hip + right_hip) * 0.5
	var direction := top.direction_to(bottom)
	var axis := direction.orthogonal()
	# Project onto one axis. Crossed or edge-on landmarks cannot invert the contour.
	var chest := absf((left_shoulder - right_shoulder).dot(axis)) * 0.5
	var pelvis := absf((left_hip - right_hip).dot(axis)) * 0.5
	chest = clampf(chest, unit * 0.095, unit * 0.64)
	pelvis = clampf(pelvis, unit * 0.075, unit * 0.53)
	var waist := minf(chest * 0.77, pelvis * 0.96)
	_surface(canvas, top - direction * unit * 0.09, bottom + direction * unit * 0.14,
		[chest * 0.74 / unit, chest * 1.06 / unit, waist / unit, pelvis * 1.05 / unit, pelvis * 0.55 / unit], unit, light, shade, false)

static func _limb(canvas: Control, points: Dictionary, from: String, to: String, profile: Array, unit: float, light: Color, shade: Color) -> void:
	if points.has_all([from, to]):
		_surface(canvas, points[from], points[to], profile, unit, light, shade)

static func _surface(canvas: Control, from: Vector2, to: Vector2, profile: Array, unit: float, light: Color, shade: Color, rounded := true) -> void:
	var across := from.direction_to(to).orthogonal()
	if across.is_zero_approx():
		return
	const SLICES := 12
	var vertices := PackedVector2Array()
	var colors := PackedColorArray()
	var triangles := PackedInt32Array()
	var left_edge := PackedVector2Array()
	var right_edge := PackedVector2Array()
	for i in range(SLICES + 1):
		var t := float(i) / SLICES
		var station := t * (profile.size() - 1)
		var index := mini(floori(station), profile.size() - 2)
		var blend := smoothstep(0.0, 1.0, station - index)
		var width := lerpf(profile[index], profile[index + 1], blend) * unit
		var center := from.lerp(to, t)
		var left := center - across * width
		var right := center + across * width
		vertices.append_array(PackedVector2Array([left, center - across * width * 0.20, right]))
		colors.append_array(PackedColorArray([shade, light, shade]))
		left_edge.append(left)
		right_edge.append(right)
		if i < SLICES:
			var a := i * 3
			triangles.append_array(PackedInt32Array([a, a + 1, a + 3, a + 1, a + 4, a + 3,
				a + 1, a + 2, a + 4, a + 2, a + 5, a + 4]))
		if rounded and i in [0, SLICES]:
			_ellipse(canvas, center, Vector2.ONE * width, shade, light)
	RenderingServer.canvas_item_add_triangle_array(canvas.get_canvas_item(), triangles, vertices, colors)
	canvas.draw_polyline(left_edge, shade, 1.15, true)
	canvas.draw_polyline(right_edge, shade, 1.15, true)

static func _ellipse(canvas: Control, center: Vector2, radii: Vector2, edge: Color, light: Color) -> void:
	const SIDES := 24
	var vertices := PackedVector2Array([center - radii * Vector2(0.27, 0.22)])
	var colors := PackedColorArray([light])
	var triangles := PackedInt32Array()
	for i in range(SIDES):
		var angle := TAU * i / SIDES
		vertices.append(center + Vector2(cos(angle), sin(angle)) * radii)
		colors.append(edge)
		triangles.append_array(PackedInt32Array([0, i + 1, (i + 1) % SIDES + 1]))
	RenderingServer.canvas_item_add_triangle_array(canvas.get_canvas_item(), triangles, vertices, colors)
	vertices.remove_at(0)
	vertices.append(vertices[0])
	canvas.draw_polyline(vertices, edge, 1.15, true)
