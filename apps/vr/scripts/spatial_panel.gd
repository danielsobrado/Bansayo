extends Node3D
## Shared 2D controls on a depth-tested world-space surface.
signal selected(index: int)

const PlayAreaPreview = preload("res://scripts/play_area_preview.gd")
const PIXELS := Vector2i(1024, 704)
const SIZE := Vector2(1.0, 0.6875)
const COACHING_PIXELS := Vector2i(1536, 960)
const COACHING_SIZE := Vector2(1.56, 0.975)
const MODE_BUTTONS := 4
const NO_HIT := Vector2(-1, -1)
const INK := Color("#f0f6fa")
const MUTED := Color("#a8bdcc")
const ACCENT := Color("#74e6ed")
const WARNING := Color("#ffcb8a")
const STOP := Color("#ff9b9b")

var coaching_layout := false
var size_multiplier := 1.0
var height_offset := -0.18

var _viewport := SubViewport.new()
var _title := Label.new()
var _status := Label.new()
var _options_scroll: ScrollContainer
var _details_scroll: ScrollContainer
var _options: VBoxContainer
var _mode_bar: HBoxContainer
var _action_bar: HBoxContainer
var _size := SIZE
var _details := Label.new()
var _detail_column := VBoxContainer.new()
var play_area: PlayAreaPreview
var _buttons: Array[Button] = []
var _labels: Array[String] = []
var _choice := -1
var _hover := -1

func _ready() -> void:
	_size = COACHING_SIZE if coaching_layout else SIZE
	_viewport.size = COACHING_PIXELS if coaching_layout else PIXELS
	_viewport.transparent_bg = coaching_layout
	_viewport.render_target_update_mode = SubViewport.UPDATE_DISABLED
	add_child(_viewport)

	var quad := QuadMesh.new()
	quad.size = _size
	var material := StandardMaterial3D.new()
	material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	material.albedo_texture = _viewport.get_texture()
	if coaching_layout:
		material.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA_SCISSOR
	var display := MeshInstance3D.new()
	display.mesh = quad
	display.material_override = material
	display.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	add_child(display)

	var frame := PanelContainer.new()
	var shell := _box(Color("#121923"), Color("#3b4b5f"), 2) if coaching_layout else _box(Color("#101d29"), ACCENT, 2)
	if coaching_layout:
		shell.set_corner_radius_all(30)
	frame.add_theme_stylebox_override("panel", shell)
	_viewport.add_child(frame)
	frame.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	var margin := MarginContainer.new()
	for edge in ["left", "right", "top", "bottom"]:
		margin.add_theme_constant_override("margin_" + edge, 38 if coaching_layout else 30)
	frame.add_child(margin)
	var layout := VBoxContainer.new()
	layout.add_theme_constant_override("separation", 16 if coaching_layout else 12)
	margin.add_child(layout)
	_label_style(_title, 56 if coaching_layout else 46, INK)
	_label_style(_status, 28 if coaching_layout else 29, ACCENT)
	layout.add_child(_title)
	layout.add_child(_status)
	if coaching_layout:
		_build_coaching_layout(layout)
	else:
		layout.add_child(HSeparator.new())
		_build_standard_layout(layout)
	visibility_changed.connect(_update_visibility)
	_update_visibility()

func _build_coaching_layout(layout: VBoxContainer) -> void:
	_mode_bar = HBoxContainer.new()
	_action_bar = HBoxContainer.new()
	_details.hide()
	_mode_bar.add_theme_constant_override("separation", 12)
	_action_bar.add_theme_constant_override("separation", 12)
	layout.add_child(_mode_bar)
	_detail_column.add_theme_constant_override("separation", 10)
	_detail_column.size_flags_vertical = Control.SIZE_EXPAND_FILL
	layout.add_child(_detail_column)
	_label_style(_details, 25, MUTED)
	_detail_column.add_child(_details)
	layout.add_child(_action_bar)

