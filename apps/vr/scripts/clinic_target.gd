extends Node3D

const INK := Color(0.20, 0.95, 0.87)
const SPARK := Color(1.0, 0.79, 0.43)
var audio_enabled := true
var target_id := ""
var _rings := Node3D.new()
var _burst := Node3D.new()
var _sound := AudioStreamPlayer3D.new()
var _height := Label3D.new()
var _glow := StandardMaterial3D.new()
var _flash := StandardMaterial3D.new()
var _effect: Tween

func _ready() -> void:
	_material(_glow, INK)
	_material(_flash, SPARK)
	_flash.transparency = BaseMaterial3D.TRANSPARENCY_ALPHA
	add_child(_rings)
	add_child(_burst)
	add_child(_sound)
	_height.position = Vector3(0, -1.6, 0.02)
	_height.font_size = 32
	_height.pixel_size = 0.007
	_height.outline_size = 5
	add_child(_height)
	_rings.add_child(_arc(1.0, 0.045, 0.0, TAU, _glow))
	for i in range(4):
		var at := i * PI / 2.0
		_rings.add_child(_arc(1.26, 0.075, at + 0.18, at + 0.75, _glow))
	_rings.add_child(_arc(0.24, 0.045, 0.0, TAU, _glow))
	for i in range(16):
		var spark := ImmediateMesh.new()
		spark.surface_begin(Mesh.PRIMITIVE_TRIANGLES, _flash)
		spark.surface_add_vertex(Vector3(0.8, -0.035, 0.0))
		spark.surface_add_vertex(Vector3(1.25 + (i % 3) * 0.18, 0.0, 0.0))
		spark.surface_add_vertex(Vector3(0.8, 0.035, 0.0))
		spark.surface_end()
		var item := MeshInstance3D.new()
		item.mesh = spark
		item.rotation.z = i * TAU / 16.0
		_burst.add_child(item)
	_burst.add_child(_arc(1.0, 0.065, 0.0, TAU, _flash))
	_sound.stream = chime()
	_sound.volume_db = -12.0
	hide()

func _material(material: StandardMaterial3D, color: Color) -> void:
	material.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	material.cull_mode = BaseMaterial3D.CULL_DISABLED
	material.albedo_color = color

func _arc(radius: float, width: float, start: float, end: float, material: Material) -> MeshInstance3D:
	var mesh := ImmediateMesh.new()
	mesh.surface_begin(Mesh.PRIMITIVE_TRIANGLE_STRIP, material)
	var segments := maxi(2, ceili((end - start) * 12.0))
	for i in range(segments + 1):
		var angle := lerpf(start, end, float(i) / segments)
		var direction := Vector3(cos(angle), sin(angle), 0.0)
		mesh.surface_add_vertex(direction * (radius - width * 0.5))
		mesh.surface_add_vertex(direction * (radius + width * 0.5))
	mesh.surface_end()
	var item := MeshInstance3D.new()
	item.mesh = mesh
	item.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	return item

func present(id: String, point: Vector3, radius: float, viewer: Vector3, height_m: float) -> void:
	if visible and target_id == id and global_position.is_equal_approx(point):
		return
	target_id = id
	global_position = point
	var facing := viewer - point
	facing.y = 0
	if facing.length_squared() > 0.001:
		look_at(point + facing)
		_height.rotation.y = PI
	scale = Vector3.ONE * radius
	_height.text = "%.0f cm" % (height_m * 100.0)
	_glow.albedo_color = INK.darkened(0.25) if id == "placement-preview" else INK
	_burst.hide()
	show()

func clear() -> void:
	if _effect:
		_effect.kill()
	_sound.stop()
	target_id = ""
	hide()

func reach(id: String) -> bool:
	if not visible or id != target_id:
		return false
	if _effect:
		_effect.kill()
	_burst.scale = Vector3.ONE
	_flash.albedo_color = SPARK
	_burst.show()
	if audio_enabled:
		_sound.play()
	_effect = create_tween().set_parallel(true)
	_effect.tween_property(_burst, "scale", Vector3.ONE * 2.3, 0.26).set_trans(Tween.TRANS_CUBIC).set_ease(Tween.EASE_OUT)
	_effect.tween_property(_flash, "albedo_color", Color(SPARK, 0.0), 0.26)
	_effect.chain().tween_callback(_burst.hide)
	return true

static func chime() -> AudioStreamWAV:
	const RATE := 22050
	var pcm := PackedByteArray()
	pcm.resize(int(RATE * 0.18) * 2)
	for i in range(pcm.size() / 2):
		var t := float(i) / RATE
		var wave := (sin(TAU * 560.0 * t) + 0.45 * sin(TAU * 840.0 * t)) * exp(-28.0 * t)
		var click := 0.25 * sin(TAU * 155.0 * t) * exp(-100.0 * t)
		pcm.encode_s16(i * 2, int(14500.0 * (wave + click) * minf(1.0, t * 480.0)))
	var stream := AudioStreamWAV.new()
	stream.format = AudioStreamWAV.FORMAT_16_BITS
	stream.mix_rate = RATE
	stream.data = pcm
	return stream
