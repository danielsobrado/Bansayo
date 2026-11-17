extends SceneTree
const PoseBuffer = preload("res://scripts/pose_buffer.gd")
const MovementCounter = preload("res://scripts/movement_counter.gd")

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
	assert(is_equal_approx(counter.max_ankle_height_m, 0.7))
	for hip in [0.0, 85.0, 5.0]: # a squat flexes both hips: not a kick
		counter.update({"right_hip_flexion": hip, "left_hip_flexion": hip}, {})
	assert(counter.kicks["right"] == 1 and counter.kicks["left"] == 0)
	print("Bansayo Godot contract smoke passed")
	quit(0)
