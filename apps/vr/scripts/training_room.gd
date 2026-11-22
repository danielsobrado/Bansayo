extends Node3D
const EngineClient = preload("res://scripts/engine_client.gd")
const PoseBuffer = preload("res://scripts/pose_buffer.gd")
const BodyRoot = preload("res://scripts/body_root.gd")
const AvatarRenderer = preload("res://scripts/avatar_renderer.gd")
const DevRig = preload("res://scripts/dev_rig.gd")
const StatusPanel = preload("res://scripts/status_panel.gd")
const EngineActions = preload("res://scripts/engine_actions.gd")
const ComfortSettings = preload("res://scripts/comfort_settings.gd")
const VrInputRouter = preload("res://scripts/vr_input_router.gd")
const PoseMirror = preload("res://scripts/pose_mirror.gd")
const Clinic = preload("res://scripts/clinic.gd")
const XrPoseCapture = preload("res://scripts/xr_pose_capture.gd")
const XrPoseUplink = preload("res://scripts/xr_pose_uplink.gd")
const ALIGNMENT_FILE := "user://xr_alignment.cfg"
const STUDIO_DARK := [Color("#202a35"), Color("#273543"), Color("#344150"), Color("#5d7880"), Color("#121a25")]
const STUDIO_LIGHT := [Color("#958979"), Color("#668082"), Color("#cbbba3"), Color("#49686a"), Color("#c4cbd0")]
var _buffer := PoseBuffer.new()
var _body := AvatarRenderer.new()
var _dev := DevRig.new()
var _panel := StatusPanel.new()
var _actions := EngineActions.new()
var _clinic := Clinic.new()
var _mirror := PoseMirror.new()
var _input_router := VrInputRouter.new()
var _comfort := ComfortSettings.new()
var _room_materials: Array[StandardMaterial3D] = []
var _room_environment := Environment.new()
var _xr_origin: XROrigin3D
var _root: Node3D
var _xr_interface: XRInterface
var _xr_camera: XRCamera3D
var _has_xr_focus := false
var _right_hand: XRController3D
## Hidden grip_pose controllers for the engine uplink only; UI keeps the aim_pose controllers.
var _grips: Array[XRController3D] = []
var _xr_uplink: XrPoseUplink
var _engine_url := ""

func _ready() -> void:
	_build_room()
	_root = BodyRoot.new()
	_root.name = "BodyRoot"
	add_child(_root)
	_root.add_child(_body)
	add_child(_dev)
	add_child(_panel)
	_panel.panel.selected.connect(_on_setup_option)
	add_child(_input_router)
	var client := EngineClient.new()
	client.hello_received.connect(func(_message: Dictionary): _buffer.clear())
	client.skeleton_changed.connect(_body.build)
	client.skeleton_changed.connect(_panel.skeleton_changed)
	client.skeleton_changed.connect(_clinic.build)
	client.training_changed.connect(_clinic.stream_state_changed)
	client.training_changed.connect(_mirror.training_changed)
	client.landmarks_received.connect(_mirror.landmarks_received)
	client.state_changed.connect(_mirror.engine_state)
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
		_mirror.connection(connected)
		if not connected:
			_buffer.clear()
	)
	add_child(client)
	_engine_url = client.engine_url
	_actions.engine_url = client.engine_url
	_actions.message.connect(_panel.set_action)
	_actions.message.connect(_clinic.set_notice)
	add_child(_actions)
	_clinic.actions = _actions
	_clinic.camera = _dev.camera
	_clinic.world_frame = _root
	add_child(_clinic)
	_clinic.closed.connect(_show_setup)
	_clinic.mirror_requested.connect(_open_mirror)
	_clinic.controls_required.connect(_close_mirror)
	add_child(_mirror)
	_mirror.closed.connect(_close_mirror)
	_comfort.load_saved()
	_apply_preferences()
	_initialise_xr()
	if _xr_interface == null:
		_show_setup()

func _show_setup() -> void:
	var camera: Camera3D = _xr_camera if _xr_interface else _dev.camera
	_panel.panel.recenter(camera, 1.4 if _xr_interface else 2.0)
	_panel.show()
	_input_router.set_panel(_panel.panel)

