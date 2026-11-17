# 29 — Tracking hardware laboratory

The controlled comparisons that decide which camera, shutter, light, tag and IMU configurations the
platform recommends and what each may claim. This document is the **experimental design**; the
individual procedures and error metrics are the V-experiments of
[16](16-accuracy-and-validation.md) and are referenced, not repeated.

**Nothing here has been run. All tables below are templates.**

## 1. Capability card

| Item | Content |
|---|---|
| Use case and value | Tracking configurations are chosen and described from measured error, not presumed superiority; the feasibility matrix ([22](22-measurement-feasibility.md)) gets report ids in its cells |
| Existing support | Recording with inputs; source masking by re-analysis; the calibration script and import; triangulation tests; the validation plan |
| Needed now | Nothing in the engine beyond what Phases 2 and 4 already require: per-stage timing, `mode` switch for alignment, an unsmoothed landmark path |
| Dependencies | The hardware list of §3; one or two test athletes; a second person for goniometer and annotation work |
| Feasibility and limits | Feasible at home-laboratory precision (millimetres for lengths, a few degrees for joint angles, about 4 ms for timing). Dynamic joint accuracy against a laboratory system is possible only with access to one |
| Priority | **P0** to run the first block (§5.1) as soon as physical cameras work; the rest follows the roadmap |
| Smallest milestone | Block A below on two cameras: one afternoon, one report |
| Evidence before development | None; this *is* the evidence step |

## 2. Questions the lab must answer

| # | Question | Decides |
|---|---|---|
| Q1 | How much do 30, 60 and 120 fps differ for strike timing, peak speed and apex height? | Rate gates of the metrics; minimum camera recommendation |
| Q2 | Which exposure is short enough for landmarks and tags at kick and punch speed, at home light levels? | Default `exposure_s`; lighting guidance; whether tags are useful on striking limbs at all |
| Q3 | Does interpolation to a common instant recover the accuracy lost to free-running cameras? | [ADR-023](adr/ADR-023-time-aligned-triangulation.md) |
| Q4 | How many cameras, and where? | Rig guidance (2 at 90°, 3 at 120°, 4) |
| Q5 | What do tags add to rotation estimates, and until what speed? | Whether package 6a is worth building; default tag sizes |
| Q6 | What do IMUs add for timing and through occlusion; how fast does heading drift; do they saturate? | Whether package 6b is worth building; which part; `evidence.imu.*` |
| Q7 | Is a hybrid configuration measurably better than its parts for any metric? | Whether `hybrid` deserves grade-A gates of its own |
| Q8 | How repeatable is each configuration across re-setup? | Minimum detectable change per method class |

## 3. Equipment

Classes of equipment, not product recommendations; specific models are recorded in each report.

| Item | Purpose | Note |
|---|---|---|
| 2–4 UVC cameras with manual exposure, capable of 60 fps at 720p or better | Baseline rig | The cameras the owner has |
| At least 1 camera capable of ≥ 100 fps (and, if obtainable, a pair with hardware trigger or global shutter) | Q1, Q3 | Lets the same motion be seen at high rate and decimated |
| Tripods or rigid wall mounts | Rig stability | A moved camera invalidates a block |
| Calibration board on a rigid panel | Calibration | ChArUco today ([08 §1.2](08-tracking-and-sensor-fusion.md)) |
| Wand, fixture, pendulum, wall rail, floor grid | References REF-A … REF-E | [16 §2](16-accuracy-and-validation.md) |
| Goniometer or digital inclinometer | REF-F | |
| Microcontroller with LED and serial clock exchange | REF-G; latency tests | [15 §4.2](15-performance-and-latency.md) |
| Phone with 240 fps video | REF-H | |
| Lux meter or phone light sensor | Light levels | Stated as approximate |
| Printed clusters in the default sizes | Q5 | [21](21-wearable-hardware.md) |
| IMU trackers on the boards already owned | Q6 | Existing R4 |
| Dimmable, flicker-free light plus the room's ordinary light | Q2 | |

## 4. Factorial design

Varying everything independently is unaffordable. The design fixes one **reference condition** and
varies one factor at a time, except where an interaction is the question.

| Factor | Levels | Reference |
|---|---|---|
| Frame rate | 30, 60, ≥ 100 fps | 60 |
| Exposure | 1/125, 1/250, 1/500, 1/1000 s | 1/500 |
| Light | low (ordinary room evening), medium, high | medium |
| Cameras | 1, 2 at ~90°, 3, 4 | 3 |
| Alignment | nearest frame, interpolated | interpolated |
| Tags | none, clusters on thigh and shin, plus forearm | none |
| IMUs | none, pelvis + thigh + shin | none |
| Movement | static poses; slow raise; front kick; roundhouse; jab–cross | all |
| Athlete | ≥ 2, different heights | both |

