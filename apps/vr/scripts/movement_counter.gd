extends RefCounted
# Session counts and peaks from the engine's joint angles and pose. Presentation only: no angle
# is computed here, repetitions are hysteresis crossings of values the engine published.

const SQUAT_DOWN_DEG := 70.0  # mean knee flexion entering the bottom of a squat
const SQUAT_UP_DEG := 25.0
const KICK_UP_DEG := 60.0  # hip flexion of the kicking leg
const KICK_DOWN_DEG := 20.0
const STANDING_HIP_MAX_DEG := 30.0  # the other leg must be near straight: a squat is not a kick
var squats := 0
var kicks := {"left": 0, "right": 0}
var peaks := {}  # joint id -> peak degrees
var max_ankle_height_m := 0.0
var min_pelvis_height_m := INF
var _in_squat := false
var _in_kick := {"left": false, "right": false}

func reset() -> void:
	squats = 0
	kicks = {"left": 0, "right": 0}
	peaks.clear()
	max_ankle_height_m = 0.0
	min_pelvis_height_m = INF
	_in_squat = false
	_in_kick = {"left": false, "right": false}

## `angles`: joint id -> degrees, only joints with a value. `heights`: metres above the floor for
## `pelvis`, `left_ankle` and `right_ankle`, only when measured or derived.
func update(angles: Dictionary, heights: Dictionary) -> void:
	for id in angles:
		peaks[id] = maxf(peaks.get(id, -INF), angles[id])
	if angles.has("left_knee_flexion") and angles.has("right_knee_flexion"):
		var knee: float = (angles["left_knee_flexion"] + angles["right_knee_flexion"]) / 2.0
		if not _in_squat and knee >= SQUAT_DOWN_DEG:
			_in_squat = true
		elif _in_squat and knee <= SQUAT_UP_DEG:
			_in_squat = false
			squats += 1
	for side in ["left", "right"]:
		var id: String = side + "_hip_flexion"
		if not angles.has(id):
			continue
		var other: String = ("right" if side == "left" else "left") + "_hip_flexion"
		var standing: bool = angles.get(other, 0.0) <= STANDING_HIP_MAX_DEG
		if not _in_kick[side] and angles[id] >= KICK_UP_DEG and standing:
			_in_kick[side] = true
		elif _in_kick[side] and angles[id] <= KICK_DOWN_DEG:
			_in_kick[side] = false
			kicks[side] += 1
	for id in ["left_ankle", "right_ankle"]:
		if heights.has(id):
			max_ankle_height_m = maxf(max_ankle_height_m, heights[id])
	if heights.has("pelvis"):
		min_pelvis_height_m = minf(min_pelvis_height_m, heights["pelvis"])
