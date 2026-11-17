# 22 — Measurement feasibility matrix

Which measurement each sensor configuration can support, for every metric of the catalogue and
every technique of the initial library. This is the document to consult **before** promising a
training capability.

**Every cell is a design expectation derived from geometry, sampling and the grade rules. None is a
validated accuracy.** The last column of each table names the experiment that replaces the
expectation with a measurement ([16](16-accuracy-and-validation.md),
[29](29-tracking-hardware-lab.md)). Where an experiment contradicts a cell, the cell and the
metric's grade gates change — not the report.

## 1. Capability card

| Item | Content |
|---|---|
| Use case and value | The athlete and the developer know, before a session and before building a feature, what will be measured, what will be estimated and what will not be reported |
| Existing support | Metric gates and grades ([05 §8–10](05-technique-library.md)); `CapabilityReport` and `TrainingState.metric_availability` ([08 §9](08-tracking-and-sensor-fusion.md), [14 §7](14-runtime-and-api-contracts.md)) |
| Needed now | Nothing new in code: the matrix is the human-readable projection of metric gates. A generated version is produced from the content store once it exists (§7) |
| Dependencies | Physical cameras, a calibrated rig, tags, IMUs; the experiments of §6 |
| Feasibility and limits | The matrix cannot express view dependence and occlusion precisely; those appear per repetition as grades |
| Priority | P0 as a document; P1 as a generated report |
| Smallest milestone | The readiness panel lists, per metric of the chosen technique, the best attainable grade and the reason for anything unavailable (Phase 1) |
| Evidence before development | None for the document. For any cell to be quoted outside it: its experiment |

## 2. Legend and configurations

| Symbol | Meaning | Stored grade | Status word ([32 §2](32-measurement-provenance.md)) |
|---|---|---|---|
| **O** | Observed: the quantity's inputs are directly measured; grade A is attainable | A | observed |
| **E** | Estimated: derived through a model, a single view or a marginal sampling rate; usable for feedback, shown as secondary in progress | B | estimated |
| **I** | Indicative: shown live, never stored as progress | C | estimated (indicative) |
| **–** | Not reported in this configuration, with a stated reason | X | unavailable |
| **N** | Not measurable by any kinematic configuration; needs instrumentation ([30](30-instrumented-equipment.md)) | — | unavailable |
| *n* | Normalised value only (body lengths), no metres | | |

| Column | Configuration | Capability profile |
|---|---|---|
| **C30** | One RGB camera, 30 fps | `basic` |
| **C60** | One RGB camera, ≥ 60 fps, short manual exposure, suitable view | `basic` |
| **M60** | Two or more calibrated free-running cameras, 60 fps, valid floor | `multi_view` |
| **M120** | As M60 at ≥ 100 fps (or hardware-triggered) | `multi_view` |
| **+T** | M60 plus printed tag clusters on the relevant segments | `marker_assisted` |
| **+I** | M60 plus IMUs on the relevant segments, valid mounting and heading | `hybrid` |
| **IMU** | IMUs only | `inertial` |

Feet-dependent rows assume a detector with heel and toe points; with the COCO-17 model that is
running today they are **–** in every camera column unless a tag or IMU supplies the foot
([08 §1.2](08-tracking-and-sensor-fusion.md)).

## 3. Metric matrix

### 3.1 Counting, timing, sequences

| Metric | C30 | C60 | M60 | M120 | +T | +I | IMU | Main limit | Validated by |
|---|---|---|---|---|---|---|---|---|---|
| Repetition count | E | E | O | O | O | O | O | Segmentation thresholds; view | V-19 |
| `*.total_time` | E | E | O | O | O | O | O | Segmentation | V-16 |
| Phase durations (`extension_time`, `retraction_time`, `guard_recovery_time`) | I | E | O | O | O | O | O | A 100–200 ms phase spans 3–6 frames at 30 fps; timestamp quality | V-16, V-20 |
| Strike instant | I | E | E | O | E | O | O | Sampling; blur; IMU gyro is the preferred source | V-16 |
| `combo.sequence_accuracy` | E | E | O | O | O | O | E | Confusable techniques need azimuth or yaw, absent in one view | V-22 |
| `combo.step_interval`, `tempo_error` | I | E | O | O | O | O | O | As strike instant | V-16 |
| `form.coverage`, `step_accuracy` | – | – | O | O | O | O | E | Forms turn away from a single camera | V-22 |

