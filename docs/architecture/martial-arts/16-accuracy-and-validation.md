# 16 — Accuracy, trust and validation

How a value earns the right to be shown, stored and compared, and the experiments that will turn
the design defaults of this document set into measured figures.

**No experiment below has been run. This document contains no accuracy result**, and none may be
quoted from it. Acceptance thresholds are decision criteria chosen in advance, not expectations.

## 1. Trust model in one page

| Mechanism | What it guarantees | Defined in |
|---|---|---|
| Evidence on every value | Provenance, method, confidence, sigma and observation time travel with the value | [04 §3](04-movement-domain-model.md) |
| Suppression inside observables | An ill-conditioned quantity is unavailable with a reason, never an unstable number | [04 §6.1](04-movement-domain-model.md) |
| Grades A / B / C / X | One deterministic rule decides what a measurement may be used for | [05 §8](05-technique-library.md) |
| Null components | A score component without an adequate measurement is `null`, never zero | [07 §7](07-recognition-and-coaching.md) |
| Cue evidence gate | No cue from grade C; persistence over several repetitions | [07 §8.2](07-recognition-and-coaching.md) |
| Comparability classes | Progress never joins measurements made by different methods | [12 §5](12-data-and-progress.md) |
| Calibration QA | Scale and floor errors are caught at calibration time | [09 §7.4](09-optical-markers-and-calibration.md) |
| Recorded inputs | Any stored result can be recomputed and audited | [12 §7](12-data-and-progress.md) |

Per-metric requirements, sensor and calibration dependencies, error sources and suppression
conditions are the columns of the metric catalogue ([05 §10](05-technique-library.md)); they are
not repeated here.

## 2. References and instruments

Ordered by what a home laboratory can obtain. Each experiment names the weakest reference that is
sufficient for it.

| Id | Reference | Gives | Limit |
|---|---|---|---|
| REF-A | Steel tape, calipers, printed board with measured edge | Lengths | ± 1 mm at best |
| REF-B | **Wand**: a rigid bar carrying two `tag36h11` tags (and two bright spheres for landmark-free tests) at a caliper-measured separation of about 500 mm | A length that must stay constant at any speed and position | Tests geometry and synchronisation, not anatomy |
| REF-C | **Fixture**: a flat plate with tags at measured positions and a second plate hinged to it with a digital angle gauge | Static relative position and angle | Static only |
| REF-D | **Pendulum**: REF-B hung from a pivot of measured length, released from a measured angle | Analytically known angular speed and period | Small-angle model; air drag ignored |
| REF-E | Wall rail with marks at measured heights; floor grid of marked points | Heights and positions of a body point held against a mark | Held, not dynamic |
| REF-F | Universal goniometer / digital inclinometer applied by a second person | Static joint angles | Its own error is several degrees; inter-rater spread is measured, not assumed |
| REF-G | LED on a microcontroller with a two-way clock exchange ([15 §4.2](15-performance-and-latency.md)) | Ground-truth event times and camera offsets | — |
| REF-H | 240 fps phone video, annotated frame by frame | Phase boundaries and strike instants | ± 4 ms, plus annotator disagreement |
| REF-I | Instructor annotation of recorded sessions (labels, phase boundaries, cue agreement) in the Technique workspace | Ground truth for recognition and coaching | Subjective; two annotators and their agreement are required |
| REF-J | Optical motion-capture laboratory, if access is obtained | Dynamic joint kinematics | Not assumed available; experiments that need it are marked |

## 3. Calibration, markers and time synchronisation

