# Hybrid owner checklist — run 1, 19 November 2026

Checklist: [HYBRID-OWNER-CHECKLIST.md](HYBRID-OWNER-CHECKLIST.md). Build: `aa5fc10`.

| Id | Result | Notes |
|---|---|---|
| C-01 | **Blocked** | `bansayo-engine --list-cameras` → `No cameras found`. Windows lists `UGREEN Camera`, `USB\VID_0C45&PID_6369&MI_00\7&5D58C34&1&0000`, status `Unknown` (not plugged in). The instance id is Windows-generated (contains `&`): as designed, this camera has no serial and is matched by `hardware_id: "vid_0c45&pid_6369"` or by `interface_id` (per USB port) |
| C-02 – C-09 | Not run | Need the camera plugged in |
| S-01 – S-06 | Not run | SlimeVR Server not running (nothing listening on 21110); no trackers available |
| H-01 – H-07 | Not run | Need cameras and trackers |
| E-01 – E-02 | Not run | No `bansayo.imu.v1` firmware exists |

Next run: plug the UGREEN camera in and start from C-01; start SlimeVR Server with trackers for S-01.
