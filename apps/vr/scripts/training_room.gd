extends Node3D
const EngineClient = preload("res://scripts/engine_client.gd")
const PoseBuffer = preload("res://scripts/pose_buffer.gd")
const BodyRoot = preload("res://scripts/body_root.gd")
const DebugSkeleton = preload("res://scripts/debug_skeleton.gd")
const DevRig = preload("res://scripts/dev_rig.gd")
const StatusPanel = preload("res://scripts/status_panel.gd")
const EngineActions = preload("res://scripts/engine_actions.gd")
const Clinic = preload("res://scripts/clinic.gd")
const ALIGNMENT_FILE := "user://xr_alignment.cfg"
var _buffer := PoseBuffer.new()
var _body := DebugSkeleton.new()
var _dev := DevRig.new()
var _panel := StatusPanel.new()
var _actions := EngineActions.new()
var _clinic := Clinic.new()
var _root: Node3D
var _xr_interface: XRInterface
var _xr_camera: XRCamera3D
var _right_hand: XRController3D

func _ready() -> void:
	_build_room()
	_root = BodyRoot.new()
	_root.name = "BodyRoot"
	add_child(_root)
	_root.add_child(_body)
	_load_alignment()
	add_child(_dev)
	add_child(_panel)
	_panel.position = Vector3(-2.3, 1.7, -0.6)
	var client := EngineClient.new()
	client.hello_received.connect(func(_message: Dictionary): _buffer.clear())
	client.skeleton_changed.connect(_body.build)
	client.skeleton_changed.connect(_panel.skeleton_changed)
	client.skeleton_changed.connect(_clinic.build)
	client.training_changed.connect(_clinic.stream_state_changed)
	client.training_event.connect(_clinic.event_received)
	client.state_changed.connect(_panel.state_changed)
	client.state_changed.connect(_actions.set_state)
	client.state_changed.connect(_clinic.engine_state)
	client.frame_received.connect(func(frame: Dictionary):
		_buffer.push(frame)
		_panel.frame_received(frame)
	)
	client.connection_changed.connect(func(connected: bool, message: String):
		_panel.connection(connected, message)
		_clinic.connection(connected)
		if not connected:
			_buffer.clear()
	)
	add_child(client)
	_actions.engine_url = client.engine_url
	_actions.message.connect(_panel.set_action)
	_actions.message.connect(_clinic.set_notice)
	add_child(_actions)
	_clinic.actions = _actions
	_clinic.camera = _dev.camera
	_clinic.world_frame = _root
	add_child(_clinic)
	_initialise_xr()

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo:
		if _clinic.opened:
			match event.keycode:
				KEY_TAB: _clinic.next()
				KEY_ENTER: _clinic.select()
			return
		match event.keycode:
			KEY_T: _clinic.show_menu()
			KEY_R: _actions.toggle_recording()
			KEY_P: _actions.toggle_replay()
			KEY_C: _reset_counts()

func _on_button(name: String, controller: XRController3D) -> void:
	if _clinic.opened:
		match name:
			"ax_button": _clinic.next()
			"by_button": _clinic.select()
			"primary_click":
				if controller == _right_hand and not _clinic._walls.opened and _clinic._phase() in ["setup", "preparing", "ready"]:
					_align_to(controller)
		return
	match name:
		"ax_button": _actions.toggle_recording()
		"by_button": _actions.toggle_replay()
		"primary_click":
			if controller == _right_hand:
				_align_to(controller)
			else:
				_clinic.show_menu()

func _reset_counts() -> void:
	_panel.counter.reset()
	_panel.set_action("Counts reset")

## The calibrated camera world is the floor board's frame. With the right controller resting on
## the board origin and pointing along the board's +X, engine world (0, 0, 0) and +X map onto
## it; the headset's floor height is kept, so only position on the floor and yaw are taken.
func _align_to(controller: XRController3D) -> void:
	if not controller.get_has_tracking_data():
		_panel.set_action("Alignment needs a tracked controller")
		return
	var forward := -controller.global_transform.basis.z
	forward.y = 0.0
	if forward.length() < 0.1:
		_panel.set_action("Alignment needs the controller held level")
		return
	forward = forward.normalized()
	var yaw := atan2(-forward.z, forward.x)
	var position := controller.global_position
	_root.transform = Transform3D(Basis(Vector3.UP, yaw), Vector3(position.x, 0.0, position.z))
	_clinic.aligned = true
	var config := ConfigFile.new()
	config.set_value("alignment", "transform", _root.transform)
	config.save(ALIGNMENT_FILE)
	_panel.alignment_text = "XR alignment: controller on board origin (saved)"
	_panel.set_action("Room aligned to the calibration board")

func _load_alignment() -> void:
	var config := ConfigFile.new()
	if config.load(ALIGNMENT_FILE) == OK and not OS.get_cmdline_user_args().has("--desktop"):
		_root.transform = config.get_value("alignment", "transform", Transform3D.IDENTITY)
		_panel.alignment_text = "XR alignment: controller on board origin (loaded)"

func _process(_delta: float) -> void:
	var pose := _buffer.sample()
	_body.apply_pose(pose)
	_clinic.pose_available = not pose.is_empty()
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
	if _xr_interface.supports_play_area_mode(XRInterface.XR_PLAY_AREA_STAGE):
		_xr_interface.set_play_area_mode(XRInterface.XR_PLAY_AREA_STAGE)
	var origin := XROrigin3D.new()
	origin.position = Vector3(0, 0, 3)
	add_child(origin)
	_xr_camera = XRCamera3D.new()
	origin.add_child(_xr_camera)
	_xr_camera.current = true
	_clinic.xr = _xr_interface
	_clinic.camera = _xr_camera
	for hand in ["left_hand", "right_hand"]:
		var controller := XRController3D.new()
		controller.tracker = StringName(hand)
		origin.add_child(controller)
		controller.button_pressed.connect(_on_button.bind(controller))
		if hand == "right_hand":
			_right_hand = controller
			_clinic.right_hand = controller
	get_viewport().use_xr = true
	_dev.enabled = false
	if _xr_interface.has_signal("session_stopping"):
		_xr_interface.connect("session_stopping", _return_to_desktop)
		_xr_interface.connect("session_focussed", func(): _clinic.xr_focused = true)
		_xr_interface.connect("session_visible", _xr_focus_lost)
		_xr_interface.connect("session_loss_pending", _xr_focus_lost)
		_xr_interface.connect("pose_recentered", func():
			_clinic.aligned = false
			_clinic.suspend()
		)

func _xr_focus_lost() -> void:
	_clinic.xr_focused = false
	_clinic.suspend()

func _return_to_desktop() -> void:
	_clinic.suspend()
	_clinic.xr = null
	_clinic.aligned = false
	_clinic.camera = _dev.camera
	get_viewport().use_xr = false
	_root.transform = Transform3D.IDENTITY # Alignment maps the headset stage, not the desktop view.
	_panel.alignment_text = "XR alignment: assumed identity"
	_dev.enabled = true
	_dev.camera.current = true
	_xr_interface = null
