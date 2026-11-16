extends Node

signal hello_received(message: Dictionary)
signal skeleton_changed(definition: Dictionary)
signal state_changed(state: Dictionary)
signal frame_received(frame: Dictionary)
signal connection_changed(connected: bool, message: String)

const PROTOCOL_VERSION := 1
const INITIAL_BACKOFF_US := 500000
const MAX_BACKOFF_US := 8000000
const MAX_PACKETS_PER_PROCESS := 64
var engine_url: String
var _socket := WebSocketPeer.new()
var _greeted := false
var _incompatible := false
var _waiting := false
var _attempt := 0
var _retry_at_us := 0
var _last_seq := -1

func _ready() -> void:
	engine_url = OS.get_environment("BANSAYO_ENGINE_URL")
	if engine_url.is_empty():
		engine_url = str(ProjectSettings.get_setting("bansayo/engine_url", "http://127.0.0.1:7700"))
	_connect()

func _connect() -> void:
	_waiting = false
	_greeted = false
	_socket = WebSocketPeer.new()
	_socket.inbound_buffer_size = 1048576
	_socket.max_queued_packets = MAX_PACKETS_PER_PROCESS
	var url := engine_url.replace("https://", "wss://").replace("http://", "ws://").trim_suffix("/")
	connection_changed.emit(false, "Connecting to " + engine_url)
	var error := _socket.connect_to_url(url + "/api/v1/stream?topics=frame,state")
	if error != OK:
		_disconnected("Engine not connected: " + engine_url)

func _process(_delta: float) -> void:
	if _incompatible:
		return
	if _waiting:
		if Time.get_ticks_usec() >= _retry_at_us:
			_connect()
		return
	_socket.poll()
	var state := _socket.get_ready_state()
	if state == WebSocketPeer.STATE_OPEN:
		var processed := 0
		while _socket.get_available_packet_count() > 0 and processed < MAX_PACKETS_PER_PROCESS:
			var packet := _socket.get_packet()
			processed += 1
			if not _socket.was_string_packet():
				continue
			var parsed: Variant = JSON.parse_string(packet.get_string_from_utf8())
			if parsed is Dictionary:
				_handle_message(parsed)
	elif state == WebSocketPeer.STATE_CLOSED:
		_disconnected("Engine disconnected. Retrying " + engine_url)

func _handle_message(message: Dictionary) -> void:
	var kind := str(message.get("type", ""))
	if kind == "hello":
		var version := int(message.get("protocol_version", -1))
		if version != PROTOCOL_VERSION:
			_incompatible = true
			_socket.close()
			connection_changed.emit(false, "Protocol mismatch: engine %d, client %d" % [version, PROTOCOL_VERSION])
			return
		if not message.get("skeleton") is Dictionary or not message.get("state") is Dictionary:
			return
		_greeted = true
		_attempt = 0
		_last_seq = -1
		hello_received.emit(message)
		skeleton_changed.emit(message["skeleton"])
		state_changed.emit(message["state"])
		connection_changed.emit(true, "Engine connected")
	elif _greeted and kind == "skeleton" and message.get("skeleton") is Dictionary:
		skeleton_changed.emit(message["skeleton"])
	elif _greeted and kind == "state" and message.get("state") is Dictionary:
		state_changed.emit(message["state"])
	elif _greeted and kind == "frame":
		var sequence := int(message.get("seq", -1))
		if sequence <= _last_seq:
			return
		_last_seq = sequence
		frame_received.emit(message)

func _disconnected(message: String) -> void:
	_greeted = false
	_waiting = true
	_retry_at_us = Time.get_ticks_usec() + mini(MAX_BACKOFF_US, INITIAL_BACKOFF_US * (1 << mini(_attempt, 4)))
	_attempt += 1
	connection_changed.emit(false, message)

func _exit_tree() -> void:
	_socket.close()
