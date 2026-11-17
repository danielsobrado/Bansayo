extends Node3D
# Engine content only. The native headset camera is never parented under this transform.
# Its transform is T_xr_stage_from_world: identity until the room is aligned in the headset
# (training_room.gd, controller on the calibration board origin).
