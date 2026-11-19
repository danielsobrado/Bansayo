extends SceneTree

const Anatomy := preload("res://scripts/pose_anatomy_2d.gd")

func _initialize() -> void:
	_run.call_deferred()

func _check(condition: bool, message: String) -> bool:
	if not condition:
		push_error(message)
		quit(1)
	return condition

func _run() -> void:
	var room: Node3D = load("res://training_room.tscn").instantiate()
	root.add_child(room)
	if not _check(room.has_method("_open_mirror"), "The training room needs a camera pose mirror."):
		return
	room._open_mirror()
	var mirror: Node3D = room._mirror
	await process_frame
	var panel: Node3D = mirror.panel
	var display: MeshInstance3D = panel.get_children().filter(func(node): return node is MeshInstance3D)[0]
	if not _check(display.mesh.size.x > 1.30 and display.mesh.size.y > 0.80, "The view must give the reflection and guide a wider coaching surface."):
		return
	var origin: Vector3 = room._dev.camera.global_position
	var aim := origin.direction_to(panel.button_world_point(3))
	if not _check(panel.press_at(panel.intersect_ray(origin, aim)) and not mirror.view.guide_pose.is_empty(), "Flexibility should be selectable in the coaching navigation."):
		return
	mirror._set_mode("session")
	var names := ["nose", "left_shoulder", "right_shoulder", "left_hip", "right_hip", "left_knee"]
	mirror.engine_state({"devices": [{"descriptor": {"id": "camera", "camera": {
		"landmark_schema": {"id": "test.body", "names": names}}}}]})
	var frame := {"frame_meta": {"device_id": "camera", "image_size_px": [1000, 500]}, "schema_id": "test.body", "origin": "live", "points": [
		{"x_px": 500, "y_px": 100, "confidence": 0.9},
		{"x_px": 600, "y_px": 200, "confidence": 0.9},
		{"x_px": 400, "y_px": 200, "confidence": 0.9},
		{"x_px": 570, "y_px": 300, "confidence": 0.9},
		{"x_px": 430, "y_px": 300, "confidence": 0.9},
		{"x_px": 620, "y_px": 380, "confidence": 0.1},
	]}
	mirror.landmarks_received(frame)
	var reflected: Dictionary = mirror.view.live_pose
	var edges := Anatomy.skeleton_edges(reflected)
	if not _check(not edges.is_empty(), "The mirror should draw bones between observed joints."):
		return
	for edge in edges:
		if not _check(reflected.has_all([edge[0], edge[1]]), "A bone cannot imply a missing landmark."):
			return
	if not _check(reflected.get("left_shoulder") == Vector2(400, 200) and not reflected.has("left_knee"), "Mirror must flip horizontal pixels and omit unreliable points."):
		return
	mirror.training_changed({"phase": "active", "current": {"title": "Front kick", "phase_id": "chamber",
		"exercise": {"ref": "taekwondo.ap_chagi@1", "side": "left"}}})
	var chamber: Dictionary = mirror.view.guide_pose.duplicate()
	mirror.training_changed({"phase": "active", "current": {"title": "Front kick", "phase_id": "extension",
		"exercise": {"ref": "taekwondo.ap_chagi@1", "side": "left"}}})
	if not _check(not chamber.is_empty() and mirror.view.guide_pose != chamber, "Clinic guide must follow the engine technique phase."):
		return
	mirror.training_changed({"phase": "active", "current": {"item_id": "squat", "title": "Squat",
		"exercise": {"ref": "general.workout.full_body_in_20@1"}}})
	if not _check(not mirror.view.guide_pose.is_empty() and mirror.view.guide_pose != chamber, "Timed workout item needs its own guide."):
		return
	mirror.training_changed({"phase": "active", "current": {"item_id": "unknown", "title": "Unknown",
		"exercise": {"ref": "other.workout@1"}}})
	if not _check(mirror.view.guide_pose.is_empty(), "Unsupported exercise must not reuse a previous guide."):
		return
	mirror.training_changed({})
	mirror.choose_flexibility()
	if not _check(not mirror.view.guide_pose.is_empty(), "A flexibility guide should be available without an active session."):
		return
	await create_timer(0.6).timeout
	if not _check(mirror.view.live_pose.is_empty(), "The reflection must expire when landmarks stop arriving."):
		return
	mirror.landmarks_received(frame)
	mirror.connection(false)
	if not _check(mirror.view.live_pose.is_empty(), "Disconnect must clear the reflection."):
		return
	room._close_mirror()
	if not _check(not mirror.visible and room._panel.visible, "Closing the mirror must return to the setup controls."):
		return
	print("Bansayo pose mirror smoke passed")
	quit()
