# Hybrid tracking — owner checklist (physical validation)

Everything below needs real hardware and has **not been run**. Until a step is recorded here as
passed, its claim stays pending, whatever the synthetic results
([2026-11-19-hybrid-synthetic.md](2026-11-19-hybrid-synthetic.md)) show. Record each run as a
dated file in this folder (template: `templates/`).

Build once: `cargo build --release -p bansayo-engine` (target dir `F:\drusniel-cache\cargo-target`).

## Cameras

| Id | Step | Command or action | Expected |
|---|---|---|---|
| C-01 | Identity and modes | `bansayo-engine --list-cameras` | Each camera prints `fingerprint`, `hardware_id vid_xxxx&pid_xxxx`, `serial` (UGREEN: `(none exposed)`), `interface_id` and its modes. No mode shows `0 fps` |
| C-02 | Configure by identity | Put `hardware_id` (or `interface_id`) instead of `device:` in a local config; start the engine | Log `Camera opened`; Cameras → properties: lifecycle `Active`, Hardware = the fingerprint from C-01 |
| C-03 | Requested vs actual | Request `preset: fast_motion` on the UGREEN (30 fps max) | Requested shows `MJPEG 1280×720 @ 30 fps` with reason `camera_mode_not_as_requested`; actual ≈ 30 fps |
| C-04 | Rate degradation | Cover the lens or dim the room with automatic exposure | Actual rate falls; below 85 % of requested the camera turns `degraded` with `camera_rate_below_requested` naming both rates |
| C-05 | Exposure | `PUT /api/v1/cameras/CAM-FRONT/settings {"exposure_s":0.004}` | `exposure_s` in effect; frame rate recovers in normal light |
| C-06 | Unplug / replug | Unplug during a session, wait 5 s, plug into **another** USB port | `Reconnecting`, reconnects +1, then `Active`; identity still confirmed (no serial: a new port changes `interface_id` but a `hardware_id`-only config keeps matching); calibration states unchanged, no recalibration |
| C-07 | Identity guard | Replace the camera with a different unit of the same model (or edit `data/cameras/CAM-FRONT.json` fingerprint) | Reason `camera_identity_changed`; intrinsics/extrinsics shown `stale`; 2D preview keeps running; "This is the same camera" restores them |
| C-08 | Two identical cameras | Two cameras of one model, both configured only by `hardware_id` | Configuration refused naming both; with `interface_id` per camera they open and keep their roles after swapping plug order at a reboot |
| C-09 | Moved camera | Move one calibrated camera ~20 cm | Reprojection errors rise; triangulated points rejected from that view. Extrinsics are not auto-invalidated: recalibrate the room (known gap) |

## SlimeVR

| Id | Step | Command or action | Expected |
|---|---|---|---|
| S-01 | Server | Start SlimeVR Server with trackers on; engine with a `slimevr_solarxr` source | Log `Connected to SlimeVR Server`; one `SlimeVR tracker announced` per unlisted tracker with its key |
| S-02 | Identity | Restart the trackers in another order | Same keys, same device ids |
| S-03 | Rates | Devices → each tracker | Measured rate near the tracker's real rate (repeated snapshots are not counted); timing `receive time`; battery and signal present |
| S-04 | Assignment and mounting | Assign pelvis/thighs/shins, capture the reference pose facing the rig's +Z | Mounting `valid`; segments `derived` |
| S-05 | Heading | Do 10 front kicks in view of two calibrated cameras | Shin and thigh segments move from `imu_mounted` to `imu_optical_fused`; the avatar turns into place at ≤ 15 °/s, no jumps |
| S-06 | Reconnect | Close SlimeVR Server for 10 s, reopen | Trackers `disconnected`, notice `slimevr_unavailable`, then back to tracking without restarting the engine |

## Hybrid

| Id | Step | Expected |
|---|---|---|
| H-01 | Slow reference movement (knee flexion) camera-only vs hybrid (`tracking.mode`) | Knee angles agree within 10° when slow |
| H-02 | Fast front kick, 30 fps camera + trackers, recorded | Skeleton updates at the output rate during the kick; shin `imu_*`, root `measured` |
| H-03 | Roundhouse kick | As H-02 |
| H-04 | Occlusion: step behind a pole for 1 s | Limbs keep moving from trackers; root `measured` for ≤ 150 ms, then floor contact (`inferred`); no position-based metric during the gap |
| H-05 | 120 fps reference camera (phone slow-motion or a fast USB camera) filming H-02 | Hybrid knee angle within 10° of the reference at publication time while camera-only 30 fps is not (ADR-025 criterion) |
| H-06 | Drift: 10 min session with periodic kicks | `heading` stays converged; no `imu_camera_disagree` while straps are tight |
| H-07 | Replay H-02 with `tracking.mode` camera_only / imu_only / hybrid_auto | Each re-analysis reproduces the same result on repeat runs |

## ESP32 (only with custom firmware sending `bansayo.imu.v1`)

| Id | Step | Expected |
|---|---|---|
| E-01 | Firmware sends 64-byte datagrams to `bind` | Devices → time quality `estimated` with an uncertainty; rate ≈ firmware rate |
| E-02 | Reboot the board | `resets` +1; no interpolation across the reboot |