func _on_setup_option(index: int) -> void:
	if not _panel.preferences.is_empty():
		_change_preference(index)
	elif _panel.diagnostics:
		match index:
			0: _panel.set_diagnostics(false)
			1: _actions.toggle_recording()
			2: _actions.toggle_replay()
			3: _reset_counts()
			4: _show_setup()
	else:
		match index:
			0: _open_clinic()
			1: _open_mirror()
			2: _panel.set_diagnostics(true)
			3: _panel.set_preferences(_comfort.options())
			4: _show_setup()
			5: _panel.hide()

func _change_preference(index: int) -> void:
	match index:
		0: _comfort.scale_index = (_comfort.scale_index + 1) % ComfortSettings.SCALE.size()
		1: _comfort.height_index = (_comfort.height_index + 1) % ComfortSettings.HEIGHT.size()
		2: _comfort.studio_light = not _comfort.studio_light
		3: _comfort.mannequin = not _comfort.mannequin
		4: _comfort.audio = not _comfort.audio
		5:
			_panel.set_preferences([])
			return
	_apply_preferences()
	if index in [0, 1]:
		_show_setup()
	_panel.set_action("" if _comfort.save() == OK else "Could not save VR preferences.")
	_panel.set_preferences(_comfort.options())

func _apply_preferences() -> void:
	for panel in [_panel.panel, _clinic.panel, _mirror.panel]:
		panel.size_multiplier = ComfortSettings.SCALE[_comfort.scale_index]
		panel.height_offset = ComfortSettings.HEIGHT[_comfort.height_index]
	_body.set_mannequin(_comfort.mannequin)
	_input_router.audio_enabled = _comfort.audio
	_clinic._target.audio_enabled = _comfort.audio
	_clinic._walls.audio_enabled = _comfort.audio
	var colors := STUDIO_LIGHT if _comfort.studio_light else STUDIO_DARK
	for i in range(_room_materials.size()):
		_room_materials[i].albedo_color = colors[i]
	_room_environment.background_color = colors[4]

func _open_clinic() -> void:
	_panel.hide()
	_clinic.show_menu()
	_input_router.set_panel(_clinic.panel)

func _open_mirror() -> void:
	if _mirror.visible or _clinic._walls.opened:
		return
	var camera: Camera3D = _xr_camera if _xr_interface else _dev.camera
	_panel.hide()
	_clinic.set_mirror_open(true)
	_mirror.open(camera, _xr_interface != null)
	_input_router.set_panel(_mirror.panel)

func _close_mirror() -> void:
	if not _mirror.visible:
		return
	_mirror.hide()
	_clinic.set_mirror_open(false)
	if _clinic.opened:
		_input_router.set_panel(_clinic.panel)
	else:
		_show_setup()

func _toggle_menu() -> void:
	if _mirror.visible:
		_close_mirror()
	elif _clinic.opened:
		_clinic.toggle_controls()
	elif _panel.visible:
		if not _panel.preferences.is_empty():
			_panel.set_preferences([])
		elif _panel.diagnostics:
			_panel.set_diagnostics(false)
		else:
			_panel.hide()
	else:
		_show_setup()

func _input(event: InputEvent) -> void:
	if _xr_interface == null and _input_router.desktop_input(event, _dev.camera):
		get_viewport().set_input_as_handled()

func _unhandled_input(event: InputEvent) -> void:
	if event is InputEventKey and event.pressed and not event.echo:
		if _mirror.visible:
			if event.keycode in [KEY_M, KEY_ESCAPE]:
				_close_mirror()
			return
		if event.keycode == KEY_M:
			_open_mirror()
			return
		if _clinic.opened:
			match event.keycode:
				KEY_TAB: _clinic.next()
				KEY_ENTER: _clinic.select()
				KEY_ESCAPE, KEY_T: _toggle_menu()
			return
		match event.keycode:
			KEY_T: _open_clinic()
			KEY_ESCAPE: _toggle_menu()
			KEY_D:
				_panel.set_preferences([])
				_panel.set_diagnostics(true)
				_show_setup()
			KEY_R: _actions.toggle_recording()
			KEY_P: _actions.toggle_replay()
			KEY_C: _reset_counts()

