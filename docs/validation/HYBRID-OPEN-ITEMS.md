# Hybrid tracking — open items needing hardware

Tracker for checklist items ([HYBRID-OWNER-CHECKLIST.md](HYBRID-OWNER-CHECKLIST.md)) that cannot
be closed with the hardware on hand (one UGREEN webcam). Update the row and link the dated run
record when an item is run. Last updated 19 November 2026.

| Item | Blocked on | What to do when available | Status |
|---|---|---|---|
| C-08 two identical cameras | A second UGREEN webcam (both report USB serial `SN0001`; Windows may not tell them apart) | Plug both, configure only by `hardware_id` (expect refusal), then by `interface_id` per camera; swap plug order and reboot | Open |
| C-09 moved camera | Two cameras calibrated into one rig (`scripts/calibrate_cameras.py`) | Move one camera ~20 cm; note reprojection errors and rejected views; extrinsics are not auto-invalidated (known gap) | Open |
| S-01 – S-06 SlimeVR | SlimeVR Server installed and running, trackers charged and paired | Start server, run engine with `config/hybrid-slimevr.yaml` adapted; follow S-01 to S-06 | Open |
| H-01 – H-07 hybrid | Calibrated cameras plus SlimeVR trackers on pelvis, thighs, shins | Front and roundhouse kicks, occlusion, 10 min drift session, replay per `tracking.mode` | Open |
| H-05 reference | A 120 fps reference camera (phone slow motion or fast USB camera) | Film H-02 with it and compare knee angles at publication time | Open |
| E-01 – E-02 ESP32 | Firmware sending `bansayo.imu.v1` (none in this repository) | Flash, list the board under `esp32_udp.trackers`, reboot the board once | Open |
| Release-build rates | A release build allowed by Bitdefender webcam protection | Repeat C-03 with the full pipeline; debug build infers at ~13 Hz | Open |

## Notes for whoever runs these

- Bitdefender webcam protection blocks each new `bansayo-engine.exe` build until it is allowed
  (symptom: `Access is denied`, 0x80070005, while ffmpeg still reads the camera).
- Keep the paging file large or reboot after long test sessions: with low free commit the engine
  fails frame allocations (`memory allocation of 2764800 bytes failed`).