Frame-rate levels are obtained from **one** high-rate recording by decimating recorded
observations in re-analysis, so that 30, 60 and 120 fps see identical motion. Camera-count and
sensor levels are obtained by **source masks** in the same way. Only exposure and light need
separate takes. Interaction studied explicitly: frame rate × exposure × movement speed (Q1, Q2).

## 5. Blocks

Each block is one sitting with one calibration; its report cites the calibration set id.

### 5.1 Block A — rig truth (no athlete)

V-01, V-02, V-03, V-07, V-08, L-01, L-02. Output: calibration repeatability, volume accuracy, floor
accuracy, camera offsets, alignment decision. **Gate:** nothing later is interpreted if volume
length error exceeds 1 % or the floor check fails.

### 5.2 Block B — static body

V-12, V-13 on both athletes, every available method from one recording. Output: bias and limits of
agreement per method for joint angles and heights; first `validated` sigma table.

### 5.3 Block C — fast movement

High-rate recording of front kick, roundhouse and jab–cross at each exposure and light level, with
REF-H video and the LED in view. V-15, V-16, V-30. Output: timing error and peak-speed error
against rate and exposure; landmark availability around the strike.

### 5.4 Block D — tags

V-04, V-05, V-06, V-09, V-10. Output: decode envelope, static pose accuracy, the speed at which
tags drop out per segment, mounting repeatability.

### 5.5 Block E — IMUs and hybrid

V-17, V-18, V-28, L-05, then Block C's movements repeated with IMUs on. Output: tilt error, drift
rate, saturation counts, timing agreement between gyroscope and camera events, occlusion bridging.

### 5.6 Block F — repeatability

V-33 over two weeks, in the configurations that survived A–E. Output: minimum detectable change
per metric and method class.

## 6. Result templates

Filled by the reports; shown here to fix the shape.

| Timing error of strike instant, ms (mean ± SD) | 30 fps | 60 fps | ≥ 100 fps |
|---|---|---|---|
| Front kick | — | — | — |
| Roundhouse | — | — | — |
| Jab | — | — | — |

| Landmark availability within ± 50 ms of the strike, % | 1/125 | 1/250 | 1/500 | 1/1000 |
|---|---|---|---|---|
| Ankle, roundhouse, medium light | — | — | — | — |
| Wrist, jab, medium light | — | — | — | — |

| Height error at four rail marks, cm (mean, max) | 2 cameras | 3 cameras | 4 cameras |
|---|---|---|---|
| Ankle | — | — | — |

| Rotation estimate vs fixture, ° | Landmarks | Tags | IMU | IMU + optical |
|---|---|---|---|---|
| Shin about its long axis, slow | — | — | — | — |

## 7. From results to the product

| Result | Changes |
|---|---|
| Error against rate and exposure (C) | Rate gates and `grade_a.max_sigma` per metric; camera guidance of [08 §3.1](08-tracking-and-sensor-fusion.md); default exposure |
| Volume and floor accuracy (A) | QA thresholds; `validated` sigma for triangulated keypoints by region of the volume |
| Alignment (A) | ADR-023 decision |
| Camera count and layout (masks) | Rig guidance; the `multi_view` rule (how many joints must triangulate) |
| Tags (D) | Default sizes; which segments are worth tagging; whether twist qualifies for grade A |
| IMUs (E) | Sensor range recommendation; heading age limit; whether `imu_gyro` timing is the preferred method |
| Hybrid vs parts (E) | Whether any metric's grade-A method list changes |
| Repeatability (F) | The noise term of progress classification; assessment comparability claims |
| Everything | Cells of [22](22-measurement-feasibility.md) annotated with report ids; open items closed in [19](19-risks-and-open-questions.md) |

A configuration is **recommended** only for the metrics where it measured better than the cheaper
one by more than the reference's own uncertainty. "More sensors" is never a reason by itself.

## 8. Practical rules

- One change per take; every take is a session with inputs, video and the "keep" flag.
- The rig is not touched within a block; a calibration check opens and closes each block.
- Athletes warm up; fast-movement takes are limited to the volume caps of
  [17 §3](17-security-privacy-and-safety.md).
- Raw recordings from the blocks become real golden sessions ([28](28-dataset-and-regression.md)).
- Reports follow [16 §10](16-accuracy-and-validation.md) and state what was *not* controlled.
