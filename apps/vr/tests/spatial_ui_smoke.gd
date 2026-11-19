extends SceneTree

func _initialize() -> void:
	_run.call_deferred()

func _run() -> void:
	const PATH := "res://scripts/spatial_panel.gd"
	if not ResourceLoader.exists(PATH):
		push_error("The training room needs a ray-interactive spatial panel.")
		quit(1)
		return
	var panel: Node3D = load(PATH).new()
	var camera := Camera3D.new()
	root.add_child(camera)
	root.add_child(panel)
	camera.position = Vector3(2.0, 1.8, 3.0)
	panel.recenter(camera)
	var details: Array[String] = ["Check the clear area."]
	var options: Array[String] = ["Start", "Pause"]
	panel.present("Training", "Tracking ready", details, options, -1, false)
	await process_frame
	var center: Vector3 = panel.global_position
	var button: Vector3 = panel.button_world_point(0)
	var selections: Array[int] = []
	panel.selected.connect(func(index: int): selections.append(index))
	var ray := camera.global_position.direction_to(button)
	var good: bool = panel.press_at(panel.intersect_ray(camera.global_position, ray))
	var miss: bool = panel.press_at(panel.intersect_ray(camera.global_position, Vector3.UP))
	camera.position.x += 1.0
	details.clear()
	panel.present("Training", "Tracking lost", details, options, -1, true)
	var blocked: bool = panel.press_at(panel.intersect_ray(button + panel.global_basis.z, -panel.global_basis.z))
	if not good or miss or blocked or selections != [0] or not panel.global_position.is_equal_approx(center) or not is_equal_approx(center.y, 1.62):
		push_error("Spatial selection, readiness, or world-space placement failed.")
		quit(1)
		return
	print("Bansayo spatial UI smoke passed")
	quit()