func _on_button(action: String, controller: XRController3D) -> void:
	if _xr_interface == null or not _clinic.xr_focused:
		return
	match action:
		"trigger_click", "grip_click":
			var blocked := "Place target & start" if controller == _right_hand else ""
			_input_router.handle_button(action, controller, blocked)
		"menu_button":
			_toggle_menu()
		"by_button":
			if _mirror.visible or _clinic.opened or _panel.visible:
				_toggle_menu()
			else:
				_show_setup()
		"ax_button":
			if not _clinic.opened and not _panel.visible:
				_actions.toggle_recording()
		"primary_click":
			if controller == _right_hand:
				if not _clinic.opened or (not _clinic._walls.opened and _clinic._phase() in ["setup", "preparing", "ready"]):
					_align_to(controller)
			elif _mirror.visible or _clinic.opened:
				_toggle_menu()
			else:
				_open_clinic()

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
	if config.load(ALIGNMENT_FILE) == OK:
		_root.transform = config.get_value("alignment", "transform", Transform3D.IDENTITY)
		_panel.alignment_text = "XR alignment: saved reference loaded · confirm with right stick"

func _process(_delta: float) -> void:
	var pose := _buffer.sample()
	_body.apply_pose(pose)
	_clinic.pose_available = not pose.is_empty()
	_panel.set_gap(pose.is_empty())
	_clinic.set_tracking(_panel.tracking_label())
	if not _mirror.visible:
		_input_router.set_panel(_clinic.panel if _clinic.opened else _panel.panel)
	_input_router.focused = _clinic.xr_focused
	if _xr_interface and not _xr_interface.is_initialized():
		_return_to_desktop()

func _build_room() -> void:
	var environment := WorldEnvironment.new()
	_room_environment.background_mode = Environment.BG_COLOR
	_room_environment.ambient_light_source = Environment.AMBIENT_SOURCE_COLOR
	_room_environment.ambient_light_color = Color(0.79, 0.84, 0.94)
	_room_environment.ambient_light_energy = 0.54
	environment.environment = _room_environment
	add_child(environment)
	var light := DirectionalLight3D.new()
	light.rotation_degrees = Vector3(-54, -28, 0)
	light.light_color = Color("#e4efff")
	light.light_energy = 0.86
	add_child(light)
	var fill := OmniLight3D.new()
	fill.position = Vector3(-2.8, 3.6, -5.5)
	fill.light_color = Color("#d5def5")
	fill.light_energy = 0.80
	fill.omni_range = 10.0
	add_child(fill)

	for i in range(STUDIO_DARK.size() - 1):
		var material := StandardMaterial3D.new()
		material.roughness = 0.88
		_room_materials.append(material)
	_surface(Vector3(16, 0.12, 16), Vector3(0, -0.064, 0), _room_materials[0])
	_surface(Vector3(5.8, 0.018, 5.8), Vector3(0, -0.01, 0), _room_materials[1])
	_surface(Vector3(16, 4.0, 0.12), Vector3(0, 2, -7.8), _room_materials[2])
	_surface(Vector3(0.12, 4.0, 16), Vector3(-7.8, 2, 0), _room_materials[2])
	for direction in [-1, 1]:
		var edge := 2.86 * float(direction)
		_surface(Vector3(5.8, 0.004, 0.018), Vector3(0, 0.001, edge), _room_materials[3])
		_surface(Vector3(0.018, 0.004, 5.8), Vector3(edge, 0.001, 0), _room_materials[3])
	_surface(Vector3(0.26, 0.004, 0.018), Vector3(0, 0.001, 0), _room_materials[3])
	_surface(Vector3(0.018, 0.004, 0.26), Vector3(0, 0.001, 0), _room_materials[3])
	for x in [-5.5, -3.0, -0.5, 2.0, 4.5, 7.0]:
		_surface(Vector3(0.07, 4.0, 0.10), Vector3(x, 2, -7.71), _room_materials[0])
	_surface(Vector3(15.8, 0.035, 0.055), Vector3(0, 2.95, -7.7), _room_materials[3])
	_surface(Vector3(15.8, 0.16, 0.08), Vector3(0, 0.08, -7.68), _room_materials[0])
	_surface(Vector3(0.08, 0.16, 15.8), Vector3(-7.68, 0.08, 0), _room_materials[0])

func _surface(size: Vector3, at: Vector3, material: StandardMaterial3D) -> void:
	var instance := MeshInstance3D.new()
	var mesh := BoxMesh.new()
	mesh.size = size
	instance.mesh = mesh
	instance.material_override = material
	instance.position = at
	add_child(instance)