| Id | Question | Procedure | Reference | Error metric | Decision it feeds |
|---|---|---|---|---|---|
| V-01 | Are intrinsics repeatable? | Five independent calibrations per camera; evaluate each on frames held out from all five | REF-A | Held-out RMS reprojection (px); spread of `fx`, `cx`, `cy` (%) | `calibration.intrinsics_max_rms_px`; the stop condition of [09 §7.2](09-optical-markers-and-calibration.md) |
| V-02 | How accurate is the rig over the volume? | Carry REF-B through a 3 × 3 × 3 grid of the training volume, static at each point; then the board | REF-A | Wand-length error: mean, SD, maximum (mm); as a function of position | `calibration.extrinsics_max_rms_px`; QA thresholds of [09 §7.4](09-optical-markers-and-calibration.md) |
| V-03 | Is the floor where the profile says? | A tag flat on the floor at nine grid points; then at three measured heights | REF-A, REF-E | Height error (mm) and tilt of the fitted plane (°) | `gates.floor_tolerance_m`; heel-height QA |
| V-04 | Static tag pose accuracy | REF-C at 1.5, 2.5 and 3.5 m; yaw 0°, 30°, 60°; 720p and 1080p; one, two and three views | REF-C | Position error (mm), orientation error (°), per condition | Sigma rule for `marker_cluster`; the single-view acceptance rule of [09 §4.2](09-optical-markers-and-calibration.md) |
| V-05 | Decode envelope | Tags of each default size moved away until decode stops; repeat per resolution and family | REF-A | Decode rate against pixels per cell and against edge pixels | The 4 px-per-cell and 32 px rules; default tag sizes; whether `tag25h9` is worth allowing |
| V-06 | Tags in motion | REF-D released from increasing angles; exposures 1/250, 1/500, 1/1000 s | REF-D | Decode rate and pose error against image motion per exposure, in cells | The "below about one cell" rule of [09 §4.3](09-optical-markers-and-calibration.md) |
| V-07 | Does the `time_sync` procedure find the offsets? | Run the arm-swing procedure ten times; compare with LED ground truth | REF-G | Offset error per camera: mean, SD (ms) | Accept the procedure if mean absolute error ≤ 2 ms and SD ≤ 2 ms; sets `evidence.sync_sigma_ms` |
| V-08 | Is `aligned` triangulation better than `gated`? | Swing REF-B at increasing speed in view of free-running cameras; solve the same recording in both modes | REF-B | Wand-length RMS error (mm) against tip speed; yield (fraction of ticks with a solution); added latency | **[ADR-023](adr/ADR-023-time-aligned-triangulation.md):** `aligned` becomes the default if, above 5 m/s, its length error is at most half of `gated`'s, its static error is not worse, and its yield is not lower |
| V-09 | Is marker mounting repeatable? | Put the carriers on and capture mounting five times; after each, hold two standard poses | — (repeatability) | Spread of the derived segment rotation in the standard poses (°) | Whether tag-derived twist qualifies for grade A across sessions; `calibration.mounting_max_age_s` |
| V-10 | Do clusters and landmarks agree? | Slow full-range movements with clusters on thigh and shin | — (agreement) | Angle between cluster-derived and landmark-derived segment axes: mean, SD | `markers.mounting_disagreement_rad` (slip detection) |
| V-11 | Is a moved camera detected? | Rotate one camera by about 1°, then 3°, during tracking | — | Time until `camera_extrinsics` is `stale`; false alarms over 30 min untouched | The invalidation rule of [09 §9](09-optical-markers-and-calibration.md) |

## 4. Joint angles, positions, speeds and timing

