# 05 — Technique library and metrics

How techniques, stances, transitions and holds are defined as data, which definitions ship first,
and the complete metric catalogue with its measurement semantics.

Status: **Proposed**. Nothing here exists in code.

## 1. Definition kinds

One schema, `bansayo.technique/v1`, covers four kinds of movement definition. The word *technique*
is used for all four.

| `kind` | Meaning | Detector | Examples |
|---|---|---|---|
| `dynamic` | A movement with ordered phases and a completion | Phase machine | Roundhouse kick, jab, age-uke, squat |
| `hold` | A posture maintained for time | Hold machine (enter, stable, release) | Side-split hold, single-leg balance |
| `stance` | A static predicate over the body | Stance predicate | Zenkutsu-dachi, ap kubi, Muay Thai stance |
| `transition` | A change from one stance to another, optionally with a step direction and a turn | Stance exit → stance enter with travel and turn checks | Step forward in zenkutsu-dachi, 180° turn, switch step |

Footwork and defensive movement are ordinary definitions of kind `transition` and `dynamic`
(`muay_thai.check`, `karate.gedan_barai`), not annotations on strikes.

## 2. Identity, variants and versions

```text
<discipline>[.<style>].<name>          taekwondo.dollyeo_chagi
                                       karate.shotokan.mawashi_geri
                                       muay_thai.te_tad
general.<group>.<name>                 general.mobility.standing_hip_flexion_raise
user.<athlete_id>.<name>               user.daniel.my_roundhouse
```

| Rule | Decision |
|---|---|
| Identifier pattern | `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*){1,4}$`; immutable once any session references it |
| Version | Integer `version`, incremented on any change to phases, segmentation, metrics, scoring or cues. Text-only changes (names, instructions) do not increment |
| Reference | `ContentRef` = `"<id>@<version>"`. Sessions store the exact reference and a snapshot of the definition ([12 §3](12-data-and-progress.md)) |
| Discipline-independent grouping | `family`, from the closed list in §4. Used for cross-discipline comparison views and for choosing confusable sets |
| Variant by discipline or style | A **separate definition** with the same `family`. The Taekwondo, Karate and Muay Thai roundhouse kicks are three definitions with different phase lists |
| Variant by instructor | A definition under `user.<athlete_id>.…` with `extends: <ContentRef>`; only the keys present override, lists are replaced whole |
| Variant by athlete | Not a definition. Personal targets come from the athlete's references and baselines ([07 §6](07-recognition-and-coaching.md)) |
| Review state | `status: draft | reviewed`. `reviewed` requires `reviewed_by` and `reviewed_on`. The UI labels drafts as such. Every definition shipped by this project starts as `draft` until a qualified instructor of that style reviews it |

`extends` is a single level: a definition that extends another may not itself be extended.

## 3. Sides and roles

Definitions are written once and instantiated per side.

| Role in content | Resolves to |
|---|---|
| `.active` | The side passed at instantiation (`left` or `right`) |
| `.support` | The opposite side |
| `.lead`, `.rear` | From the session stance: `orthodox` → lead = left, rear = right; `southpaw` → the reverse. Without a stance, from `lead_side` at repetition start |
| unsuffixed unsided observables | Used as is. Signed ones (`pelvis_yaw`, `trunk_axial_rotation`, `trunk_lateral_lean`, `pelvis_obliquity`) are multiplied by −1 when the active side is `right` and the definition sets `mirror_signed: true` (default) |

A definition is authored for **active side = left** and mirrored by the engine. Thresholds on
signed observables are therefore written once.

## 4. Families

Closed list in `bansayo-training::content::Family`. A new family is a code change (one enum
value); it carries no behaviour and exists for grouping.

```text
kick.front  kick.roundhouse  kick.side  kick.back  kick.axe  kick.hook  kick.spinning  kick.push  kick.low
punch.straight  punch.hook  punch.uppercut
knee.straight  knee.diagonal
elbow.horizontal  elbow.diagonal
block.high  block.middle  block.low  defence.check
stance  transition.step  transition.turn  transition.switch
strength.squat  strength.lunge  balance.single_leg
mobility.hip  mobility.leg_raise  mobility.split  mobility.ankle
```

## 5. Schema `bansayo.technique/v1`

Field reference. `R` = required. The formal YAML is in [13 §6](13-configuration-schemas.md); a
complete example is [examples/technique-roundhouse-kick.yaml](examples/technique-roundhouse-kick.yaml).

