extends SceneTree

## Run against a fresh simulator engine using --xr-mode off -- --desktop.
const Room = preload("res://training_room.tscn")
var room: Node3D

func _initialize() -> void:
	_run.call_deferred()

func _check(condition: bool, message: String) -> bool:
	if not condition:
		push_error(message)
		quit(1)
	return condition

func _wait_phase(phase: String, seconds: int) -> bool:
	for i in range(seconds * 10):
		if room._clinic._phase() == phase:
			return true
		await create_timer(0.1).timeout
	return false

func _choose(label: String) -> void:
	room._clinic._choice = room._clinic._menu().find(label)
	if not _check(room._clinic._choice >= 0, "Missing clinic action " + label):
		return
	await room._clinic.select()

func _run() -> void:
	room = Room.instantiate()
	root.add_child(room)
	for i in range(100):
		if room._clinic.connected:
			break
		await create_timer(0.1).timeout
	if not _check(room._clinic.connected, "Clinic did not connect"):
		return
	var actions = room._actions
	const POST := HTTPClient.METHOD_POST
	const PUT := HTTPClient.METHOD_PUT
	var athlete := "xr_test_%d" % int(Time.get_unix_time_from_system())
	await actions.request(POST, "/athletes", JSON.stringify({"id": athlete, "display_name": "XR Test", "height_m": 1.75, "default_stance": "orthodox"}))
	await actions.request(POST, "/athletes/" + athlete + "/consent", JSON.stringify({"motion_data": true}))
	await actions.request(PUT, "/simulation", JSON.stringify({"motion": "standing", "speed": 1}))
	await room._clinic.show_menu()
	for i in range(room._clinic._athletes.size()):
		if room._clinic._athletes[i].get("id") == athlete:
			room._clinic._athlete = i
			break
	room._clinic._kick = 1
	await _choose("Prepare clinic")
	if not _check(not room._clinic._session.is_empty(), "Clinic did not prepare"):
		return
	await _choose("Acknowledge safety notes")
	if not _check(await _wait_phase("ready", 12), "Clinic did not become ready"):
		return
	await _choose("Confirm clear area & start")
	if not _check(await _wait_phase("active", 12), "Clinic did not start"):
		return
	await actions.request(PUT, "/simulation", JSON.stringify({"motion": "roundhouse_kick", "speed": 1}))
	if not _check(await _wait_phase("resting", 95), "First set did not finish"):
		return
	await actions.request(PUT, "/simulation", JSON.stringify({"motion": "standing", "speed": 1}))
	await _choose("Skip rest")
	await actions.request(PUT, "/simulation", JSON.stringify({"motion": "roundhouse_kick", "speed": 1}))
	if not _check(await _wait_phase("completed", 95), "Second set did not finish"):
		return
	await actions.request(PUT, "/simulation", JSON.stringify({"motion": "standing", "speed": 1}))
	await create_timer(0.75).timeout
	await _choose("Review sets & replay ghost")
	if not _check(room._clinic._review.get("second") is Dictionary and room._clinic._ghost.visible, "Set comparison or ghost is missing"):
		return
	var previous: String = room._clinic._session
	await _choose("Practise again")
	if not _check(room._clinic._session != previous and await _wait_phase("ready", 12), "Repeat clinic did not prepare"):
		return
	await _choose("End clinic")
	print("Bansayo Godot clinic smoke passed")
	quit(0)