| Id | Question | Procedure | Reference | Error metric | Decision it feeds |
|---|---|---|---|---|---|
| V-12 | Static joint angles | Knee, elbow and hip held at about 0°, 45°, 90°, 120°; each realised by every available method (`image_plane` in a lateral view, `skeleton_fit_axis`, `imu_mounted`, `marker_cluster`) from one recording | REF-F (two raters) | Bias and limits of agreement per method (°); rater disagreement as the reference's own error | First `validated` sigma tables; whether any joint definition may set `validated: true` |
| V-13 | Heights above the floor | Ankle and knee held against REF-E marks at four heights, both sides, three positions in the volume | REF-E | Height error: mean, SD, maximum (cm), per method | `grade_a.max_sigma` for height metrics; the claim level of UC-05 |
| V-14 | Dynamic kinematics | Kicks and punches recorded simultaneously by the rig and a laboratory system | REF-J | RMSE of joint angles and keypoint paths over the movement | Only if REF-J is available. Without it, dynamic results are reported as **agreement between methods**, never as accuracy |
| V-15 | Linear speed | REF-D and REF-B swung; captured at 30, 60 and 120 fps | REF-D | Peak-speed error (%) against frame rate and filter cut-off | The 50 Hz and 100 Hz rate gates of the speed metrics |
| V-16 | Event timing | Kicks filmed by REF-H with an LED in view for clock alignment; technique phase machines run on the rig's recording | REF-G, REF-H | Strike-time error and phase-duration error (ms): mean, SD; per input rate and `time_quality` | The timing sigma formula of [04 §7](04-movement-domain-model.md); the analysis cut-offs (6, 8, 12, 15, 20 Hz compared) of [04 §6.3](04-movement-domain-model.md) |
| V-17 | IMU orientation | Tilt against an inclinometer on a jig; heading drift over 10 min at rest and after 5 min of kicking; saturated-sample counts at shin and ankle during fast kicks | REF-F, REF-C | Tilt error (°); heading drift (°/min); fraction of saturated samples | `evidence.imu.*`; `calibration.heading_max_age_s`; whether high-g parts are needed ([19 §3](19-risks-and-open-questions.md) O-07) |
| V-18 | Heading correction | Inject a known heading offset on a tracker whose segment is seen by cameras | Cameras as reference | Convergence time; steady-state error against `fusion.heading_tau_s` | `fusion.heading_tau_s`, `heading_valid_s` |

## 5. Counting, recognition and coaching

Ground truth is REF-I unless stated. A detection matches an annotated repetition when their time
intervals overlap with intersection-over-union ≥ 0.5.

| Id | Question | Data | Metric | Target (goal, not result) |
|---|---|---|---|---|
| V-19 | Repetition counting per technique and capability profile | ≥ 30 repetitions × ≥ 3 athletes per technique, recorded with inputs so that every profile is produced by source masking | Precision, recall; count error per 20 repetitions | Precision and recall ≥ 0.95 in guided modes; count error ≤ 1 per 20 |
| V-20 | Phase boundaries | The same recordings, phases annotated on REF-H video | Boundary error (ms): mean, SD; inter-annotator difference as the ceiling | Within two input periods on the simulator; on real data, not worse than twice the inter-annotator SD |
| V-21 | Free-mode recognition | ≥ 10 min of free training per athlete, annotated | Confusion matrix; precision per class; share reported `unknown` | Precision ≥ 0.95; recall reported, not targeted ([07 §9](07-recognition-and-coaching.md)) |
| V-22 | Sequences | Scripted combinations with deliberate wrong side, wrong technique, omission, early and late steps; forms with scripted omissions | Step-result accuracy; alignment edits recovered | Every scripted edit recovered on the simulator; ≥ 0.9 step-result accuracy on real recordings |
| V-23 | Definition review | The protocol of [05 §12](05-technique-library.md) | V-19 and V-20 for that definition | Passing sets `status: reviewed` with the reviewer's name |
| V-24 | Are cues right? | Sets in which cues were issued, shown to two instructors with the video and without the cue; they list the faults they see | Cue precision: share of issued cues that match a fault at least one instructor listed; agreement between instructors | ≥ 0.8 before coaching level `detailed` is offered by default. Cue recall is not targeted: silence is acceptable, a wrong correction is not |
| V-25 | Stage-2 templates | Template sets built from half of the confirmed repetitions, tested on the other half and on confusable techniques | Change in precision at equal recall against stage 1 | Stage 2 is enabled per technique only where precision does not fall |