| Field | R | Type | Meaning |
|---|---|---|---|
| `schema` | R | `bansayo.technique/v1` | |
| `id`, `version` | R | id, integer | §2 |
| `status`, `reviewed_by`, `reviewed_on` | R (`status`) | enum, text, date | §2 |
| `extends` | | `ContentRef` | §2 |
| `discipline`, `style` | R, — | text ids from `disciplines/` | |
| `family`, `kind` | R | §4, §1 | |
| `names` | R | `{ display, native?, aliases[] }` | Display strings; `display` is an English default, localisable through `i18n/` |
| `sides` | R | subset of `[left, right]` | Which side may be active. `[]` for bilateral movements (squat) |
| `active_limb` | R | `leg | arm | none` | Selects default segmentation driver and safety radius |
| `start_stance` | | technique id of kind `stance` | Must hold before a repetition is armed in guided modes |
| `end_stance` | | technique id of kind `stance` | Defaults to `start_stance` |
| `instructions` | | list of text | Shown in the `ready` sub-state |
| `segmentation` | R for `dynamic` | §6.1 | Activity window |
| `phases` | R for `dynamic` | list, §6.2 | Ordered |
| `strike` | | §6.3 | Key instant |
| `complete` | R for `dynamic` | condition | Ends the repetition |
| `hold` | R for `hold` | §6.4 | |
| `predicate` | R for `stance` | condition | Evaluated continuously |
| `transition` | R for `transition` | §6.5 | |
| `required_observables` | R | list of observable ids with roles | Without all of them the technique cannot be detected; the engine reports it as unavailable in the current capability profile |
| `optional_observables` | | list | Enable optional phases and additional metrics |
| `confusable_with` | | list of technique ids | Run as distractors when this technique is the expected one |
| `metrics` | | list of `{ use, params? }` | References into the metric catalogue (§7) |
| `scoring` | | scoring model id | [07 §7](07-recognition-and-coaching.md) |
| `cues` | | list | [07 §8](07-recognition-and-coaching.md) |
| `tempo` | | `{ slow_scale }` | Factor applied to phase time limits in stage `slow`; default 0.5 means limits are doubled |
| `max_gap_ms` | | integer | Longest tolerated absence of a required observable inside a repetition; default 80 |
| `safety` | R | `{ notes[], min_clear_radius_m, max_reps_per_set, max_sets_per_session, warmup_required }` | [17 §3](17-security-privacy-and-safety.md) |
| `progression` | | list of `{ level, label, description, constraints }` | Recommended beginner ladder; `constraints` may cap `tempo`, target height band, or require stage `slow` |

## 6. Detection grammar

The only "logic" a definition may contain. It is a predicate grammar with no arithmetic, variables,
loops or user-defined functions ([ADR-030](adr/ADR-030-configuration-and-content.md)).

```text
condition := { all: [atom…] } | { any: [atom…] } | atom
atom      := { obs: <observable-with-role>, op: <op>, value: <number>, unit: <unit>,
               ref?: <ref>, hold_ms?: <int>, prominence?: <number> }
op        := gt | lt | between | rises_above | falls_below | local_max | local_min | changed_by
ref       := absolute (default) | rep_start | phase_start | rest
unit      := rad | rad_s | m | m_s | leg_length | leg_length_s | arm_length | arm_length_s
             | shoulder_width | foot_length | bool
```

| Element | Semantics |
|---|---|
| `gt`, `lt` | Value compared with `value`; true while it holds |
| `between` | `value` is `[lo, hi]` |
| `rises_above`, `falls_below` | True on the tick of the crossing; latched for the phase |
| `local_max`, `local_min` | True when an extremum with at least `prominence` is confirmed (two fresh samples past it) |
| `changed_by` | Absolute change of the observable since `ref` is at least `value` |
| `ref: rep_start` / `phase_start` | The observable's value at that instant is subtracted first |
| `ref: rest` | The value captured in the `ready` sub-state while the start stance held is subtracted first |
| `hold_ms` | The atom must stay true this long (hysteresis in time) |
| Unavailable observable | The atom is **unknown**. `all` with an unknown atom is unknown; `any` is true if any known atom is true. An unknown mandatory phase condition blocks the transition; an unknown optional phase is skipped |
| Normalised units | The observable's `norm` value is compared. Positions and speeds in definitions shipped by the project **must** use normalised units so that thresholds transfer between athletes and to the image-plane realisation |
| Depth | `all`/`any` nest at most one level |

### 6.1 Segmentation block

```yaml
segmentation:
  driver: ankle_speed.active     # observable whose magnitude marks activity
  unit: leg_length_s
  start_above: 1.2
  end_below: 0.5
  min_ms: 250
  max_ms: 2500
```

In guided modes the driver arms the phase machine. In free training the shared activity segmenter
uses its own drivers ([07 §2](07-recognition-and-coaching.md)) and this block supplies the
duration limits.

### 6.2 Phase

```yaml
- id: extension                  # ^[a-z][a-z0-9_]*$, unique in the definition
  label: Extension
  optional: false                # optional phases are skipped when their condition is unknown or never met
  enter: <condition>
  min_ms: 0
  max_ms: 600                    # exceeded → repetition outcome `incomplete`
```

