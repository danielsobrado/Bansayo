extends SceneTree
const PoseBuffer = preload("res://scripts/pose_buffer.gd")

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
	print("Kinexa Godot contract smoke passed")
	quit(0)