func _build_standard_layout(layout: VBoxContainer) -> void:
	_options_scroll = ScrollContainer.new()
	_details_scroll = ScrollContainer.new()
	_options = VBoxContainer.new()
	play_area = PlayAreaPreview.new()
	var columns := HBoxContainer.new()
	columns.size_flags_vertical = Control.SIZE_EXPAND_FILL
	columns.add_theme_constant_override("separation", 30)
	layout.add_child(columns)
	_options_scroll.custom_minimum_size.x = 468
	_options_scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	_options_scroll.add_child(_options)
	_options.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_options.add_theme_constant_override("separation", 8)
	columns.add_child(_options_scroll)
	_detail_column.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_detail_column.add_theme_constant_override("separation", 14)
	_detail_column.add_child(play_area)
	_details_scroll.size_flags_vertical = Control.SIZE_EXPAND_FILL
	_details_scroll.horizontal_scroll_mode = ScrollContainer.SCROLL_MODE_DISABLED
	_details.autowrap_mode = TextServer.AUTOWRAP_WORD_SMART
	_details.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	_label_style(_details, 28, MUTED)
	_details.add_theme_constant_override("line_spacing", 7)
	_details_scroll.add_child(_details)
	_detail_column.add_child(_details_scroll)
	columns.add_child(_detail_column)

func set_visual(visual: Control) -> void:
	_detail_column.add_child(visual)
	_detail_column.move_child(visual, 0)

func _update_visibility() -> void:
	_viewport.render_target_update_mode = SubViewport.UPDATE_WHEN_VISIBLE if is_visible_in_tree() else SubViewport.UPDATE_DISABLED
	if not is_visible_in_tree():
		hover_at(NO_HIT)

func _label_style(label: Label, size: int, color: Color) -> void:
	label.add_theme_font_size_override("font_size", size)
	label.add_theme_color_override("font_color", color)

func _box(fill: Color, border: Color, width: int) -> StyleBoxFlat:
	var style := StyleBoxFlat.new()
	style.bg_color = fill
	style.border_color = border
	style.set_border_width_all(width)
	style.set_corner_radius_all(10)
	return style

func recenter(camera: Camera3D, distance := 1.4) -> void:
	var forward := -camera.global_basis.z
	forward.y = 0.0
	forward = Vector3.FORWARD if forward.length_squared() < 0.01 else forward.normalized()
	var basis := Basis.looking_at(forward, Vector3.UP).scaled(Vector3.ONE * size_multiplier)
	global_transform = Transform3D(basis, camera.global_position + forward * distance + Vector3.UP * height_offset)

func present(title: String, status: String, lines: Array[String], options: Array[String], choice: int, disabled: bool, warning := false, stop := false) -> void:
	if _title.text != title:
		_title.text = title
		if not coaching_layout:
			_details_scroll.scroll_vertical = 0
	if _status.text != status:
		_status.text = status
	var color := STOP if stop else (WARNING if warning else ACCENT)
	if _status.get_theme_color("font_color") != color:
		_status.add_theme_color_override("font_color", color)
	var details := "\n\n".join(lines)
	if _details.text != details:
		_details.text = details
		_details.visible = not coaching_layout or not details.is_empty()
	if options != _labels:
		var count_changed := options.size() != _labels.size()
		_labels = options.duplicate()
		while _buttons.size() > options.size():
			var button: Button = _buttons.pop_back()
			button.get_parent().remove_child(button)
			button.queue_free()
		while _buttons.size() < options.size():
			_add_button()
		for i in range(options.size()):
			_buttons[i].text = options[i]
		if count_changed and not coaching_layout:
			_options_scroll.scroll_vertical = 0
	_choice = choice
	for i in range(_buttons.size()):
		_buttons[i].disabled = disabled
		_buttons[i].set_pressed_no_signal(i == _choice or i == _hover)