The machine is strictly ordered: phase *n+1* can be entered only from phase *n* or from an
optional phase after *n*. There are no backward transitions; a movement that reverses is an
`incomplete` repetition followed by a new one.

### 6.3 Strike

```yaml
strike:
  obs: ankle_speed.active
  op: local_max
  prominence: 1.0
  unit: leg_length_s
  in_phase: extension
```

Emits `strike_peak`. Timing between strikes is what combination windows measure.

### 6.4 Hold

```yaml
hold:
  enter: <condition>             # posture reached
  measure: hip_abduction.active  # observable whose stability defines the hold
  tolerance_rad: 0.087
  max_speed: { obs: ankle_speed.active, value: 0.10, unit: m_s }
  support: floor                 # none | floor | wall | hand | partner | strap  → measurement class (03 §8)
  max_hold_s: 60
  min_rest_s: 20
```

### 6.5 Transition

```yaml
transition:
  from: karate.shotokan.zenkutsu_dachi
  to: karate.shotokan.zenkutsu_dachi
  stepping_foot: rear            # lead | rear | either | none
  travel: { direction_deg: 0, tolerance_deg: 30, min: 0.5, unit: leg_length }   # relative to facing at start
  turn:   { by_deg: 0, tolerance_deg: 25 }                                      # positive = left
  max_ms: 1500
```

`travel` and `turn` are checked only when `root_x`/`root_z` and `pelvis_yaw` are available with
provenance `measured` or `derived`; otherwise the result is `not_assessed`.

## 7. Initial catalogue

All entries are `status: draft`. Phases are the project's initial segmentation for detection; they
are not a statement of correct technique. Phase names in parentheses are optional phases.

### 7.1 Kicks

| Id | Family | Phases | Strike observable | Distinguishing conditions |
|---|---|---|---|---|
| `taekwondo.ap_chagi` | kick.front | chamber → extension → retraction → recovery | `ankle_speed.active` max | `thigh_azimuth.active` < 0.5 at chamber when observable |
| `taekwondo.dollyeo_chagi` | kick.roundhouse | chamber → (pivot) → extension → retraction → recovery | `ankle_speed.active` max | Knee flexion ≥ 1.2 rad in chamber; `thigh_azimuth.active` > 0.6 at strike |
| `taekwondo.yeop_chagi` | kick.side | chamber → (pivot) → thrust → retraction → recovery | `foot_reach.active` max | `thigh_azimuth.active` > 1.1 at strike; knee extension through thrust |
| `taekwondo.dwit_chagi` | kick.back | turn → chamber → thrust → recovery | `foot_reach.active` max | `pelvis_yaw` changed by ≥ 2.3 rad before thrust |
| `taekwondo.naeryeo_chagi` | kick.axe | lift → descent → recovery | `ankle_height.active` max, then descent | Knee flexion < 0.5 rad at the apex |
| `taekwondo.huryeo_chagi` | kick.hook | chamber → extension → hook → recovery | `knee_extension_rate.active` min (fastest flexion) | Knee re-flexes while `thigh_elevation.active` stays high |
| `taekwondo.dwi_huryeo_chagi` | kick.spinning | turn → chamber → extension → hook → recovery | As hook | Turn ≥ 2.3 rad first |
| `karate.shotokan.mae_geri` | kick.front | chamber → extension → retraction → recovery | `ankle_speed.active` max | As `ap_chagi`; retraction mandatory |
| `karate.shotokan.mawashi_geri` | kick.roundhouse | chamber → (pivot) → extension → retraction → recovery | `ankle_speed.active` max | Chamber with `thigh_azimuth.active` > 0.8 |
| `karate.shotokan.yoko_geri_kekomi` | kick.side | chamber → thrust → retraction → recovery | `foot_reach.active` max | As `yeop_chagi` |
| `muay_thai.teep` | kick.push | chamber → thrust → retraction → recovery | `foot_reach.active` max | Strike defined by reach, not speed |
| `muay_thai.te_tad` | kick.roundhouse | step → (hip_drive) → swing → follow_through → recovery | `ankle_speed.active` max | **No chamber phase**; knee flexion at strike is a metric, not a gate |
| `muay_thai.te_kha` | kick.low | step → (hip_drive) → swing → recovery | `ankle_speed.active` max | `ankle_height.active` peak < 0.45 `leg_length` |

The three roundhouse definitions demonstrate the variant rule: same family, different phase lists,
no shared "correct" kinematics.

### 7.2 Punches, knees, elbows, defence

