extends RefCounted
## Authored 2D examples for the bundled exercises. Coordinates are relative to the hips.
const WORKOUT := "general.workout.full_body_in_20"
const FRONT_KICK := "taekwondo.ap_chagi"
const ROUNDHOUSE := "taekwondo.dollyeo_chagi"
const NEUTRAL := {
	"head": Vector2(0, -0.55),
	"left_shoulder": Vector2(0.15, -0.38), "right_shoulder": Vector2(-0.15, -0.38),
	"left_elbow": Vector2(0.19, -0.19), "right_elbow": Vector2(-0.19, -0.19),
	"left_wrist": Vector2(0.20, -0.02), "right_wrist": Vector2(-0.20, -0.02),
	"left_hip": Vector2(0.10, 0), "right_hip": Vector2(-0.10, 0),
	"left_knee": Vector2(0.11, 0.27), "right_knee": Vector2(-0.11, 0.27),
	"left_ankle": Vector2(0.12, 0.53), "right_ankle": Vector2(-0.12, 0.53),
}
const WORKOUT_ITEMS := {
	"march": "March", "side_steps": "Side steps", "arm_swings": "Step and swing",
	"reach": "Step and reach", "knees": "Alternating knee lifts", "squat": "Squat",
	"push": "Wall push-up", "hinge": "Hip hinge", "cross": "Standing cross-crawl",
	"sweep": "Side step with arm sweeps", "walk": "Easy walk", "shoulders": "Shoulder circles",
	"calves": "Calf movements",
}
const FLEXIBILITY := {"reach": "Overhead reach", "side_bend": "Side bend", "calf_stretch": "Calf stretch"}
const TECHNIQUES := {FRONT_KICK: "Front kick", ROUNDHOUSE: "Roundhouse kick"}
const KNEE_LIFT := {"left_knee": Vector2(0.24, -0.05), "left_ankle": Vector2(0.28, 0.20)}
const SIDE_STEP := {"left_knee": Vector2(0.24, 0.25), "left_ankle": Vector2(0.37, 0.49)}

static func for_session(state: Dictionary) -> Dictionary:
	if state.get("phase") in ["completed", "aborted"]:
		return {}
	var item: Variant = state.get("current")
	if not item is Dictionary:
		return {}
	var exercise: Dictionary = item.get("exercise", {})
	var reference := str(exercise.get("ref", ""))
	var id := reference.get_slice("@", 0)
	var revision := reference.get_slice("@", 1)
	if not revision.is_empty() and revision != "1":
		return {}
	var phase := str(item.get("phase_id")) if item.get("phase_id") is String else ""
	var left: bool = exercise.get("side") != "right"
	if id in TECHNIQUES:
		if state.get("phase") == "resting":
			phase = "ready"
		return technique(id, phase, left)
	if id == WORKOUT:
		return workout(str(item.get("item_id", "")))
	return {}

static func technique(id: String, phase: String, left: bool) -> Dictionary:
	if not TECHNIQUES.has(id):
		return {}
	var points := NEUTRAL.duplicate()
	points.merge({"left_elbow": Vector2(0.20, -0.20), "right_elbow": Vector2(-0.20, -0.21),
		"left_wrist": Vector2(0.10, -0.38), "right_wrist": Vector2(-0.12, -0.40)}, true)
	var label := phase.capitalize()
	match phase:
		"chamber", "retraction":
			points.merge(KNEE_LIFT, true)
			if id == ROUNDHOUSE:
				points["left_ankle"] = Vector2(0.06, 0.04)
		"extension":
			points["left_knee"] = Vector2(0.33, -0.05)
			points["left_ankle"] = Vector2(0.59, -0.11 if id == FRONT_KICK else -0.04)
			if id == ROUNDHOUSE:
				points["head"] = Vector2(-0.14, -0.51)
				points["left_shoulder"] = Vector2(0.07, -0.36)
				points["right_shoulder"] = Vector2(-0.23, -0.32)
		"recovery":
			points["left_ankle"] = Vector2(0.22, 0.48)
		_:
			label = "Ready stance"
	if not left:
		points = _other_side(points)
	return {"title": TECHNIQUES[id], "caption": label + (" · Left" if left else " · Right"), "points": points}

