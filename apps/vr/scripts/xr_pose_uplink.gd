extends Node
## Streams OpenXR head + grip samples to the engine at GET /api/v1/ingest/xr (Phase 01).
## Send-only and separate from EngineClient's receive-only downlink. One socket at a time; a
## sample is skipped, never queued, when the socket is backed up. Never touches the avatar.
const XrPoseCapture = preload("res://scripts/xr_pose_capture.gd")

signal status_changed(text: String)

const PROTOCOL_VERSION := 1
const PATH := "/api/v1/ingest/xr"

var engine_url := "http://127.0.0.1:7700"
var rate_hz := 60
var max_buffered_bytes := 8192
var reconnect_initial_us := 500000
var reconnect_max_us := 8000000
## Returns the three samples for this tick (XrPoseCapture.samples).
var capture: Callable
## Only a focused session reports tracked poses.
var focused := false
var dropped := 0
var status := ""

var _socket: WebSocketPeer
var _hello_sent := false
var _handshaken := false
var _incompatible := false
var _seq := 0
var _next_sample_us := 0
var _attempt := 0
var _retry_at_us := 0
var _paused_sent := false

static func enabled() -> bool:
	return bool(ProjectSettings.get_setting("bansayo/xr_uplink_enabled", false))

static func ingest_url(base: String) -> String:
	return base.replace("https://", "wss://").replace("http://", "ws://").trim_suffix("/") + PATH

func configure(base_url: String) -> void:
	engine_url = base_url
	rate_hz = clampi(int(ProjectSettings.get_setting("bansayo/xr_uplink_rate_hz", 60)), 1, 120)
	max_buffered_bytes = maxi(1024, int(ProjectSettings.get_setting("bansayo/xr_uplink_max_buffered_bytes", 8192)))
	reconnect_initial_us = maxi(100, int(ProjectSettings.get_setting("bansayo/xr_uplink_reconnect_initial_ms", 500))) * 1000
	reconnect_max_us = maxi(reconnect_initial_us / 1000, int(ProjectSettings.get_setting("bansayo/xr_uplink_reconnect_max_ms", 8000))) * 1000

func _ready() -> void:
	_connect()

func _set_status(text: String) -> void:
	if text == status:
		return
	status = text
	print("XR uplink: ", text)
	status_changed.emit(text)

func _connect() -> void:
	_socket = WebSocketPeer.new()
	_hello_sent = false
	_handshaken = false
	_paused_sent = false
	var error := _socket.connect_to_url(ingest_url(engine_url))
	if error != OK:
		_retry("cannot connect to " + engine_url)
	else:
		# Needs the TCP peer connect_to_url created. Batched small frames would reach the
		# engine's arrival-time rate gate as bursts.
		_socket.set_no_delay(true)
		_set_status("connecting to " + engine_url)

func _retry(reason: String) -> void:
	_handshaken = false
	_retry_at_us = Time.get_ticks_usec() + mini(reconnect_max_us, reconnect_initial_us * (1 << mini(_attempt, 5)))
	_attempt += 1
	_socket = null
	_set_status(reason + " · retrying")

func _process(_delta: float) -> void:
	if _incompatible:
		return
	if _socket == null:
		if Time.get_ticks_usec() >= _retry_at_us:
			_connect()
		return
	_socket.poll()
	match _socket.get_ready_state():
		WebSocketPeer.STATE_OPEN:
			if not _hello_sent:
				_hello_sent = _socket.send_text(JSON.stringify({"type": "hello", "protocol_version": PROTOCOL_VERSION})) == OK
			_read()
			if _handshaken:
				_tick(Time.get_ticks_usec())
		WebSocketPeer.STATE_CLOSED:
			var code := _socket.get_close_code()
			# A refused upgrade (404 xr disabled, 409 busy or replay) closes without a code.
			_retry("engine closed the uplink (%d)" % code if code > 0 else "engine refused or unreachable")

func _read() -> void:
	while _socket.get_available_packet_count() > 0:
		var packet := _socket.get_packet()
		if not _socket.was_string_packet():
			continue
		var message: Variant = JSON.parse_string(packet.get_string_from_utf8())
		if not message is Dictionary:
			continue
		match str(message.get("type", "")):
			"ready":
				if int(message.get("protocol_version", -1)) == PROTOCOL_VERSION and not _handshaken:
					_handshaken = true
					_attempt = 0
					_seq = 0 # Per connection; the engine re-links devices on a new socket.
					_next_sample_us = 0
					_set_status("streaming (%s)" % str(message.get("time_quality", "")))
			"error":
				var code := str(message.get("code", ""))
				_set_status("engine rejected: %s" % code)
				if code == "xr_protocol_version":
					_incompatible = true
					_socket.close()

## At most rate_hz samples on the monotonic process clock, independent of the render rate.
func _tick(now_us: int) -> void:
	if now_us < _next_sample_us:
		return
	var interval := 1000000 / rate_hz
	_next_sample_us = maxi(_next_sample_us + interval, now_us + interval / 2)
	var device_samples: Array
	if focused and capture.is_valid():
		_paused_sent = false
		device_samples = capture.call()
	elif not _paused_sent:
		# One explicit "untracked" frame on focus loss, then nothing until focus returns.
		_paused_sent = true
		device_samples = []
		for role in XrPoseCapture.ROLES:
			device_samples.append(XrPoseCapture.untracked(role))
	else:
		return
	if _socket.get_current_outbound_buffered_amount() > max_buffered_bytes:
		dropped += 1
		return
	if _socket.send_text(JSON.stringify(XrPoseCapture.frame(_seq, now_us, device_samples))) == OK:
		_seq += 1

## XR recentered: stage poses may jump; they are still raw stage, so streaming continues.
func recentered() -> void:
	print("XR uplink: headset recentered; stage poses may shift, world alignment must be redone")

func stop() -> void:
	if _socket:
		_socket.close(1000, "xr session ended")
		_socket = null
	_handshaken = false
	_incompatible = true
	_set_status("stopped")

func _exit_tree() -> void:
	if _socket:
		_socket.close(1000, "client exiting")