Recognition by a learned model is not validated here because none is built
([ADR-026](adr/ADR-026-recognition-engine.md)); its entry condition is a measured shortfall in V-21.

## 6. Robustness

All of these reuse one kind of recording: a session captured with **every** available source and
video, then re-analysed under source masks ([12 §7.2](12-data-and-progress.md)). The motion is
identical across conditions by construction.

| Id | Condition | Variation | Reported |
|---|---|---|---|
| V-26 | Occlusion and camera loss | Mask each camera in turn; mask down to two; to one | Per metric: yield (share of repetitions at grade A, B, C, X) and paired difference to the unmasked result |
| V-27 | Camera layout | Four cameras recorded; subsets at about 60° and 90° separation, front/side/rear combinations | As V-26, plus triangulation sigma and reprojection error |
| V-28 | IMU dropout | Injected packet loss of 5, 20 and 50 %; gaps of 200 ms and 1 s inside repetitions | Repetition outcome changes; timing-metric differences |
| V-29 | Lighting | Three light levels measured with a lux meter or phone sensor (stated as approximate); mains flicker present and absent; these need separate recordings | Keypoint confidence, exposure reached, tag decode rate, metric yield |
| V-30 | Fast movement | The fastest kicks and punches the test athletes produce, at 60 and 120 fps and three exposures | Landmark availability within ± 50 ms of the strike; share of speed metrics suppressed |
| V-31 | Clothing and background | Loose trousers against fitted; cluttered against plain background; a second person in view | Subject selection errors; left/right swaps caught by the bone-length gate |
| V-32 | Long run | [15 §4.2](15-performance-and-latency.md) L-08 | Drops, memory, heading drift across 90 min |

## 7. Repeatability, comparability and confidence calibration

| Id | Question | Procedure | Metric | Feeds |
|---|---|---|---|---|
| V-33 | Cross-session repeatability | One athlete, five sessions over two weeks, each with full re-setup (floor board re-laid, carriers and trackers re-mounted); the same assessment each time, performed at a controlled sub-maximal target so that the athlete's own variation is small | Within-subject SD, standard error of measurement, minimum detectable change (MDC95 = 1.96 · √2 · SEM), per metric and method class | The validated minimum detectable change in the `noise` term of [12 §6](12-data-and-progress.md); whether an assessment may claim comparability across recalibration |
| V-34 | Method difference between sensor combinations | The masked re-analyses of V-26/V-27 | Paired median difference and spread per metric between comparability classes | Shown in Progress as "method difference"; confirms that classes must not be merged, or identifies pairs that could be |
| V-35 | Does confidence mean anything? | Pool V-12, V-13, V-16 | Reliability curve: error against binned confidence; sigma coverage: share of errors within 1σ and 2σ; error distribution of grade A against grade B | Sigma formulas of [04 §8](04-movement-domain-model.md) are kept, rescaled, or replaced by `validated` tables. If grade A is not measurably better than grade B for a metric, its grade thresholds are wrong and are changed |
| V-36 | End-to-end latency | [15 §4.2](15-performance-and-latency.md) L-01 to L-07 | ms | Budgets of [15 §2](15-performance-and-latency.md) |

"Improved movement or improved sensing?" is answered operationally by V-33 and V-34 together: a
change smaller than the method class's MDC95 is `no_clear_change`, and a change that coincides
with a class or calibration change is never labelled improvement ([12 §5](12-data-and-progress.md)).

## 8. Defaults and the experiment that sets each