| Id | Family | Phases | Strike observable |
|---|---|---|---|
| `muay_thai.jab` | punch.straight | extension → retraction → guard | `wrist_reach.lead` max |
| `muay_thai.cross` | punch.straight | (rotation) → extension → retraction → guard | `wrist_reach.rear` max |
| `muay_thai.hook` | punch.hook | load → swing → guard | `wrist_speed.active` max with `elbow_flexion.active` between 1.0 and 2.1 rad |
| `muay_thai.uppercut` | punch.uppercut | dip → drive → guard | `wrist_height.active` rate max with elbow flexed |
| `karate.shotokan.choku_zuki` | punch.straight | extension → lock | `wrist_reach.active` max |
| `karate.shotokan.gyaku_zuki` | punch.straight | (rotation) → extension → lock | `wrist_reach.rear` max |
| `karate.shotokan.oi_zuki` | punch.straight | step → extension → lock | `wrist_reach.lead` max after a `transition.step` |
| `muay_thai.khao_trong` | knee.straight | lift → drive → recovery | `knee_speed.active` max |
| `muay_thai.khao_chiang` | knee.diagonal | lift → drive → recovery | `knee_speed.active` max with `thigh_azimuth.active` > 0.5 |
| `muay_thai.sok_tad` | elbow.horizontal | load → swing → guard | `elbow_speed.active` max |
| `muay_thai.sok_chiang` | elbow.diagonal | load → swing → guard | `elbow_speed.active` max |
| `muay_thai.check` | defence.check | lift → hold → return | `knee_height.active` max; hold ≥ 150 ms |
| `karate.shotokan.age_uke` | block.high | preparation → sweep → lock | `wrist_height.active` max; lock when `wrist_speed.active` < 0.5 `arm_length`/s |
| `karate.shotokan.soto_uke`, `uchi_uke` | block.middle | preparation → sweep → lock | `wrist_speed.active` max |
| `karate.shotokan.gedan_barai` | block.low | preparation → sweep → lock | `wrist_height.active` min |

### 7.3 Stances and transitions

Stance predicates use normalised stance geometry and knee flexion bands. Bands are initial values
for detection, deliberately wide.

| Id | Predicate (all must hold for 300 ms) |
|---|---|
| `karate.shotokan.zenkutsu_dachi` | `stance_length` 0.75–1.25 `leg_length`; `stance_width` 0.6–1.4 `shoulder_width`; `knee_flexion.lead` 0.6–1.6 rad; `knee_flexion.rear` < 0.45 rad |
| `karate.shotokan.kokutsu_dachi` | `stance_length` 0.6–1.1; `knee_flexion.rear` 0.6–1.6 rad; `knee_flexion.lead` < 0.7 rad |
| `karate.shotokan.kiba_dachi` | `stance_length` < 0.2; `stance_width` 1.6–2.6 `shoulder_width`; both knees 0.6–1.6 rad |
| `taekwondo.ap_seogi` | `stance_length` 0.25–0.6; both knees < 0.35 rad |
| `taekwondo.ap_kubi` | `stance_length` 0.75–1.2; `knee_flexion.lead` 0.5–1.5 rad; `knee_flexion.rear` < 0.45 rad |
| `taekwondo.dwit_kubi` | `stance_length` 0.5–0.95; `knee_flexion.rear` 0.5–1.5 rad |
| `taekwondo.juchum_seogi` | As `kiba_dachi` with `stance_width` 1.4–2.4 |
| `taekwondo.gyeorugi_junbi` | `stance_length` 0.3–0.8; both `guard_distance` < 0.75 `arm_length` |
| `muay_thai.stance` | `stance_length` 0.25–0.7; `stance_width` 0.5–1.3; both `guard_distance` < 0.6 `arm_length` |
| `general.stance.neutral` | `stance_length` < 0.2; `stance_width` 0.4–1.4; both knees < 0.3 rad |

Transitions shipped: `*.step_forward`, `*.step_back`, `*.turn_90_left`, `*.turn_90_right`,
`*.turn_180`, `*.turn_270_left` per stance family, and `muay_thai.switch_step`.

### 7.4 General

| Id | Kind | Phases or hold |
|---|---|---|
| `general.strength.squat` | dynamic | descent → bottom → ascent; driver `pelvis_height` |
| `general.strength.lunge` | dynamic | step → descent → ascent → return |
| `general.balance.single_leg` | hold | `foot_contact.active` = 0 and `knee_height.active` > 0.15 `leg_length`; measure `pelvis_height` stability and `root_speed` |
| `general.mobility.hip_rotation` | dynamic | lift → circle → return (controlled articular rotation); driver `knee_speed.active` |
| `general.mobility.standing_hip_flexion_raise` | dynamic | raise → apex → lower; `support: none` |
| `general.mobility.standing_hip_abduction_raise` | dynamic | raise → apex → lower |
| `general.mobility.front_split_hold`, `side_split_hold` | hold | `support: floor` |
| `general.mobility.hamstring_forward_fold` | hold | `support: none` (gravity-assisted is declared `floor` when hands reach the floor) |
| `general.mobility.ankle_dorsiflexion_lunge` | hold | `support: floor` |
| `general.mobility.controlled_high_kick` | dynamic | raise → apex hold (≥ 500 ms) → lower |

