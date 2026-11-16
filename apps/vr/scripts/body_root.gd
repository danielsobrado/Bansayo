extends Node3D
# Engine content only. The native headset camera is never parented under this transform.
# T_xr_stage_from_world is identity until XR alignment is implemented.
func _ready() -> void:
	transform = Transform3D.IDENTITY