| Configuration default ([13 §4](13-configuration-schemas.md)) | Set by |
|---|---|
| `triangulation.max_skew_ms`, `align_exact_ms`, `mode` | V-07, V-08 |
| `triangulation.max_reprojection_error_*`, minimum views and ray angle | V-02, V-27 |
| `gates.max_speed_m_s`, `bone_length_tolerance`, `floor_tolerance_m` | V-30, V-31, V-03 |
| `fusion.heading_tau_s`, `heading_valid_s`, `heading_reacquire_*` | V-18 |
| `evidence.detector_noise_floor_px`, `sync_sigma_ms`, `evidence.imu.*` | V-35, V-07, V-17 |
| `analysis.filters.*` | V-15, V-16 |
| `recognition.segmenter.*`, `accept_confidence`, `ambiguity_margin` | V-19, V-21 |
| `fiducials.*` thresholds, default tag sizes | V-05, V-06 |
| `calibration.*_max_rms_px`, `mounting_max_age_s`, `heading_max_age_s`, `qa_max_age_h` | V-01, V-02, V-09, V-17 |
| `markers.geometry_max_rms_mm`, `mounting_disagreement_rad` | V-04, V-10 |
| Grade thresholds (`grade_a.min_confidence`, `max_sigma`, rate gates) per metric | V-35, V-15 |
| Scoring tolerances and `tolerance_scale` | Not a measurement question: set with instructors during V-24 and versioned with the scoring model |
| `progress.*` windows and the `noise` rule | V-33 |
| `coaching.cooldown_reps`, persistence rule | V-24 |

Until its experiment has run, a default is labelled "starting value" wherever it is documented, and
changing it changes `tracking_config_id` ([08 §9.2](08-tracking-and-sensor-fusion.md)), so results
before and after are never silently mixed.

## 9. How uncertainty reaches the athlete

| Rule | Realisation |
|---|---|
| One way to show a number | `MeasurementValue` ([10 §5.1](10-desktop-architecture.md)): grade by shape and text (A solid, B outlined, C "indicative", X a dash with its reasons), never by colour alone |
| No false precision | Display resolution follows sigma: a value is rounded to the first significant digit of its sigma, and to whole centimetres, whole degrees and 10 ms at the finest. `reference_match` is shown in five bands |
| Estimated is said | A propagated sigma is drawn as a band labelled "estimated"; the word "accuracy" appears in the UI only for `validated` sigmas |
| Missing is explained | A suppressed metric shows why and what would make it available ("needs a second calibrated camera"), from the reasons in the capability report |
| Before the session | Readiness lists which metrics will and will not be measured with the current setup |
| Trends | Separate series per comparability class; series-break markers; `no_clear_change` is the stated result when the change is inside the noise term |
| Tracking loss | Figures change style or disappear; nothing freezes at its last value (existing ADR-013); smoothing is limited to the documented analysis filter |
| Simulated data | Origin badge on every screen that shows it (existing rule); excluded from progress |

## 10. Records, tables and exit criteria

| Item | Rule |
|---|---|
| Report | One Markdown file per experiment run under `docs/validation/` (`YYYY-MM-DD-V-nn.md`): rig, calibration set id, tracking configuration id, engine version, session ids of the recordings, method, results with spread, deviations from this plan |
| Raw data | The sessions' `recording.mcap` (and video where used), kept with "keep" set; they are the regression corpus for later algorithm changes |
| Validated sigma tables | A versioned data file in the engine (`evidence/validated.v<n>.yaml`): per method, quantity and operating range, the measured sigma and the report it comes from. Loading it switches `sigma_basis` to `validated` for the covered cases and changes `tracking_config_id` |
| Regression | Every experiment's recording becomes a replay test with a tolerance band around the reported result, so that an algorithm change that moves a validated figure fails a test |
| What may be claimed | A figure may appear in the product or its documentation only with its report id, operating range and reference. Outside the validated range the product shows grades and estimated bands only |
| Order | V-01 to V-03 and V-07/V-08 gate roadmap Phase 4; V-12, V-13, V-16 and V-19 gate the first accuracy statement; V-33 gates assessment comparability claims ([18](18-implementation-roadmap.md)) |
| `validated: true` on a joint definition | Requires V-12 for that joint and method, with the bias and limits of agreement recorded in the definition |
