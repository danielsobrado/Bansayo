extends Control
## Overhead view of the headset boundary and the required radius around the headset.
const BOUNDARY := Color("#a8bdcc")
const FITS := Color("#74e6ed")
const ATTENTION := Color("#ffcb8a")
var _bounds := PackedVector3Array()
var _head := Vector2.ZERO
var _radius := 0.0
var _fits := false

func _ready() -> void:
	custom_minimum_size.y = 166
	mouse_filter = Control.MOUSE_FILTER_IGNORE
	hide()

func present(bounds: PackedVector3Array, head: Vector2, radius: float, fits: bool) -> void:
	if _bounds != bounds or not _head.is_equal_approx(head) or _radius != radius or _fits != fits:
		_bounds = bounds
		_head = head
		_radius = radius
		_fits = fits
		queue_redraw()
	visible = radius > 0.0

func _draw() -> void:
	if _radius <= 0.0:
		return
	var extents := Rect2(_head - Vector2.ONE * _radius, Vector2.ONE * 2.0 * _radius)
	for corner in _bounds:
		extents = extents.expand(Vector2(corner.x, corner.z))
	var pixels_per_meter := minf((size.x - 28) / extents.size.x, (size.y - 24) / extents.size.y)
	var offset := size * 0.5 - extents.get_center() * pixels_per_meter
	draw_rect(Rect2(Vector2.ZERO, size), Color("#152633"))
	if _bounds.size() >= 3:
		var outline := PackedVector2Array()
		for corner in _bounds:
			outline.append(offset + Vector2(corner.x, corner.z) * pixels_per_meter)
		outline.append(outline[0])
		draw_polyline(outline, BOUNDARY, 2.0, true)
	var center := offset + _head * pixels_per_meter
	var tint := FITS if _fits else ATTENTION
	draw_arc(center, _radius * pixels_per_meter, 0.0, TAU, 56, tint, 3.0, true)
	draw_circle(center, 5.0, tint, true, -1.0, true)
