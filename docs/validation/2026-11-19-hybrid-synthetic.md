# Hybrid tracking on synthetic data — 19 November 2026

**What this is:** a regression record of the rung-1 fusion method (ADR-025) on the simulator's
ground truth. **What it is not:** an accuracy figure for any real camera, tracker or athlete.
Real-world claims stay pending until the owner checklist
([HYBRID-OWNER-CHECKLIST.md](HYBRID-OWNER-CHECKLIST.md)) has been run.

## Setup

`crates/bansayo-engine/tests/hybrid_fusion.rs`. One simulated body, the built-in front kick played
at 4× speed (period 0.625 s, hip peak about 650 °/s) or the roundhouse kick. Three cameras at
1280×720, 55° vertical field of view, ±1 px uniform landmark noise, free-running phase (+3.7 ms),
**40 ms from capture to the solver** (inference). Five trackers (pelvis, thighs, shins), ±0.5°
noise, mounted at arbitrary rotations calibrated as the reference-pose capture would, **5 ms**
from sample to solver. The solver ticks at 120 Hz. Errors are taken against the truth **at the
publication instant**, which is what an athlete or a VR client sees.

```powershell
cargo test -p bansayo-engine --test hybrid_fusion -- --nocapture --test-threads 1
```

## Results (run of 19 November 2026)

| Scenario | Shin error RMS / max, ° | Knee error RMS, ° | Ankle error RMS / max, mm | Shin evidence age RMS, ms | Shin available, % | Root measured, % |
|---|---|---|---|---|---|---|
| camera 30 fps | 28.9 / 60.4 | 23.9 | 282 / 575 | 59 | 100 | 100 |
| camera 60 fps | 24.9 / 47.4 | 20.8 | 243 / 457 | 51 | 100 | 100 |
| camera 120 fps (reference) | 22.9 / 40.9 | 19.2 | 224 / 398 | 46 | 100 | 100 |
| IMU 100 Hz | 5.6 / 13.8 | 4.8 | — (no position claimed) | 11 | 100 | 0 |
| IMU 200 Hz | 4.2 / 9.1 | 3.6 | — (no position claimed) | 8 | 100 | 0 |
| camera 30 + IMU 100 | 5.6 / 13.8 | 4.8 | 56 / 135 | 11 | 100 | 100 |
| camera 60 + IMU 100 | 5.6 / 13.8 | 4.8 | 56 / 139 | 11 | 100 | 100 |
| camera 30 + IMU 100, 300 ms camera occlusion | 5.6 / 13.8 | 4.8 | 53 / 129 | 11 | 100 | 88 |
| camera 30 + IMU 100, shin tracker lost 0.8 s | 28.7 / 90.5 | 21.1 | 258 / 833 | 66 | 100 | 100 |
| camera 30 + IMU 100, trackers yawed 40° | 9.7 / 50.6 | 4.7 | 117 / 573 | 11 | 100 | 100 |

## Reading the table

- **Improvement:** with 100 Hz trackers the shin is 5× closer to the truth at publication time than
  with a 30 fps camera alone, because the camera's evidence is 46–59 ms old (frame interval plus
  inference). A 120 fps camera does not close the gap: its latency, not its rate, dominates here.
- **Camera rate does not matter once trackers drive the limbs**: 30 and 60 fps rows are equal. The
  camera contributes the root and the world alignment.
- **Ankle position stays camera-bound** (56 mm RMS hybrid): the root is a camera measurement up to
  ~75 ms old and is not moved between frames (no dead reckoning). Its uncertainty is published as
  `sigma` growing with anchor age (`fusion.anchor_speed_bound_m_s`, 3 m/s).
- **IMU-only has no positions** (no root measurement); the published root is floor contact,
  `inferred`.
- **Occlusion:** trackers keep the shin; the last camera set anchors the root for 150 ms
  (`time_alignment.max_optical_anchor_age_ms`), then position claims stop (root measured 88 %).
- **Tracker loss:** the shin falls back to the camera fit for that segment only, with camera-level
  error, and returns to the tracker when it comes back.
- **Heading:** trackers whose reference frame is yawed 40° start wrong; the optical correction
  converges at ≤ 15 °/s while the thighs and shins are tilted by the kicks, and does nothing while
  standing (upright segments carry no heading information). The row includes the convergence time.
- **Deterioration:** none of the hybrid rows is worse than the camera-only row it extends; the
  roundhouse kick test asserts the same halving of shin error.

## Not covered

Real cameras (blur, detector errors, occlusion by the athlete's own limbs), real trackers (gyro
drift, magnetic disturbance, strap slip), clock offsets between real devices, and the effect of
SlimeVR's receive-time stamping (no sample times in SolarXR). Those are the owner checklist.
