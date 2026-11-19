extends Control
## Two dimensionless camera-plane figures. Body volumes are drawn around visible joints.
const LIVE := Color("#74e6ed")
const GUIDE := Color("#c8b5ff")
const MUTED := Color("#b6c9d8")
const LIMBS := [
	["left_shoulder", "left_elbow", 0.16], ["left_elbow", "left_wrist", 0.13],
	["right_shoulder", "right_elbow", 0.16], ["right_elbow", "right_wrist", 0.13],
	["left_hip", "left_knee", 0.24], ["left_knee", "left_ankle", 0.17],
	["right_hip", "right_knee", 0.24], ["right_knee", "right_ankle", 0.17],
]
const TORSO := ["left_shoulder", "right_shoulder", "right_hip", "left_hip"]
var live_pose := {}
var guide_pose := {}
var live_caption := ""
var guide_caption := ""

func _ready() -> void:
	custom_minimum_size.y = 420
	mouse_filter = Control.MOUSE_FILTER_IGNORE

func present(live: Dictionary, guide: Dictionary, camera_text: String, guide_text: String) -> void:
	if live == live_pose and guide == guide_pose and camera_text == live_caption and guide_text == guide_caption:
		return
	live_pose = live
	guide_pose = guide
	live_caption = camera_text
	guide_caption = guide_text
	queue_redraw()

func _draw() -> void:
	var width := (size.x - 12) * 0.5
	_draw_card(Rect2(0, 0, width, size.y), "Your reflection", live_caption, live_pose, LIVE)
	_draw_card(Rect2(width + 12, 0, width, size.y), "Reference pose", guide_caption, guide_pose, GUIDE)

func _draw_card(card: Rect2, title: String, caption: String, joints: Dictionary, ink: Color) -> void:
	draw_rect(card, Color("#152734"))
	draw_rect(card, Color("#365163"), false, 2)
	var font := ThemeDB.fallback_font
	draw_string(font, card.position + Vector2(12, 30), title, HORIZONTAL_ALIGNMENT_LEFT, card.size.x - 24, 23, ink)
	draw_string(font, card.position + Vector2(12, card.size.y - 16), caption, HORIZONTAL_ALIGNMENT_LEFT, card.size.x - 24, 18, MUTED)
	if joints.is_empty():
		draw_string(font, card.position + Vector2(12, card.size.y * 0.52), "Pose unavailable", HORIZONTAL_ALIGNMENT_CENTER, card.size.x - 24, 19, MUTED)
		return
	_draw_body(Rect2(card.position + Vector2(20, 56), card.size - Vector2(40, 96)), joints, ink)

func _draw_body(area: Rect2, joints: Dictionary, ink: Color) -> void:
	var bounds := Rect2(joints.values()[0], Vector2.ZERO)
	for point: Vector2 in joints.values():
		bounds = bounds.expand(point)
	var torso_length := 0.0
	var pivot := bounds.get_center()
	var projected_pivot := area.get_center()
	if joints.has_all(TORSO):
		var shoulders: Vector2 = (joints["left_shoulder"] + joints["right_shoulder"]) * 0.5
		pivot = (joints["left_hip"] + joints["right_hip"]) * 0.5
		torso_length = shoulders.distance_to(pivot)
		projected_pivot.y += area.size.y * 0.10
	var fit := minf(
		(projected_pivot.x - area.position.x) / maxf(pivot.x - bounds.position.x, 0.001),
		(area.end.x - projected_pivot.x) / maxf(bounds.end.x - pivot.x, 0.001))
	fit = minf(fit, minf(
		(projected_pivot.y - area.position.y) / maxf(pivot.y - bounds.position.y, 0.001),
		(area.end.y - projected_pivot.y) / maxf(bounds.end.y - pivot.y, 0.001)))
	if torso_length > 0.001:
		fit = minf(fit, area.size.y * 0.27 / torso_length)
	var offset := projected_pivot - pivot * fit
	var points := {}
	for key in joints:
		points[key] = joints[key] * fit + offset
	var unit := clampf(torso_length * fit, 42.0, 90.0) if torso_length > 0.001 else 42.0
	var fill := Color("#152734").lerp(ink, 0.7)
	for limb in LIMBS:
		if not points.has_all([limb[0], limb[1]]):
			continue
		var start: Vector2 = points[limb[0]]
		var finish: Vector2 = points[limb[1]]
		var radius: float = limb[2] * unit * 0.62
		var tip_radius := radius * 0.70
		var axis := start.direction_to(finish).orthogonal()
		if not axis.is_zero_approx():
			draw_colored_polygon(PackedVector2Array([
				start + axis * radius, start - axis * radius,
				finish - axis * tip_radius, finish + axis * tip_radius]), fill)
		draw_circle(start, radius, fill, true, -1, true)
		draw_circle(finish, tip_radius, fill, true, -1, true)
	if points.has_all(TORSO):
		var polygon := PackedVector2Array()
		for name in TORSO:
			polygon.append(points[name])
		# Near-profile and overlapping landmarks can collapse or cross the torso corners.
		var triangles := Geometry2D.triangulate_polygon(polygon)
		if triangles.size() > 0:
			draw_colored_polygon(polygon, fill)
			for i in range(polygon.size()):
				draw_line(polygon[i], polygon[(i + 1) % polygon.size()], fill, unit * 0.10, true)
				draw_circle(polygon[i], unit * 0.05, fill, true, -1, true)
	if points.has("head"):
		var center: Vector2 = points["head"]
		if points.has_all(["left_shoulder", "right_shoulder"]):
			var neck: Vector2 = (points["left_shoulder"] + points["right_shoulder"]) * 0.5
			draw_line(neck, neck.move_toward(center, unit * 0.28), fill, unit * 0.20, true)
		var head := PackedVector2Array()
		for i in range(32):
			var angle := TAU * i / 32.0
			head.append(center + Vector2(cos(angle) * 0.18, sin(angle) * 0.24) * unit)
		draw_colored_polygon(head, fill)
	for point: Vector2 in points.values():
		draw_circle(point, 3, ink, true, -1, true)
