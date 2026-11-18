# 2026-11-18 — First physical webcam (roadmap Phase 2 camera checks)

| Field | Value |
|---|---|
| Report id | 2026-11-18-phase2-camera-bringup |
| Machine | Owner's Windows 11 PC (RTX 4080 reference machine), Rust 1.92 |
| Camera | UGREEN Camera, USB VID 0C45 PID 6369, system index 0 |
| Engine commits | `04dc99f` (format, exposure fallback), `a11831f` (settings), `56080aa` (timing) |
| People | None recorded or annotated. Whether anyone was in view during the timing run was not controlled; the reported keypoint quality suggests nobody was |

This is a hardware bring-up record, not V-19, V-20, L-01, L-03 or L-06: those need people,
a phone at 240 fps or an LED rig (owner checklist).

## Formats (`bansayo-engine --list-cameras`)

MJPEG and NV12 at 320×240, 640×480, 800×600, 1024×768, 1280×720, 1280×960, 1920×1080; at most
30 fps everywhere (640×480 also 25). The camera does not offer ≥ 60 fps, so at this camera's best
the profile is `basic` at 30 fps (column C30 of [22 §2](../architecture/martial-arts/22-measurement-feasibility.md)):
phase timing indicative, peak foot speed not reported.

## Rate and exposure

| Check | Result |
|---|---|
| Raw capture rate, 1280×720 MJPEG, release | 30.0 frames/s (60 frames timed) |
| Raw capture rate, debug build before the dev-profile fix | 7.6 frames/s (JPEG decode unoptimised); after the fix 30.0 |
| Manual exposure set and read back | 0.004 s → 1/256 s; 0.002 s → 1/512 s (Media Foundation steps of whole log2 seconds) |
| Live change over `PUT /cameras/CAM-FRONT/settings {"exposure_s":0.002}` | Read back 1/512 s; reported in `DeviceStatus.camera_settings` |
| Engine end to end, release, one camera, pose on DirectML | 30.0 Hz, state `tracking`, profile `basic` |
| Camera left at automatic exposure after the tests | Yes (`controls::automatic_exposure`) |

## Stage timings (15 §2.1), release, 30 fps, `exposure_s: 0.004`, 10 s window

| Stage | p50 | p95 | p99 | max | Budget | Met |
|---|---|---|---|---|---|---|
| decode (MJPEG 1280×720) | 2.5 ms | 4.3 ms | 6.5 ms | 9.2 ms | 4 ms | p50 yes, p95 no |
| inference | 15.9 ms | 22.0 ms | 27.7 ms | 29.4 ms | 12 ms | no |
| tick wait | 8.0 ms | 16.3 ms | 18.1 ms | 31.0 ms | 16.7 ms (60 Hz tick) | yes at p95 |
| solve | 0.02 ms | 0.04 ms | 0.06 ms | 1.9 ms | 2 ms | yes |
| publish (frame serialisation) | 0.02 ms | 0.03 ms | 0.04 ms | 0.3 ms | 2 ms | yes |
| training | — | — | — | — | 1 ms | not measured (no session ran with the camera) |

Frames dropped in the one-slot buffer: 44 of about 900. The stage-over-budget notices fired for
inference and decode. Inference is over budget because no subject was followed, so the person
detector ran on every frame; with a followed subject it runs every tenth frame (3.7 ms per tracked
frame, 17 November). Consequences are recorded in ADR-020 and ADR-021.

## L-06, engine part, on a replayed take (not a person)

The Phase 2 acceptance test replays the simulator's roundhouse as recorded landmarks through an
image-folder camera at 30 fps. Repetition end → `rep_completed` event: 149, 144, 144 ms
(`crates/bansayo-engine/tests/phase2_camera.rs`, printed with `--nocapture`). This is the
segmenter's 150 ms end hold; the client part and a real athlete are owner steps.