### 3.2 Positions and heights

| Metric | C30 | C60 | M60 | M120 | +T | +I | IMU | Main limit | Validated by |
|---|---|---|---|---|---|---|---|---|---|
| `kick.peak_ankle_height`, `peak_knee_height`, `knee.peak_knee_height`, `check.peak_knee_height` | E*n* | E*n* | O | O | O | O | – | Floor calibration; apex blur; skew. One camera: only when the limb moves in the image plane | V-03, V-13 |
| `rom.peak_limb_elevation` | E | E | O | O | O | O | E | View suitability | V-12 |
| `kick.guard_at_strike`, `punch.rear_guard_at_strike`, `strike.guard_at_strike` | – | – | O | O | O | O | – | Wrist occluded by the trunk; needs depth | V-13 |
| `punch.reach_at_strike` | I*n* | E*n* | O | O | O | O | – | Foreshortening towards the camera; shoulder landmark convention | V-13 |
| `punch.stance_stability`, stance geometry | – | – | O | O | O | O | – | Needs both ankles in world and pelvis heading | V-13 |
| `knee.hip_drive`, root displacement, `form.travel_error` | – | – | O | O | O | O | – | Root must be measured, not inferred | V-02 |
| `squat.pelvis_drop` | E*n* | E*n* | O | O | O | O | – | — | V-13 |

### 3.3 Angles

| Metric | C30 | C60 | M60 | M120 | +T | +I | IMU | Main limit | Validated by |
|---|---|---|---|---|---|---|---|---|---|
| Knee and elbow flexion values (`kick.knee_flexion_at_strike`, `support_knee_range`, `squat.depth`) | E | E | O | O | O | O | O | Out-of-plane error in one view; occlusion of the support knee | V-12, V-14 |
| `rom.peak_angle`, `rom.hold_angle` (joint range) | – | – | O | O | O | O | O | Needs pelvis orientation; otherwise it is limb elevation, a different metric | V-12 |
| `kick.hip_rotation_range`, `punch.trunk_rotation_range`, `elbow.trunk_rotation_range` (heading ranges) | – | – | O | O | O | O | E | Hip and shoulder landmark occlusion in side views; IMU heading drift | V-12, V-18 |
| `kick.support_pivot` (foot yaw range) | – | – | O | O | O | O | E | Short baseline (foot length); needs heel and toe | V-12 |
| Twist: `hip_rotation`, `punch.shoulder_hip_separation`, forearm rotation | – | – | – / E | – / E | O | O | O | Two joint centres do not give twist; E only while the child joint is flexed | V-04, V-09 |
| `knee.trunk_lean_at_strike`, trunk lean | E | E | O | O | O | O | O | View; plane conditioning | V-12 |
| `form.facing_error` | – | – | O | O | O | O | E | Heading reference | V-18 |

### 3.4 Speeds and dynamics

| Metric | C30 | C60 | M60 | M120 | +T | +I | IMU | Main limit | Validated by |
|---|---|---|---|---|---|---|---|---|---|
| Peak linear speed (`kick.peak_ankle_speed`, `punch.peak_wrist_speed`, `knee.peak_knee_speed`, `elbow.peak_elbow_speed`) | – | – | E | O | E | E | – | Sampling rate; blur; camera skew; filter cut-off. Tags blur out at the strike; IMUs give angular rate, not linear speed | V-15, V-30 |
| Segment angular speed (`shin_angular_speed` and similar) | – | – | E | E | E | O | O | Direct gyroscope measurement; saturation at the ankle | V-17 |
| Joint extension rate | – | I | E | O | E | O | O | As above | V-15 |
| Linear or angular acceleration | – | – | – | – | – | I | I | Second derivative of noisy positions; not in the catalogue ([23 §3](23-biomechanics-model.md)) | V-15 |
| `rom.speed_peaks` (smoothness of a controlled raise) | E | E | O | O | O | O | O | Slow movement: low sampling demand | V-16 |

### 3.5 Balance, stability, consistency

