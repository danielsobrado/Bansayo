extends Node
# Recording and replay through the engine's HTTP API; the engine owns both.

signal message(text: String)
var engine_url: String
var recording := false
var replaying := false
var _busy := false

func set_state(state: Dictionary) -> void:
	recording = state.get("recording") is Dictionary
	replaying = str(state.get("engine", {}).get("mode", "")) == "replay"

func toggle_recording() -> void:
	if recording:
		request(HTTPClient.METHOD_POST, "/recording/stop", "{}", "Recording saved")
	else:
		request(HTTPClient.METHOD_POST, "/recording/start", "{}", "Recording")

## Replays the newest session, or returns to live input while replaying.
func toggle_replay() -> void:
	if replaying:
		request(HTTPClient.METHOD_POST, "/playback/stop", "{}", "Live")
		return
	var sessions: Variant = await request(HTTPClient.METHOD_GET, "/sessions")
	if not sessions is Array or sessions.is_empty():
		message.emit("No recorded session to replay")
		return
	var newest: Dictionary = sessions[0]
	for session in sessions:
		if str(session.get("created_utc", "")) > str(newest.get("created_utc", "")):
			newest = session
	request(HTTPClient.METHOD_POST, "/playback/start", JSON.stringify({"session_id": newest["id"], "looping": true}), "Replaying " + str(newest.get("name", newest["id"])))

func request(method: int, path: String, body: String = "", done: String = "", quiet := false) -> Variant:
	if _busy:
		return null
	_busy = true
	var http := HTTPRequest.new()
	http.timeout = 5.0
	add_child(http)
	var result_code := http.request(engine_url.trim_suffix("/") + "/api/v1" + path, ["Content-Type: application/json"], method, body)
	if result_code != OK:
		http.queue_free()
		_busy = false
		if not quiet:
			message.emit("The engine request could not start")
		return null
	var result: Array = await http.request_completed
	http.queue_free()
	_busy = false
	var parsed: Variant = JSON.parse_string((result[3] as PackedByteArray).get_string_from_utf8())
	if result[0] != HTTPRequest.RESULT_SUCCESS or not parsed is Dictionary:
		if not quiet:
			message.emit("Engine not reachable")
		return null
	if not parsed.get("ok", false):
		if not quiet:
			message.emit(str(parsed.get("error", {}).get("message", "Request failed")))
		return null
	if not done.is_empty():
		message.emit(done)
	return parsed.get("data")
