extends Node3D
var _label := Label3D.new()
var _connected := false
var _connection_message := "Engine not connected"
var _state: Dictionary = {}
var _frame: Dictionary = {}
var _gap := true

func _ready() -> void:
	_label.font_size = 36
	_label.pixel_size = 0.002
	_label.outline_size = 4
	_label.no_depth_test = true
	_label.billboard = BaseMaterial3D.BILLBOARD_ENABLED
	add_child(_label)
	_refresh()

func connection(connected: bool, message: String) -> void:
	_connected = connected
	_connection_message = message
	if not connected:
		_frame.clear()
	_refresh()

func state_changed(state: Dictionary) -> void:
	_state = state
	_refresh()

func frame_received(frame: Dictionary) -> void:
	_frame = frame
	_refresh()

func set_gap(gap: bool) -> void:
	if _gap != gap:
		_gap = gap
		_refresh()

func _refresh() -> void:
	var lines: Array[String] = ["BANSAYO · TRAINING ROOM", _connection_message]
	if _connected and not _frame.is_empty():
		var origins: Array = _frame.get("origins", [])
		var origin_text := ", ".join(PackedStringArray(origins)).to_upper()
		if str(_state.get("engine", {}).get("mode", "")) == "replay":
			origin_text = "REPLAY · " + origin_text
		var quality: Dictionary = _frame.get("quality", {})
		lines.append(origin_text)
		lines.append("%s · %d/%d segments" % [str(quality.get("state", "unavailable")), int(quality.get("observed_segments", 0)), int(quality.get("total_segments", 0))])
		if _gap:
			lines.append("No pose data · stream gap or pose unavailable")
	lines.append("XR alignment: assumed identity")
	lines.append("50 ms interpolation buffer · joint angles not validated")
	_label.text = "\n".join(PackedStringArray(lines))