| Metric | C30 | C60 | M60 | M120 | +T | +I | IMU | Main limit | Validated by |
|---|---|---|---|---|---|---|---|---|---|
| `kick.balance_offset_peak` (balance **proxy**) | – | – | O | O | O | O | – | Population mass model; missing keypoints; it is not centre of pressure | V-13 |
| `kick.recovery_settle_time`, `balance.sway_rms`, `balance.hold_duration` | I | I | O | O | O | O | – | Root must be measured | V-13 |
| `rom.stable_hold_duration` | E | E | O | O | O | O | O | — | V-16 |
| `set_variability`, `path_consistency`, `rom.range_consistency` | as the underlying metric | | | | | | | Needs ≥ 5 values at grade A or B | V-33 |
| `asymmetry` metrics | as the underlying metric | | | | | | | Needs both sides in one class | V-33 |

### 3.6 Not measurable kinematically

| Quantity | Any camera, tag or IMU configuration | What would measure it |
|---|---|---|
| Impact force, impulse, power | N | Force or acceleration sensing in the target ([30](30-instrumented-equipment.md)) |
| Contact, contact time, contact quality | N | Contact switch or pressure sensor in a pad |
| Centre of pressure, ground reaction, weight distribution | N | Pressure mat or insoles, force plates |
| Muscle activation, effort | N | Electromyography — out of scope |
| Whether a stretch is active or passive | N | Not instrumented; declared by the chosen definition |
| Injury risk, joint load | N | Not a measurement the platform makes |

## 4. Technique matrix

"Detectable" means the technique's required observables exist, so its phase machine can run.
"Closed" = when it is the expected technique (guided modes); "open" = distinguishable from its
confusables in free training.

### 4.1 Kicks

| Technique | C60 closed | C60 open | M60 | Needs for full metric set | Specific limit |
|---|---|---|---|---|---|
| `taekwondo.ap_chagi`, `karate.shotokan.mae_geri` (front) | Yes, lateral view | No: front vs roundhouse needs thigh azimuth | Yes | M120 for speed | Towards-camera kicks foreshorten in one view |
| `taekwondo.dollyeo_chagi`, `karate.shotokan.mawashi_geri` (roundhouse) | Yes | No | Yes | Feet for pivot; M120 for speed; +T/+I for hip twist | Support pivot is the headline coaching point and needs heel and toe |
| `muay_thai.te_tad`, `te_kha` (round, low) | Yes | No | Yes | As above | No chamber phase: fewer events to anchor timing |
| `taekwondo.yeop_chagi`, `karate.shotokan.yoko_geri_kekomi`, `muay_thai.teep` (side, push) | Yes, strike by reach (normalised) | No | Yes | M60 for reach in metres | Reach towards the camera is unobservable in one view |
| `taekwondo.dwit_chagi`, `dwi_huryeo_chagi` (back, spinning) | **No**: the turn condition needs pelvis yaw | No | Yes | ≥ 3 cameras recommended; +I helps through self-occlusion | Body turns away from every camera in turn |
| `taekwondo.naeryeo_chagi` (axe) | Yes | Partly: straight-knee apex is distinctive | Yes | M60 for apex height | — |
| `taekwondo.huryeo_chagi` (hook) | Yes | No | Yes | M120 for the re-flexion rate | Fast knee re-flexion spans few frames |

### 4.2 Punches, knees, elbows, defence

| Technique | C60 closed | C60 open | M60 | Needs for full metric set | Specific limit |
|---|---|---|---|---|---|
| `muay_thai.jab`, `cross`; `karate.shotokan.choku_zuki`, `gyaku_zuki` | Yes, lateral view | Lead vs rear only by side | Yes | M120 for speed; +I or XR hands for timing; +T/+I for separation | A straight punch lasts of the order of 100 ms: 3 frames at 30 fps |
| `karate.shotokan.oi_zuki` | No: needs the preceding step transition | No | Yes | — | Depends on stance and travel |
| `muay_thai.hook`, `uppercut` | Yes | No | Yes | M120 | Hook plane vs camera plane |
| `muay_thai.khao_trong`, `khao_chiang` (knees) | Yes | Straight vs diagonal: no | Yes | M60 for hip drive | — |
| `muay_thai.sok_tad`, `sok_chiang` (elbows) | Yes | No | Yes | M120 | Elbow occluded by the guard in frontal views |
| `muay_thai.check` | Yes | Yes (held lift) | Yes | — | — |
| `karate.shotokan.age_uke`, `soto_uke`, `uchi_uke`, `gedan_barai` (blocks) | Yes, frontal view | High vs low: yes; middle variants: no | Yes | +T/+I for forearm rotation | Forearm rotation is part of the technique and is twist |