func _add_button() -> void:
	var button := Button.new()
	var index := _buttons.size()
	button.custom_minimum_size.y = 68 if coaching_layout else 64
	if coaching_layout:
		button.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	button.add_theme_font_size_override("font_size", 25)
	button.add_theme_color_override("font_color", INK)
	button.add_theme_color_override("font_pressed_color", INK)
	button.add_theme_color_override("font_hover_pressed_color", INK)
	button.add_theme_stylebox_override("normal", _box(Color("#1c3343"), Color("#33536a"), 2))
	button.add_theme_stylebox_override("pressed", _box(Color("#214857"), ACCENT, 3))
	button.add_theme_stylebox_override("disabled", _box(Color("#152431"), Color("#293d4d"), 2))
	if coaching_layout:
		var mode := index < MODE_BUTTONS
		button.add_theme_font_size_override("font_size", 28 if mode else 25)
		button.add_theme_stylebox_override("normal", _box(Color("#1b2632") if mode else Color("#19212c"), Color("#303d4d"), 1))
		button.add_theme_stylebox_override("pressed", _box(Color("#254152"), Color("#7dbdc7"), 2))
	button.text_overrun_behavior = TextServer.OVERRUN_TRIM_ELLIPSIS
	button.toggle_mode = true
	button.focus_mode = Control.FOCUS_NONE
	button.pressed.connect(func(): selected.emit(index))
	_buttons.append(button)
	if coaching_layout:
		var bar := _mode_bar if index < MODE_BUTTONS else _action_bar
		bar.add_child(button)
	else:
		_options.add_child(button)

func intersect_ray(origin: Vector3, direction: Vector3) -> Vector2:
	if not is_visible_in_tree():
		return NO_HIT
	var local := to_local(origin)
	var aim := global_basis.inverse() * direction
	if local.z <= 0.0 or aim.z >= -0.0001:
		return NO_HIT
	var at := local - aim * (local.z / aim.z)
	var uv := Vector2(at.x / _size.x + 0.5, 0.5 - at.y / _size.y)
	if not Rect2(Vector2.ZERO, Vector2.ONE).has_point(uv):
		return NO_HIT
	return uv * Vector2(_viewport.size)

func world_point(pixel: Vector2) -> Vector3:
	var uv := pixel / Vector2(_viewport.size) - Vector2(0.5, 0.5)
	return to_global(Vector3(uv.x * _size.x, -uv.y * _size.y, 0.0))

func button_world_point(index: int) -> Vector3:
	return world_point(_buttons[index].get_global_rect().get_center())

func _button_at(pixel: Vector2) -> int:
	if not is_visible_in_tree() or (not coaching_layout and not _options_scroll.get_global_rect().has_point(pixel)):
		return -1
	for i in range(_buttons.size()):
		if not _buttons[i].disabled and _buttons[i].get_global_rect().has_point(pixel):
			return i
	return -1

func label_at(pixel: Vector2) -> String:
	var index := _button_at(pixel)
	return _labels[index] if index >= 0 else ""

func hover_at(pixel: Vector2) -> void:
	var index := _button_at(pixel)
	if index == _hover:
		return
	_hover = index
	for i in range(_buttons.size()):
		_buttons[i].set_pressed_no_signal(i == _choice or i == _hover)

func press_at(pixel: Vector2) -> bool:
	var index := _button_at(pixel)
	if index < 0:
		return false
	_buttons[index].pressed.emit()
	return true

func scroll_at(pixel: Vector2, amount: int) -> void:
	if coaching_layout:
		return
	if _options_scroll.get_global_rect().has_point(pixel):
		_options_scroll.scroll_vertical += amount
	elif _details_scroll.get_global_rect().has_point(pixel):
		_details_scroll.scroll_vertical += amount

func show_choice(index: int) -> void:
	if not coaching_layout and index >= 0 and index < _buttons.size():
		_options_scroll.ensure_control_visible(_buttons[index])