### 7.5 Adding a discipline

1. Add `disciplines/<id>.yaml` (name, styles, default stance vocabulary).
2. Add technique files. Reuse metric catalogue entries; add a metric definition only if a new
   combination of existing metric kind and observable is needed.
3. Add a family value **only** if none fits.
4. No engine change is required unless a new observable or metric kind is needed
   ([04 §9](04-movement-domain-model.md)).

## 8. Measurement quality grades

Every metric measurement has a grade computed by one deterministic rule from the metric's gates.

| Grade | Rule (first matching row from the top) | Used for |
|---|---|---|
| `X` suppressed | A required observable was unavailable for longer than `max_gap_ms` inside the window; **or** input rate below `min_rate_hz`; **or** a calibration kind listed in `gates.calibration` is not `valid`; **or** the realising method is not in `grade_b_methods`; **or** an explicit `suppress_when` condition held | Nothing. `value: null` with reasons |
| `A` | Every input method is in `grade_a_methods`; minimum confidence in the window ≥ `grade_a.min_confidence` (default 0.7); sigma, if not null, ≤ `grade_a.max_sigma`; origin is `live` | Progress, baselines, personal bests, assessments, scoring, cues |
| `B` | Every input method is in `grade_b_methods`; minimum confidence ≥ `grade_b.min_confidence` (default 0.4) | Scoring and cues (with the stricter cue rule in [07 §8](07-recognition-and-coaching.md)); shown in progress as a separate, visually secondary series |
| `C` | Otherwise available | Live display labelled "indicative" and per-repetition review only |

Simulated and fixture origins are graded normally but stored with their origin; progress queries
exclude non-`live` origins unless explicitly requested.

```ts
interface MetricMeasurement {
  metric: ContentRef;              // metric definition id@version
  value: number | null;            // native unit
  norm: number | null;             // normalised form when the definition declares one
  unit: string;
  grade: "A" | "B" | "C" | "X";
  evidence: Evidence;              // combined over the window (04 §3)
  comparability_class: string;     // 12 §5
  reasons: Reason[];
}
```

## 9. Metric kinds

Closed set in `bansayo-training::metrics`. A metric definition (`bansayo.metric/v1`) picks one kind
and parameterises it.

| Kind | Parameters | Result |
|---|---|---|
| `extremum` | `obs`, `agg: max|min`, `window` | Extreme value in the window |
| `value_at` | `obs`, `at: strike | phase_start:<id> | phase_end:<id> | rep_end` | Value at an instant |
| `range` | `obs`, `window`, `unwrap: bool` | Max − min in the window (`unwrap` for headings) |
| `duration` | `from`, `to` (event anchors) | Milliseconds between two anchors |
| `mean`, `median`, `rms` | `obs`, `window` | Statistic over the window |
| `peak_count` | `obs`, `window`, `prominence` | Number of local maxima |
| `settle_time` | `obs`, `below`, `for_ms`, `from` | Time from an anchor until the observable stays below a level |
| `path_deviation` | `points: [keypoint…]`, `reference` | Mean distance between the time-aligned keypoint path and the selected reference, in the path's normalised unit ([07 §4](07-recognition-and-coaching.md)) |
| `set_variability` | `of: <metric id>`, `stat: sd|cv|iqr` | Dispersion of another metric across the repetitions of a set (grade-A/B values only; needs ≥ 5) |
| `asymmetry` | `of: <metric id>` | `(left − right) / max(|left|, |right|)` over the medians of each side in one session (needs ≥ 5 grade-A values per side, same comparability class) |

`window` is `rep`, `{ phase: <id> }` or `{ from_phase: <id>, to_phase: <id> }`. Anchors are
`rep_start`, `rep_end`, `strike`, `phase_start:<id>`, `phase_end:<id>`.

## 10. Metric catalogue

Confidence representation is uniform for every metric: the `MetricMeasurement` of §8 (grade,
evidence with propagated or validated sigma, reasons). The columns below give the remaining
per-metric items required by the brief. "A methods" lists the realising methods of the principal
observable that qualify for grade A; all other world methods qualify for B; `image_plane` qualifies
only where stated.

### 10.1 Kicks

