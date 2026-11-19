extends Control
## Side-by-side planar figures with shared framing, anatomy and provenance labels.
const Anatomy := preload("res://scripts/pose_anatomy_2d.gd")
const LIVE := Color("#82dce4")
const GUIDE := Color("#c7b6ee")
const MUTED := Color("#b6c3d4")
const INK := Color("#edf2f9")
const GAP := 18.0
var live_pose := {}
var guide_pose := {}
var live_caption := ""
var guide_caption := ""
var _confidence := ""
var _live_torso := 0.0
var _last_sample_ms := 0
var _card_style := StyleBoxFlat.new()

func _ready() -> void:
	custom_minimum_size.y = 420
	size_flags_vertical = Control.SIZE_EXPAND_FILL
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	_card_style.bg_color = Color("#191f2a")
	_card_style.border_color = Color("#3a475a")
	_card_style.set_border_width_all(2)
	_card_style.set_corner_radius_all(22)
	_card_style.anti_aliasing = true

func reset_framing() -> void:
	_live_torso = 0.0
	_last_sample_ms = 0

func present(live: Dictionary, guide: Dictionary, camera_text: String, guide_text: String, confidence := "") -> void:
	if live == live_pose and guide == guide_pose and camera_text == live_caption and guide_text == guide_caption and confidence == _confidence:
		return
	if not live.is_empty() and (live != live_pose or _live_torso <= 0.0):
		var torso := Anatomy.torso_length(live)
		if torso > 0.0:
			var now := Time.get_ticks_msec()
			var weight := 1.0 - exp(-float(now - _last_sample_ms) / 2400.0)
			_live_torso = torso if _live_torso <= 0.0 else lerpf(_live_torso, torso, weight)
			_last_sample_ms = now
	live_pose = live
	guide_pose = guide
	live_caption = camera_text
	guide_caption = guide_text
	_confidence = confidence
	queue_redraw()

func _draw() -> void:
	var width := (size.x - GAP) * 0.5
	_draw_card(Rect2(0, 0, width, size.y), "YOUR REFLECTION", live_caption, _confidence, live_pose, LIVE, _live_torso)
	_draw_card(Rect2(width + GAP, 0, width, size.y), "REFERENCE POSE", guide_caption, "Illustrated  ·  Unscored", guide_pose, GUIDE, Anatomy.torso_length(guide_pose))

func _draw_card(card: Rect2, title: String, caption: String, note: String, joints: Dictionary, ink: Color, torso: float) -> void:
	draw_style_box(_card_style, card)
	var font := ThemeDB.fallback_font
	var left := card.position + Vector2(26, 0)
	var text_width := card.size.x - 52
	draw_circle(left + Vector2(6, 34), 5, ink, true, -1, true)
	draw_string(font, left + Vector2(24, 44), title, HORIZONTAL_ALIGNMENT_LEFT, text_width - 24, 28, INK)
	var footer := card.size.y - 70
	draw_line(left + Vector2(0, footer), left + Vector2(text_width, footer), Color("#394456"), 1.0, true)
	draw_string(font, left + Vector2(0, footer + 41), caption, HORIZONTAL_ALIGNMENT_LEFT, text_width * 0.50, 23, ink)
	draw_string(font, left + Vector2(text_width * 0.48, footer + 41), note, HORIZONTAL_ALIGNMENT_RIGHT, text_width * 0.52, 21, MUTED)
	if joints.is_empty():
		draw_string(font, left + Vector2(0, card.size.y * 0.51), "Pose unavailable", HORIZONTAL_ALIGNMENT_CENTER, text_width, 28, MUTED)
		return
	var stage := Rect2(card.position + Vector2(35, 63), card.size - Vector2(70, 143))
	var floor_y := stage.end.y - 2.0
	draw_line(Vector2(stage.position.x + 38, floor_y), Vector2(stage.end.x - 38, floor_y), Color("#303a4b"), 1.0, true)
	var result := _project(stage.grow(-12.0), joints, torso)
	Anatomy.draw(self, result.points, result.unit, ink)

func _project(area: Rect2, joints: Dictionary, torso: float) -> Dictionary:
	var bounds := Rect2(joints.values()[0], Vector2.ZERO)
	for point: Vector2 in joints.values():
		bounds = bounds.expand(point)
	var pivot := bounds.get_center()
	if joints.has_all(Anatomy.HIPS):
		pivot = (joints[Anatomy.HIPS[0]] + joints[Anatomy.HIPS[1]]) * 0.5
	var anchor := area.position + area.size * Vector2(0.5, 0.565)
	var unit := area.size.y * 0.288
	var scale_factor := unit / torso if torso > 0.0 else area.size.y * 0.72 / maxf(bounds.size.y, 1.0)
	# Only constrain the stable scale when a pose approaches the card edges.
	var room_before := anchor - area.position
	var room_after := area.end - anchor
	var extent_before := pivot - bounds.position
	var extent_after := bounds.end - pivot
	if extent_before.x > 0.0:
		scale_factor = minf(scale_factor, room_before.x / extent_before.x)
	if extent_before.y > 0.0:
		scale_factor = minf(scale_factor, room_before.y / extent_before.y)
	if extent_after.x > 0.0:
		scale_factor = minf(scale_factor, room_after.x / extent_after.x)
	if extent_after.y > 0.0:
		scale_factor = minf(scale_factor, room_after.y / extent_after.y)
	var projected := {}
	for name in joints:
		projected[name] = anchor + (joints[name] - pivot) * scale_factor
	return {"points": projected, "unit": clampf(torso * scale_factor, unit * 0.67, unit)}