### 4.3 Stances, transitions, general

| Technique | C60 | M60 | Specific limit |
|---|---|---|---|
| All stances (`zenkutsu_dachi`, `kokutsu_dachi`, `kiba_dachi`, `ap_seogi`, `ap_kubi`, `dwit_kubi`, `juchum_seogi`, `gyeorugi_junbi`, `muay_thai.stance`, `general.stance.neutral`) | **Not assessed**: stance length and width need world ankles and pelvis heading. Guided modes arm on stillness instead ([06 §3](06-exercise-and-workout-engine.md)) | Yes | Rear foot occluded by the lead leg in frontal views; foot angle needs heel and toe |
| Transitions (steps, turns, `switch_step`) | Not assessed | Yes | Root must be measured; turn angle needs a heading |
| `general.strength.squat`, `lunge` | Yes | Yes | Depth as a knee angle is view-dependent in one camera |
| `general.balance.single_leg` | Duration only (I) | Yes | Sway needs a measured root |
| `general.mobility.standing_hip_flexion_raise`, `standing_hip_abduction_raise`, `controlled_high_kick` | Limb elevation (E) in the matching view | Joint range (O) | The pelvis tilts to gain height; without its orientation only limb elevation is honest |
| `general.mobility.front_split_hold`, `side_split_hold`, `hamstring_forward_fold`, `ankle_dorsiflexion_lunge` | Hold duration; angle E in the matching view | Yes | Floor-level limbs are often occluded; ankle dorsiflexion needs heel and toe |
| `general.mobility.hip_rotation` | Count only | Yes; twist with +T/+I | Controlled rotation is mostly twist |

## 5. What the athlete is told

The brief's example, produced from fields that already exist — `TrainingState.metric_availability`
before the session and each `MetricMeasurement` after a repetition:

| Rig | Readiness text for a roundhouse-kick session |
|---|---|
| One webcam, 30 fps | Repetitions: available. Phase timing: indicative (camera rate 30 fps). Kick height: estimated, body-relative only (no floor calibration). Hip rotation: unavailable (needs two calibrated cameras). Support-foot pivot: unavailable (pose model has no foot points). Peak foot speed: not reported (sampling rate insufficient). |
| Three calibrated 60 fps cameras | Repetitions: available. Phase timing: available. Kick height: available. Hip rotation: available. Support-foot pivot: unavailable (pose model has no foot points). Peak foot speed: estimated (camera rate below 100 fps). |
| The same with a stale floor calibration | As above, except — Kick height: unavailable (floor calibration is stale; recalibrate). |

Each phrase is `status word` + `(first reason)`, rendered by clients from `best_grade` and
`reasons`; no client decides availability.

## 6. Cells that need physical validation first

| Claim in the matrix | Why it is uncertain | Experiment |
|---|---|---|
| "O" for heights at M60 | Depends on real calibration, floor and apex blur | V-02, V-03, V-13 |
| "E" vs "O" boundary for speeds at 60 vs 100+ fps | The rate gates (50, 100 Hz) are arithmetic | V-15, V-30 |
| "E" for image-plane angles | Out-of-plane error is unquantified | V-12 |
| "O" for twist with tags | Mounting repeatability and soft tissue | V-04, V-09, V-10 |
| "O" for heading with IMUs | Drift between optical corrections | V-17, V-18 |
| Phase timing at 30 fps as "I" rather than "E" | Could be better than assumed for slow techniques | V-16 |
| Closed-set detectability in one view | Phase machines on real athletes | V-19 |
| All of the above across sessions | Repeatability and minimum detectable change | V-33 |

## 7. Keeping the matrix true

The matrix is derived data. Once the content store exists, `GET /content/feasibility` (a pure
function of metric gates × the profile rules of [08 §9.2](08-tracking-and-sensor-fusion.md))
returns the same tables per technique, and this document's §3–4 are replaced by its output so that
they cannot drift from the gates. Validation reports then annotate cells with their report id.