func _initialise_xr() -> void:
	if OS.get_cmdline_user_args().has("--desktop"):
		return
	_xr_interface = XRServer.find_interface("OpenXR")
	if not _xr_interface or not _xr_interface.initialize():
		_xr_interface = null
		return
	if _xr_interface.supports_play_area_mode(XRInterface.XR_PLAY_AREA_STAGE):
		_xr_interface.set_play_area_mode(XRInterface.XR_PLAY_AREA_STAGE)
	_xr_origin = XROrigin3D.new()
	_xr_origin.position = Vector3(0, 0, 3)
	add_child(_xr_origin)
	_panel.hide()
	_load_alignment()
	_xr_camera = XRCamera3D.new()
	_xr_origin.add_child(_xr_camera)
	_xr_camera.current = true
	_clinic.xr = _xr_interface
	_clinic.camera = _xr_camera
	for hand in ["left_hand", "right_hand"]:
		var controller := XRController3D.new()
		controller.tracker = StringName(hand)
		controller.pose = &"aim_pose"
		_xr_origin.add_child(controller)
		_input_router.add_hand(controller)
		controller.button_pressed.connect(_on_button.bind(controller))
		if hand == "right_hand":
			_right_hand = controller
			_clinic.right_hand = controller
	for hand in ["left_hand", "right_hand"]:
		var grip := XRController3D.new()
		grip.name = hand.capitalize().replace(" ", "") + "Grip"
		grip.tracker = StringName(hand)
		grip.pose = &"grip_pose"
		_xr_origin.add_child(grip)
		_grips.append(grip)
	if XrPoseUplink.enabled():
		_xr_uplink = XrPoseUplink.new()
		_xr_uplink.configure(_engine_url)
		_xr_uplink.capture = _capture_xr
		add_child(_xr_uplink)
	else:
		print("XR uplink: disabled (bansayo/xr_uplink_enabled)")
	get_viewport().use_xr = true
	_dev.enabled = false
	if _xr_interface.has_signal("session_stopping"):
		_xr_interface.connect("session_stopping", _return_to_desktop)
		_xr_interface.connect("session_focussed", _xr_focus_gained)
		_xr_interface.connect("session_visible", _xr_focus_lost)
		_xr_interface.connect("session_loss_pending", _xr_focus_lost)
		_xr_interface.connect("pose_recentered", func():
			if _xr_uplink:
				_xr_uplink.recentered()
			_clinic.aligned = false
			_panel.alignment_text = "XR alignment: headset recentered · align again"
			_clinic.suspend()
			if _mirror.visible:
				_mirror.open(_xr_camera, true)
			elif _clinic.opened:
				_clinic.recenter()
			elif _panel.visible:
				_show_setup()
		)

## One tick's raw stage poses; a device counts as tracked only while the session is focused.
func _capture_xr() -> Array:
	var focused := _clinic.xr_focused and _xr_interface != null
	var tracked: Array[bool] = [
		focused and XrPoseCapture.head_tracked(),
		focused and _grips[0].get_has_tracking_data(),
		focused and _grips[1].get_has_tracking_data(),
	]
	return XrPoseCapture.samples(_xr_origin, _xr_camera, _grips[0], _grips[1], tracked)

func _xr_focus_gained() -> void:
	if _xr_interface == null:
		return
	if _xr_uplink:
		_xr_uplink.focused = true
	_clinic.xr_focused = true
	if not _has_xr_focus:
		_has_xr_focus = true
		_show_setup()

func _xr_focus_lost() -> void:
	if _xr_uplink:
		_xr_uplink.focused = false
	_clinic.xr_focused = false
	_clinic.suspend()

func _return_to_desktop() -> void:
	_xr_focus_lost()
	if _xr_uplink:
		_xr_uplink.stop()
		_xr_uplink.queue_free()
		_xr_uplink = null
	_input_router.focused = false
	_clinic.xr = null
	_clinic.aligned = false
	_clinic.camera = _dev.camera
	get_viewport().use_xr = false
	_root.transform = Transform3D.IDENTITY # Alignment maps the headset stage, not the desktop view.
	_panel.alignment_text = "XR alignment: assumed identity"
	_dev.enabled = true
	_dev.camera.current = true
	_xr_interface = null
	if _mirror.visible:
		_mirror.open(_dev.camera, false)
	elif _clinic.opened:
		_clinic.recenter()
	else:
		_show_setup()