| Metric id · unit | Calculation | Required observables | Grade-A methods | Main error sources | Suppressed when |
|---|---|---|---|---|---|
| `kick.peak_ankle_height` · m, `leg_length` | `extremum max` of `ankle_height.active` over the repetition | `ankle_height.active` | `triangulated`, `marker_cluster` + `floor_plane` valid | Floor calibration; ankle landmark jitter at speed; motion blur; camera skew | No valid `floor_plane`; ankle untriangulated at the apex; `image_plane` gives B only when `view` ≠ `unknown` and only the normalised value |
| `kick.peak_knee_height` · m, `leg_length` | `extremum max` of `knee_height.active` in `chamber` | `knee_height.active` | As above | As above, smaller (slower joint) | As above |
| `kick.peak_ankle_speed` · m/s | `extremum max` of `ankle_speed.active` | `ankle_speed.active` | `triangulated` with input rate ≥ 100 Hz | Differentiation noise; frame rate; blur; time sync | Input rate < 50 Hz (X); 50–100 Hz gives at most B; `image_plane` never |
| `kick.hip_rotation_range` · rad | `range` (unwrapped) of `pelvis_yaw` from `rep_start` to `strike` | `pelvis_yaw` | `triangulated` hips, `imu_optical_fused`, `marker_cluster` | Hip landmark occlusion in lateral views; IMU heading drift | Pelvis yaw from `imu_mounted` with heading `assumed`; either hip missing > 80 ms |
| `kick.support_pivot` · rad | `range` (unwrapped) of `foot_yaw.support` over the repetition | `foot_yaw.support` | `triangulated` heel and toe, `marker_cluster`, `imu_optical_fused` | Small baseline (foot length); toe/heel landmark quality | Detector schema without feet; foot off the floor > 150 ms |
| `kick.support_knee_range` · rad | `range` of `knee_flexion.support` over the repetition | `knee_flexion.support` | `skeleton_fit_axis` from triangulated joints, `imu_mounted` | Support knee occluded by the kicking leg | Occlusion gap > `max_gap_ms` |
| `kick.knee_flexion_at_strike` · rad | `value_at strike` of `knee_flexion.active` | `knee_flexion.active` | As above | Fast joint; blur | — |
| `kick.extension_time` · ms | `duration` from `phase_start:extension` (or `swing`) to `strike` | Phase events | Any world method; `imu_gyro` best | Sample rate; filter delay (removed by zero-phase re-filter); timestamp quality | `time_quality: receive_time` gives at most B |
| `kick.retraction_time` · ms | `duration` from `strike` to `phase_end:retraction` | Phase events | As above | As above | Definition has no retraction phase |
| `kick.total_time` · ms | `duration` `rep_start` → `rep_end` | — | Any incl. `image_plane` | Segmentation thresholds | — |
| `kick.recovery_settle_time` · ms | `settle_time` of `root_speed` below 0.15 m/s for 300 ms, from `phase_start:recovery` | `root_speed` | `triangulated` root | Root noise | Root `inferred` (floor-contact rule) |
| `kick.balance_offset_peak` · `foot_length` | `extremum max` of `com_proxy_offset.support` from `strike` to `rep_end` | `com_proxy_offset.support` | All keypoints `triangulated` | Mass model; missing keypoints | More than 3 keypoints not measured |
| `kick.guard_at_strike` · `arm_length` | `value_at strike` of `guard_distance.lead` (and `.rear` as a second instance) | `guard_distance.*` | `triangulated` wrist and head | Wrist occlusion behind the trunk | Wrist unobserved at strike |
| `kick.path_consistency` · `leg_length` | `set_variability` computed as mean pairwise `path_deviation` of the ankle path | Ankle keypoint path | `triangulated` | All of the above | Fewer than 5 completed repetitions |

### 10.2 Punches

| Metric id · unit | Calculation | Required observables | Grade-A methods | Main error sources | Suppressed when |
|---|---|---|---|---|---|
| `punch.peak_wrist_speed` · m/s | `extremum max` of `wrist_speed.active` | `wrist_speed.active` | `triangulated` ≥ 100 Hz; `xr_device` (controller or tracked hand) | Short duration relative to frame period; blur | Input rate < 50 Hz |
| `punch.reach_at_strike` · `arm_length` | `value_at strike` of `wrist_reach.active` | `wrist_reach.active` | `triangulated`, `xr_device` + shoulder measured | Shoulder landmark convention | Shoulder or wrist unobserved |
| `punch.extension_time`, `punch.retraction_time` · ms | `duration` between phase anchors | Phase events | Any world method | Sample rate | — |
| `punch.elbow_azimuth_peak` · rad | `extremum max` of `shoulder_azimuth.active` during `extension` | `shoulder_azimuth.active` | `skeleton_fit_axis` + chest rotation measured | Chest orientation from shoulders only | Chest rotation not observed |
| `punch.trunk_rotation_range` · rad | `range` of `chest_yaw` over the repetition | `chest_yaw` | `triangulated` shoulders, IMU on chest with valid heading | Shoulder protraction contaminates chest yaw | — |
| `punch.shoulder_hip_separation` · rad | `value_at strike` of `trunk_axial_rotation` | `trunk_axial_rotation` | Twist observed for pelvis and chest | As above | `twist_not_observed` |
| `punch.guard_recovery_time` · ms | `duration` from `strike` to `phase_start:guard` | Phase events | Any world method | Threshold choice | — |
| `punch.rear_guard_at_strike` · `arm_length` | `value_at strike` of `guard_distance` of the non-striking hand | `guard_distance.*` | `triangulated` | Occlusion | Wrist unobserved |
| `punch.stance_stability` · `leg_length` | `range` of `stance_length` over the repetition | `stance_length` | `triangulated` ankles | — | Ankles not measured |

