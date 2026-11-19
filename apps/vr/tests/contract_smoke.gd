extends SceneTree
const AvatarRenderer = preload("res://scripts/avatar_renderer.gd")
const PoseBuffer = preload("res://scripts/pose_buffer.gd")
const MovementCounter = preload("res://scripts/movement_counter.gd")
const Clinic = preload("res://scripts/clinic.gd")
const ClinicTarget = preload("res://scripts/clinic_target.gd")
const GhostWalls = preload("res://scripts/ghost_walls.gd")

func _initialize() -> void:
	# Golden contracts remain the source of truth; no duplicate wire schema is maintained here.
	var path := ProjectSettings.globalize_path("res://../../contracts/examples/hello.json")
	var hello: Dictionary = JSON.parse_string(FileAccess.get_file_as_string(path))
	assert(int(hello["protocol_version"]) == 1)
	assert(hello["conventions"]["quaternion_order"] == "xyzw")
	assert(hello["conventions"]["up"] == "+y")
	assert(hello["skeleton"]["segments"].size() == 19)
	var q := PoseBuffer.quaternion([0, sqrt(0.5), 0, sqrt(0.5)])
	assert((q * Vector3(0, 0, 1)).distance_to(Vector3(1, 0, 0)) < 0.00001)
	var buffer := PoseBuffer.new()
	buffer.push({"t_us": 0, "pose": null}, 100000)
	assert(buffer.sample(100000).is_empty())
	var counter := MovementCounter.new()
	for knee in [5.0, 40.0, 90.0, 60.0, 30.0, 10.0, 80.0, 20.0]: # two squats; 30 is not yet up
		counter.update({"left_knee_flexion": knee, "right_knee_flexion": knee}, {"pelvis": 1.0 - knee / 200.0})
	assert(counter.squats == 2)
	assert(is_equal_approx(counter.peaks["left_knee_flexion"], 90.0))
	assert(is_equal_approx(counter.min_pelvis_height_m, 0.55))
	for hip in [0.0, 70.0, 10.0, 50.0, 0.0]: # one kick; 50 never reaches the kick threshold
		counter.update({"right_hip_flexion": hip, "left_hip_flexion": 5.0}, {"right_ankle": hip / 100.0})
	assert(counter.kicks["right"] == 1 and counter.kicks["left"] == 0)
	var bounds := PackedVector3Array([Vector3(-3, 0, -3), Vector3(3, 0, -3), Vector3(3, 0, 3), Vector3(-3, 0, 3)])
	assert(Clinic.circle_fits(bounds, Vector2.ZERO, 2.3))
	assert(not Clinic.circle_fits(bounds, Vector2(1, 0), 2.3))
	assert(not Clinic.circle_fits(bounds, Vector2(4, 0), 0.1))
	assert(not Clinic.circle_fits(PackedVector3Array(), Vector2.ZERO, 2.3))
	assert(is_equal_approx(counter.max_ankle_height_m, 0.7))
	for hip in [0.0, 85.0, 5.0]: # a squat flexes both hips: not a kick
		counter.update({"right_hip_flexion": hip, "left_hip_flexion": hip}, {})
	assert(counter.kicks["right"] == 1 and counter.kicks["left"] == 0)
	_check_target_effect.call_deferred()

func _check_target_effect() -> void:
	var target := ClinicTarget.new()
	root.add_child(target)
	target.present("clinic-target", Vector3(0, 0.6, -1), 0.15, Vector3(0, 1.7, 0), 0.6)
	assert(target.visible and not target._burst.visible)
	assert(target._height.text == "60 cm")
	assert(not target.reach("other-target"))
	assert(target.reach("clinic-target"))
	assert(target._burst.visible and target._sound.stream is AudioStreamWAV)
	await create_timer(0.35).timeout
	assert(not target._burst.visible)
	target.clear()
	assert(not target.visible and not target._sound.playing)
	if not _check_avatar() or not _check_ghost_walls():
		quit(1)
		return
	print("Bansayo Godot contract smoke passed")
	quit(0)