static func workout(id: String) -> Dictionary:
	if not WORKOUT_ITEMS.has(id):
		return {}
	var points := NEUTRAL.duplicate()
	var caption := "Example position"
	match id:
		"march", "walk", "knees", "cross":
			points.merge(KNEE_LIFT, true)
			if id == "cross":
				points["right_elbow"] = Vector2(-0.10, -0.19)
				points["right_wrist"] = Vector2(0.16, -0.07)
		"side_steps", "reach", "sweep":
			points.merge(SIDE_STEP, true)
			if id == "reach":
				points["left_elbow"] = Vector2(0.25, -0.54)
				points["left_wrist"] = Vector2(0.29, -0.72)
			elif id == "sweep":
				points["left_elbow"] = Vector2(0.34, -0.34)
				points["left_wrist"] = Vector2(0.49, -0.32)
		"arm_swings", "shoulders":
			points["left_elbow"] = Vector2(0.28, -0.24)
			points["left_wrist"] = Vector2(0.41, -0.14)
		"squat":
			points.merge({"left_knee": Vector2(0.21, 0.15), "right_knee": Vector2(-0.21, 0.15),
				"left_ankle": Vector2(0.23, 0.38), "right_ankle": Vector2(-0.23, 0.38),
				"left_wrist": Vector2(0.27, -0.27), "right_wrist": Vector2(-0.27, -0.27)}, true)
			caption = "Lowered position · Front view"
		"push", "hinge":
			points = _profile(id)
			caption = "Example position · Side view"
		"calves":
			points["left_ankle"] = Vector2(0.12, 0.45)
			points["right_ankle"] = Vector2(-0.12, 0.45)
			caption = "Heel raise"
	return {"title": WORKOUT_ITEMS[id], "caption": caption, "points": points}

static func flexibility(id: String) -> Dictionary:
	if not FLEXIBILITY.has(id):
		return {}
	var points := NEUTRAL.duplicate()
	var caption := "Illustrated hold · Front view"
	match id:
		"reach", "side_bend":
			points.merge({"left_elbow": Vector2(0.20, -0.56), "right_elbow": Vector2(-0.20, -0.56),
				"left_wrist": Vector2(0.22, -0.73), "right_wrist": Vector2(-0.22, -0.73)}, true)
			if id == "side_bend":
				points["head"] += Vector2(-0.15, 0)
				points["right_shoulder"] += Vector2(-0.10, 0.05)
				points["left_shoulder"] += Vector2(-0.10, -0.04)
				points["left_elbow"] = Vector2(-0.05, -0.56)
				points["left_wrist"] = Vector2(-0.21, -0.68)
				points["right_elbow"] = Vector2(-0.28, -0.16)
				points["right_wrist"] = Vector2(-0.23, 0.02)
		"calf_stretch":
			points = _profile(id)
			caption = "Illustrated hold · Side view"
	return {"title": FLEXIBILITY[id], "caption": caption, "points": points}

static func _profile(id: String) -> Dictionary:
	var points := NEUTRAL.duplicate()
	points.merge({"head": Vector2(0.22, -0.49),
		"left_shoulder": Vector2(0.16, -0.35), "right_shoulder": Vector2(0.10, -0.33),
		"left_wrist": Vector2(0.36, -0.29), "right_wrist": Vector2(0.32, -0.29),
		"left_elbow": Vector2(0.17, -0.26), "right_elbow": Vector2(0.13, -0.25),
		"left_hip": Vector2(0.02, 0), "right_hip": Vector2(-0.04, 0),
		"left_knee": Vector2(0.07, 0.25), "right_knee": Vector2(-0.02, 0.26),
		"left_ankle": Vector2(0.02, 0.52), "right_ankle": Vector2(-0.03, 0.53)}, true)
	if id == "hinge":
		points["head"] = Vector2(0.44, -0.31)
		points["left_shoulder"] = Vector2(0.33, -0.21)
		points["right_shoulder"] = Vector2(0.27, -0.21)
		points["left_elbow"] = Vector2(0.31, -0.03)
		points["right_elbow"] = Vector2(0.25, -0.03)
		points["left_wrist"] = Vector2(0.27, 0.12)
		points["right_wrist"] = Vector2(0.21, 0.12)
	elif id == "calf_stretch":
		points["left_knee"] = Vector2(0.17, 0.23)
		points["left_ankle"] = Vector2(0.13, 0.48)
		points["right_knee"] = Vector2(-0.15, 0.23)
		points["right_ankle"] = Vector2(-0.27, 0.49)
	return points

static func _other_side(points: Dictionary) -> Dictionary:
	var mirrored := {}
	for name: String in points:
		var point: Vector2 = points[name]
		var other := name
		if name.begins_with("left_"):
			other = "right_" + name.trim_prefix("left_")
		elif name.begins_with("right_"):
			other = "left_" + name.trim_prefix("right_")
		mirrored[other] = Vector2(-point.x, point.y)
	return mirrored