### 10.3 Knees, elbows, defence

| Metric id · unit | Calculation | Required observables | Grade-A methods | Suppressed when |
|---|---|---|---|---|
| `knee.peak_knee_height` · `leg_length` | `extremum max` of `knee_height.active` | `knee_height.active` | `triangulated` + floor | No floor calibration |
| `knee.peak_knee_speed` · m/s | `extremum max` of `knee_speed.active` | `knee_speed.active` | `triangulated` ≥ 100 Hz | Rate < 50 Hz |
| `knee.hip_drive` · `leg_length` | `range` of root position projected on the facing direction at `rep_start`, from `rep_start` to `strike` | `root_x`, `root_z`, `pelvis_yaw` | `triangulated` root | Root `inferred` |
| `knee.trunk_lean_at_strike` · rad | `value_at strike` of `trunk_forward_lean` (negative = leaning back) | `trunk_forward_lean` | World methods | `out_of_plane` |
| `elbow.peak_elbow_speed` · m/s | `extremum max` of `elbow_speed.active` | `elbow_speed.active` | `triangulated` ≥ 100 Hz | Rate < 50 Hz |
| `elbow.trunk_rotation_range` · rad | `range` of `chest_yaw` | `chest_yaw` | As punches | — |
| `strike.guard_at_strike` · `arm_length` | `value_at strike` of `guard_distance` of the non-striking hand | `guard_distance.*` | `triangulated` | Wrist unobserved |
| `strike.recovery_time` · ms | `duration` `strike` → `rep_end` | Events | Any | — |
| `check.peak_knee_height` · `leg_length`; `check.hold_time` · ms | `extremum`, `duration` of phase `hold` | `knee_height.active` | `triangulated` + floor | No floor calibration (height only) |

### 10.4 Combinations and forms

| Metric id · unit | Calculation | Requires | Suppressed when |
|---|---|---|---|
| `combo.sequence_accuracy` · fraction | Correct steps / expected steps | Step results | — |
| `combo.step_interval` · ms (one per transition) | `duration` between consecutive `strike_peak` events | Strike events of both steps | Either strike not detected |
| `combo.total_time` · ms | First `rep_start` → last `rep_end` | — | Round not completed |
| `combo.tempo_error` · ms | Signed difference between each strike and the metronome grid | `tempo_bpm` set | No metronome |
| `form.coverage` · fraction | Steps with a detected technique / steps in the form | Step results | — |
| `form.step_accuracy` · fraction | Steps whose every assessed check passed / steps with at least one assessed check | Step results | Fewer than half of the steps assessable |
| `form.facing_error` · rad (per step) | Absolute difference between `pelvis_yaw` at step end and the step's expected facing, both relative to the start heading | `pelvis_yaw` measured | Pelvis yaw not `measured`/`derived` with valid heading |
| `form.travel_error` · `leg_length` (per step) | Distance between root displacement and expected displacement direction × observed length | `root_x`, `root_z` measured | Root `inferred` |
| `form.tempo_ratio` · ratio (per step) | Step duration / reference step duration after global tempo normalisation | Alignment | No reference selected |
| `form.stance_conformance` · fraction | Steps whose end stance predicate held ≥ 300 ms / steps with an assessable stance | Stance observables | — |
| `form.mirror_symmetry` · ratio | For step pairs declared `mirror_of`, ratio of durations and of peak metrics | Paired steps | Pair incomplete |

### 10.5 Flexibility, mobility, balance

