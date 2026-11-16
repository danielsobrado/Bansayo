extends Node3D
const EngineClient = preload("res://scripts/engine_client.gd")
const PoseBuffer = preload("res://scripts/pose_buffer.gd")
const BodyRoot = preload("res://scripts/body_root.gd")
const DebugSkeleton = preload("res://scripts/debug_skeleton.gd")
const DevRig = preload("res://scripts/dev_rig.gd")
const StatusPanel = preload("res://scripts/status_panel.gd")
var _buffer := PoseBuffer.new()
var _body := DebugSkeleton.new()
var _dev := DevRig.new()
var _panel := StatusPanel.new()
var _xr_interface: XRInterface
var _xr_camera: XRCamera3D

func _ready() -> void:
	_build_room()
	var root := BodyRoot.new()
	root.name = "BodyRoot"
	add_child(root)
	root.add_child(_body)
	add_child(_dev)
	add_child(_panel)
	_panel.position = Vector3(-1.55, 1.6, -0.25)
	var client := EngineClient.new()
	client.hello_received.connect(func(_message: Dictionary): _buffer.clear())
	client.skeleton_changed.connect(_body.build)
	client.state_changed.connect(_panel.state_changed)
	client.frame_received.connect(func(frame: Dictionary):
		_buffer.push(frame)
		_panel.frame_received(frame)
	)
	client.connection_changed.connect(func(connected: bool, message: String):
		_panel.connection(connected, message)
		if not connected:
			_buffer.clear()
	)
	add_child(client)
	_initialise_xr()

func _process(_delta: float) -> void:
	var pose := _buffer.sample()
	_body.apply_pose(pose)
	_panel.set_gap(pose.is_empty())
	if _xr_interface and not _xr_interface.is_initialized():
		_return_to_desktop()

func _build_room() -> void:
	var environment := WorldEnvironment.new()
	var settings := Environment.new()
	settings.background_mode = Environment.BG_COLOR
	settings.background_color = Color(0.035, 0.045, 0.06)
	settings.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	settings.ambient_light_color = Color(0.7, 0.75, 0.85)
	settings.ambient_light_energy = 0.7
	environment.environment = settings
	add_child(environment)
	var light := DirectionalLight3D.new()
	light.rotation_degrees = Vector3(-35, -30, 0)
	light.light_energy = 1.4
	add_child(light)
	var floor_mesh := MeshInstance3D.new()
	var plane := PlaneMesh.new()
	plane.size = Vector2(12, 12)
	floor_mesh.mesh = plane
	var material := StandardMaterial3D.new()
	material.albedo_color = Color(0.055, 0.07, 0.09)
	material.roughness = 1
	floor_mesh.material_override = material
	add_child(floor_mesh)
	var grid_mesh := ImmediateMesh.new()
	var grid_material := StandardMaterial3D.new()
	grid_material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	grid_material.albedo_color = Color(0.15, 0.19, 0.23)
	grid_mesh.surface_begin(Mesh.PRIMITIVE_LINES, grid_material)
	for index in range(-10, 11):
		var coordinate := float(index) * 0.5
		grid_mesh.surface_add_vertex(Vector3(coordinate, 0.002, -5))
		grid_mesh.surface_add_vertex(Vector3(coordinate, 0.002, 5))
		grid_mesh.surface_add_vertex(Vector3(-5, 0.002, coordinate))
		grid_mesh.surface_add_vertex(Vector3(5, 0.002, coordinate))
	grid_mesh.surface_end()
	var grid := MeshInstance3D.new()
	grid.mesh = grid_mesh
	add_child(grid)
	for axis in ["+X", "+Z"]:
		var label := Label3D.new()
		label.text = axis + " · 1 m"
		label.position = Vector3(1, 0.03, 0) if axis == "+X" else Vector3(0, 0.03, 1)
		label.rotation_degrees.x = -90
		label.font_size = 24
		label.pixel_size = 0.003
		add_child(label)

func _initialise_xr() -> void:
	if OS.get_cmdline_user_args().has("--desktop"):
		return
	_xr_interface = XRServer.find_interface("OpenXR")
	if not _xr_interface or not _xr_interface.initialize():
		_xr_interface = null
		return
	var origin := XROrigin3D.new()
	origin.position = Vector3(0, 0, 3)
	add_child(origin)
	_xr_camera = XRCamera3D.new()
	origin.add_child(_xr_camera)
	_xr_camera.current = true
	for hand in ["left_hand", "right_hand"]:
		var controller := XRController3D.new()
		controller.tracker = StringName(hand)
		origin.add_child(controller)
	get_viewport().use_xr = true
	_dev.enabled = false
	if _xr_interface.has_signal("session_stopping"):
		_xr_interface.connect("session_stopping", _return_to_desktop)

func _return_to_desktop() -> void:
	get_viewport().use_xr = false
	_dev.enabled = true
	_dev.camera.current = true
	_xr_interface = null
