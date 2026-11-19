extends RefCounted

const PATH := "user://bansayo_vr.cfg"
const SCALE := [1.0, 1.2, 1.4]
const HEIGHT := [-0.18, -0.32, 0.0]
const HEIGHT_NAMES := ["Standard", "Lower", "Eye level"]
var scale_index := 0
var height_index := 0
var studio_light := false
var mannequin := true
var audio := true

func load_saved() -> void:
	var config := ConfigFile.new()
	if config.load(PATH) != OK:
		return
	scale_index = clampi(int(config.get_value("comfort", "scale", 0)), 0, SCALE.size() - 1)
	height_index = clampi(int(config.get_value("comfort", "height", 0)), 0, HEIGHT.size() - 1)
	studio_light = config.get_value("display", "light_studio", false) == true
	mannequin = config.get_value("display", "mannequin", true) == true
	audio = config.get_value("comfort", "audio", true) == true

func save() -> Error:
	var config := ConfigFile.new()
	config.set_value("comfort", "scale", scale_index)
	config.set_value("comfort", "height", height_index)
	config.set_value("comfort", "audio", audio)
	config.set_value("display", "light_studio", studio_light)
	config.set_value("display", "mannequin", mannequin)
	return config.save(PATH)

func options() -> Array[String]:
	return ["Panel size · %d%%" % roundi(SCALE[scale_index] * 100.0),
		"Panel height · " + HEIGHT_NAMES[height_index],
		"Environment · " + ("Light Dojo" if studio_light else "Performance Studio"),
		"Avatar · " + ("Mannequin" if mannequin else "Technical skeleton"),
		"Audio feedback · " + ("On" if audio else "Off"),
		"Back to setup"]