func _check_avatar() -> bool:
	var avatar: Node3D = AvatarRenderer.new()
	root.add_child(avatar)
	if not avatar.has_method("set_mannequin"):
		push_error("The avatar renderer needs a mannequin mode.")
		return false
	avatar.set_mannequin(true)
	avatar.build({"rev": 1, "segments": [{"id": "left_foot", "tail_m": [0, -0.07, 0.19]}]})
	var foot := {"position_m": [0, 0, 0], "rotation": [0, 0, 0, 1], "provenance": "inferred"}
	avatar.apply_pose({"skeleton_rev": 1, "segments": [foot]})
	assert(avatar._segments[0].visible and avatar._meshes[0].scale.is_equal_approx(Vector3.ONE * 0.06))
	avatar.apply_pose({"skeleton_rev": 1, "segments": []})
	assert(not avatar._segments[0].visible)
	avatar.queue_free()
	return true

func _wall_ticks(walls: Node3D, seconds: float, offset := Vector3.ZERO) -> void:
	for i in range(roundi(seconds / 0.05)):
		walls.step(0.05, true, offset)

func _check_ghost_walls() -> bool:
	var camera := Camera3D.new()
	root.add_child(camera)
	var clinic := Clinic.new()
	root.add_child(clinic)
	clinic.camera = camera
	clinic.opened = true
	clinic.show()
	clinic._choice = clinic._menu().find("Ghost walls")
	clinic.engine_state({"training": {"phase": "active"}})
	clinic.select()
	assert(not clinic._walls.opened)
	clinic.engine_state({})
	clinic.select()
	var walls := clinic._walls
	assert(walls.opened and clinic.panel.visible)
	walls.select(false)
	assert(walls.phase == "ready")
	walls.select(true)
	_wall_ticks(walls, 3.05)
	assert(walls.phase == "running" and walls._wall.visible)
	_wall_ticks(walls, 1.2)
	walls.step(0.05, false)
	assert(walls.phase == "paused" and not walls._wall.visible and walls.assessed == 0)
	walls.select(true)
	_wall_ticks(walls, 3.05)
	_wall_ticks(walls, 2.45, Vector3(-0.2, 0, 0))
	assert(walls.assessed == 1 and walls.cleared == 1 and not walls._wall.visible)
	walls.interrupt("Test pause after a clear")
	walls.select(true)
	_wall_ticks(walls, 3.05)
	assert(walls._kind() == "right" and walls.assessed == 1)
	_wall_ticks(walls, GhostWalls.SLOT_S + 0.05)
	assert(walls._kind() == "duck" and walls.assessed == 2 and walls.cleared == 1)
	_wall_ticks(walls, GhostWalls.SLOT_S + 0.05, Vector3(0, -0.19, 0))
	assert(walls.assessed == 3 and walls.cleared == 2)
	_wall_ticks(walls, 2.0 * GhostWalls.SLOT_S + 0.15)
	assert(walls.phase == "rest" and walls.assessed == 5)
	_wall_ticks(walls, 2.0)
	var remaining := walls._rest_left
	walls.interrupt("Test pause during rest")
	walls.select(true)
	_wall_ticks(walls, 3.05)
	assert(walls.phase == "rest" and absf(walls._rest_left - remaining) < 0.10)
	_wall_ticks(walls, GhostWalls.REST_S + 3.10)
	assert(walls.phase == "running" and walls._round == 1 and walls._kind() == "duck")
	_wall_ticks(walls, 5.0 * GhostWalls.SLOT_S + 0.30)
	assert(walls.phase == "complete" and walls.assessed == 10)
	walls.select(true)
	assert(walls.phase == "ready" and walls.cleared == 0)
	clinic.next()
	clinic.select()
	assert(not walls.opened and clinic.panel.visible)
	clinic._session = "current"
	clinic.stream_state_changed({"session_id": "current", "phase": "ready", "rev": 1})
	assert(clinic._menu().has("Confirm clear area & start"))
	clinic._use_targets = true
	assert(clinic._menu().has("Place target & start"))
	clinic.stream_state_changed({"session_id": "current", "phase": "active", "rev": 2})
	var event := {"type": "target_reached", "event_seq": 3, "data": {"id": "clinic-target"}}
	clinic.event_received("current", event)
	clinic.event_received("current", event)
	assert(clinic._target_reaches == 1)
	clinic.connection(false)
	clinic.connection(true)
	clinic.stream_state_changed({"session_id": "replacement", "phase": "ready"})
	assert(clinic._phase() == "setup" and clinic._session.is_empty() and clinic._target_reaches == 0)
	clinic.queue_free()
	camera.queue_free()
	return true
