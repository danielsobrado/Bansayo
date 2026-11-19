# Hybrid owner checklist — run 2, 19 November 2026

Checklist: [HYBRID-OWNER-CHECKLIST.md](HYBRID-OWNER-CHECKLIST.md). Hardware: UGREEN Camera
(`USB\VID_0C45&PID_6369`), one unit, debug build of the engine. Config: one camera,
`hardware_id: "vid_0c45&pid_6369"`, `preset: fast_motion`, `fallback_fps: [60, 30]`.

| Id | Result | Observed |
|---|---|---|
| C-01 | **Pass** | `--list-cameras`: fingerprint `usb:0c45:6369:usb#vid_0c45&pid_6369&mi_00#7&5d58c34&1&0000`, `hardware_id vid_0c45&pid_6369`, serial `(none exposed)`, 72 modes, none at `0 fps`. The USB parent reports serial `SN0001`, a generic value the identity code does not use |
| C-02 | **Pass** | Opened by `hardware_id`; lifecycle `active`, identity confirmed, profile written to `data/cameras/CAM-FRONT.json` with `last_mode` 1280×720 MJPEG 30 |
| C-03 | **Pass** | Requested and negotiated `MJPEG 1280×720 @ 30` with `camera_mode_not_as_requested: 120 fps is not offered; using fallback 30 fps`; measured 30.05 fps, interval mean 33.3 ms, p95 47.9 ms, p99 48.3 ms, jitter 4.3 ms |
| C-04 | **Pass** (by exposure, not by covering the lens) | `exposure_s: 0.1` (driver step 0.125 s) → measured 15.0 fps, lifecycle `degraded`, `camera_rate_below_requested: Requested 30 fps, measured 15.0 fps` |
| C-05 | **Pass** | `exposure_s: 0.004` applied as 0.0039 s (1/256); 30.0 fps |
| C-06 | **Pass** (after a reboot) | Unplugged while streaming: `Could not capture frame` at 14:53:17, `no connected camera matches the configured identity` while out, reopened 14:53:33; reconnects 1, identity confirmed, 30 fps, no crash; private memory flat at ~922 MB through the unplug (108 samples at 0.5 s). The webcam keeps the same interface id on any port: Windows keys its composite parent on the device's serial `SN0001` |
| C-07 | **Pass after fixes** | Profile fingerprint edited (file saved by PowerShell, UTF-8 with BOM) → `camera_identity_changed`, identity unconfirmed, 2D continues; `POST /cameras/CAM-FRONT/identity/confirm` → confirmed and still confirmed 4 s later |
| C-08, C-09 | Not run | Need a second identical camera / a calibrated rig |

Debug-build inference ran at 13.5 Hz, so 325 frames were replaced before inference in 10 s;
a release build is needed for rate checks of the full pipeline.

## Defects found and fixed in this run

1. A confirmed identity was overwritten by the capture thread's next status publish, so the
   confirmation never stuck. Now the thread re-reads the profile while unconfirmed.
2. A device profile saved with a UTF-8 byte-order mark (Notepad, PowerShell 5) was read as invalid.
   The loader now accepts it.
3. A camera's informational mode note hid the generic "rate below nominal" reason, and the camera
   rate was compared with the preset's 120 fps instead of the 30 fps mode in use. Both fixed.
4. `exposure_automatic` claimed the camera was in automatic exposure; after a session that set
   manual exposure, the camera keeps it (8.1 fps observed). The message now says the camera keeps
   its own setting. The webcam was returned to automatic exposure after this run.

## Environment findings

- Before a reboot the engine crashed twice (`memory allocation of 2764800 bytes failed`, one
  1280×720 RGB frame) with 26 GB of RAM free but only ~2.3 GB of commit free (small paging file,
  many test runs). After the reboot (73 GB commit free) the same unplug passed with flat memory.
- After rebuilding `bansayo-engine.exe`, Media Foundation returned `Access is denied`
  (0x80070005) while ffmpeg read the camera through DirectShow. Cause: **Bitdefender webcam
  protection** blocks each new executable until allowed. The `camera_unavailable` reason now names
  privacy settings and antivirus webcam protection for this error code.