| Metric id · unit | Calculation | Required observables | Grade-A methods | Suppressed when |
|---|---|---|---|---|
| `rom.peak_angle` · rad | `extremum max` of the definition's `measure` observable over the raise | e.g. `thigh_elevation.active`, `hip_abduction.active` | `skeleton_fit_axis` on triangulated joints with pelvis rotation measured; `imu_mounted` on both segments with valid heading | Pelvis orientation not observed (the pelvis compensates and the angle would be thigh-to-vertical, which is reported as the separate metric below) |
| `rom.peak_limb_elevation` · rad | `extremum max` of the limb axis angle from world down | `thigh_elevation`-like, world-referenced | Any world method; `image_plane` gives B in a suitable view | `view_unsuitable` |
| `rom.hold_angle` · rad | `median` of `measure` during the stable part of a hold | As `rom.peak_angle` | As `rom.peak_angle` | Stable duration < 3 s |
| `rom.stable_hold_duration` · s | Sum of stable intervals | `measure`, end-point speed | Any world method; `image_plane` B | — |
| `rom.speed_peaks` · count | `peak_count` of the angular speed of `measure` over the raise (1 = single smooth movement) | Rate of `measure` | World methods ≥ 50 Hz | Rate < 30 Hz |
| `rom.range_consistency` · rad | `set_variability sd` of `rom.peak_angle` | — | — | < 5 repetitions |
| `rom.asymmetry` · ratio | `asymmetry` of `rom.peak_angle` or `rom.hold_angle` | Both sides | — | < 5 grade-A values per side |
| `balance.hold_duration` · s | Duration of a stable single-leg hold | `foot_contact`, `root_speed` | `triangulated` | Root `inferred` |
| `balance.sway_rms` · m/s | `rms` of `root_speed` during the hold | `root_speed` | `triangulated` root ≥ 30 Hz | Root `inferred` |
| `squat.depth` · rad; `squat.pelvis_drop` · `leg_length` | `extremum max` of mean knee flexion; `range` of `pelvis_height` | Knee flexion; `pelvis_height` | World methods | — |

`rom.peak_angle` versus `rom.peak_limb_elevation` is deliberate: the first is a joint angle and
needs the pelvis orientation; the second is how high the limb points and is what a single camera
can honestly give. They are different metrics with different ids and are never merged in a trend.

### 10.6 Observability by capability profile

Best grade attainable, assuming valid calibration and unoccluded views. `–` = suppressed.

| Metric group | `basic` | `multi_view` | `marker_assisted` | `hybrid` | `extended` |
|---|---|---|---|---|---|
| Repetition count, total time, phase durations | B | A | A | A | A |
| Peak heights (ankle, knee) in metres | – (B normalised only) | A | A | A | A |
| Peak linear speeds | – | B at < 100 Hz, A at ≥ 100 Hz | Same | Same; timing improves | Same |
| Hip rotation range, trunk rotation range | – | A when both hips/shoulders seen | A | A | A |
| Support pivot | – | A with a feet-capable detector | A | A | A |
| Twist observables (hip rotation, shoulder–hip separation) | – | – unless joint flexed (`skeleton_fit_plane`, B) | A on tagged segments | A on instrumented segments | A |
| Guard distances | – | A | A | A | A |
| Balance proxies | – | A | A | A | A |
| Joint ROM (`rom.peak_angle`) | – | A | A | A | A |
| Limb elevation ROM | B in a suitable view | A | A | A | A |
| Form facing and travel | – | A | A | A | A |
| Root position metrics during jumps | – | A | A | A | A |

The seventh profile, `inertial` (IMUs without world keypoints), is not a column because its
coverage depends on which segments carry trackers: joint angles between two instrumented segments
and all phase durations reach grade A; heading-dependent metrics reach A only with a valid heading
alignment and B otherwise; heights, root position, linear speeds and guard distances are
suppressed.

`extended` adds no metric by itself; depth or ranging inputs raise the evidence of root and floor
related observables and make the floor reference independent of camera calibration
([08 §8](08-tracking-and-sensor-fusion.md)).

## 11. What is measured, estimated, and not observable

| Class | Quantities |
|---|---|
| **Measured** (with multi-view or marker observation) | Keypoint positions; segment long-axis directions; heights above a calibrated floor; headings of pelvis, chest and feet |
| **Measured** (with IMU) | Segment angular velocity; segment tilt; event timing |
| **Estimated** | Segment twist without tags or IMU; joint centres from detector landmarks; speeds (differentiated); balance proxies; foot contact without a sensor; IMU heading between corrections |
| **Not observable — never reported** | Impact force, power or energy transfer; contact quality; muscle activation or effort; exact centre of mass and centre of pressure; ground reaction force; joint loads; injury risk; whether a stretch is active or passive; intent or target accuracy against a real opponent |

## 12. Validation of definitions

- Schema validation at load ([13 §11](13-configuration-schemas.md)); a definition that fails is
  reported and not offered.
- Each shipped definition has a simulator motion with scripted ground truth (phase times, peak
  values) and a regression test asserting detection and metric values within tolerance.
- Before a definition is marked `reviewed`: at least 30 real repetitions from at least 3 athletes
  recorded with inputs, with phase boundaries annotated by an instructor; detection precision and
  recall per [16 §5](16-accuracy-and-validation.md).
