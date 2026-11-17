# 23 — Biomechanics model and athlete personalisation

Body geometry, kinematic quantities beyond joint angles, joint constraints, and the rules that
adapt references and targets to one athlete's body. Builds on
[04](04-movement-domain-model.md); read that first.

## 1. Capability cards

| Item | Biomechanical modelling | Personal adaptation |
|---|---|---|
| Use case and value | Coordination and control measures (hip-to-trunk timing, proximal-to-distal sequencing, balance proxy) that explain *how* a strike was produced, not only how high or fast | Two athletes executing the same kick correctly are not scored against identical angles; progress is relative to one's own controlled range |
| Existing support | 19-segment skeleton, forward kinematics, observables catalogue, evidence | `AthleteProfile` (height, dominant side, stance, tolerance scale), baselines, personal records, reference retargeting ([07 §6.3](07-recognition-and-coaching.md)) |
| Needed now | The `peak:` anchor of §3.2; the body-model record of §2; nothing else | `rom_envelope` derivation of §5.2 (a query, no new table); `personal` and `fraction_of_envelope` target sources |
| Dependencies | Body-proportion calibration (Phase 4); V-15 for anything involving acceleration | A few sessions of grade-A history per athlete; assessments for the envelope |
| Feasibility and limits | Kinematic quantities are feasible with `multi_view`; kinetic quantities are not (§6) | Feasible; limited by how well the envelope reflects *controlled* range rather than a one-off maximum |
| Priority | P0 for definitions and data model; P1 for sequencing metrics | P0 data model; P1 use in targets |
| Smallest milestone | `kick.sequence_lag_pelvis_to_ankle` computed on the simulator and on one recorded rig session | A height target expressed as a fraction of the athlete's own controlled raise |
| Evidence before development | V-16 timing resolution at the rig's frame rate (a lag of tens of milliseconds must exceed the timing sigma) | V-33 repeatability of the envelope assessments |

## 2. Athlete body model

One record per athlete, produced by the body-proportion procedure
([09 §8](09-optical-markers-and-calibration.md) row 5) and versioned by
`SkeletonDefinition.rev`.

| Component | Content | Source | State labels |
|---|---|---|---|
| Stature | `height_m` | Entered | — |
| Segment lengths | 19 tail lengths | Default ratios × stature (`assumed`), or median measured joint distances in the reference pose (`valid`) | Recorded in `norm_basis` of the comparability class |
| Derived lengths | `leg_length`, `arm_length`, `shoulder_width`, `foot_length` | Sums and distances of the above | — |
| Mass fractions | One population table id (segment mass as a fraction of body mass, centre of mass along the segment) | Published anthropometric table; no body mass is stored, fractions suffice for a relative centre | The proxy label is permanent |
| Joint limits | §4 | Table in code | — |

Rules: proportions never adapt silently during a session ([08 §4.5](08-tracking-and-sensor-fusion.md));
recalibrating them increments the revision and starts a new calibration set; left and right
lengths are kept symmetric (the mean) unless the procedure measures a difference above 3 %, in
which case both are stored and the fact is shown — no interpretation is offered.

## 3. Kinematic quantities

### 3.1 Already in the catalogue

Joint and segment angles, heights, reaches, stance geometry, linear speeds, angular speeds,
extension rates, headings and their rates, foot contact, the centre-of-mass proxy
([04 §6](04-movement-domain-model.md)).

### 3.2 Additions planned (catalogue changes, each a small code change with tests)

| Addition | Definition | Why | Status |
|---|---|---|---|
| Anchor `peak:<observable>` | The interpolated time of an observable's extremum inside the repetition window (same parabola rule as strike time) | Lets the existing `duration` metric kind express coordination timing without a new kind | Proposed, P1 |
| `kick.sequence_lag_pelvis_to_ankle`, `punch.sequence_lag_pelvis_to_wrist` (ms) | `duration` from `peak:pelvis_yaw_rate` to `peak:ankle_speed.active` (or wrist) | Proximal-to-distal sequencing | Proposed, P1 |
| `punch.sequence_lag_pelvis_to_chest` (ms) | `duration` from `peak:pelvis_yaw_rate` to `peak:chest_yaw_rate` | Hip-then-trunk coordination | Proposed, P1 |
| `com_proxy_height`, `com_proxy_speed` | Height and horizontal speed of the mass-weighted keypoint centre | Sinking, rising and drift during techniques and forms | Proposed, P1 |
| `step_length`, `step_direction` | Root displacement between consecutive stance intervals | Footwork (§7) | Proposed, with transitions |

Sequencing lags are differences of two event times, each with its own timing sigma; the metric's
sigma is their root sum of squares and its grade-A gate requires that sigma ≤ 10 ms. At 60 fps
with estimated timestamps that gate will usually fail, and the lag is then grade B: usable in
feedback, not in progress.

### 3.3 Deliberately not added

| Quantity | Reason |
|---|---|
| Linear acceleration of keypoints | Second derivative of camera positions; noise grows with the square of the sampling rate. Reconsidered only if V-15 shows a usable band |
| Segment angular acceleration | Same; from gyroscopes it is one derivative and may be added for IMU-instrumented segments on evidence |
| Joint torques, powers, energies | Need masses, inertias and ground reaction: inverse dynamics on unmeasured inputs |
| Anatomical joint-coordinate-system angles | Need bony landmarks no sensor here observes ([ADR-019](adr/ADR-019-canonical-movement-representation.md)) |

## 4. Joint constraints

| Use | Rule |
|---|---|
| Plausibility (solver stage 10, planned) | A joint angle outside a **generous anatomical envelope** marks the contributing segment evidence as `inferred` with reason `implausible_joint_angle`; the value is not clamped silently |
| Skeleton fit | Twist that cannot be observed inherits from the parent (as built) and is labelled `inferred` |
| Never | As scoring targets, coaching thresholds or "normal range" statements |

The envelope is one table per joint observable (`min`, `max` in radians) in
`bansayo-training::observables`, taken from a cited normative source when it is written and widened
to cover trained martial artists. No numbers are fixed in this document because none has been
sourced yet ([19 §3](19-risks-and-open-questions.md) O-23).

## 5. Personalisation

### 5.1 What adapts to the athlete, and how

| Aspect | Mechanism | Defined in |
|---|---|---|
| Height and limb proportions | Lengths normalise positions; reference clips are rotations replayed on the viewer's skeleton | [07 §6.3](07-recognition-and-coaching.md) |
| Dominant side, preferred stance | Mirroring of references; `lead`/`rear` role resolution | [05 §3](05-technique-library.md) |
| Experience | `tolerance_scale` on scoring tolerances | [07 §7.2](07-recognition-and-coaching.md) |
| Left/right difference | Baselines and targets are **per side**; asymmetry is its own metric and is not corrected for | [05 §9](05-technique-library.md) |
| Range of motion | Range-type components take `personal` targets; §5.2 | Below |
| Style | A separate definition or an `extends` variant, never a per-athlete tweak | [05 §2](05-technique-library.md) |
| Individual technique | An `athlete_selected` reference | [07 §6.1](07-recognition-and-coaching.md) |

### 5.2 Controlled range-of-motion envelope

```text
rom_envelope(athlete, observable, side) =
    median of the session values of the matching rom.* metric (grade A, active class of movement,
    newest comparability class) over the last 3 valid assessments or mobility sessions
```

It is a **derived query** over existing rows, like progress; nothing new is stored. Uses:

1. **Safety cap.** Prompts and targets never exceed the envelope plus the definition's
   `progression_step` ([17 §3](17-security-privacy-and-safety.md)). The athlete may lower the cap
   manually: `AthleteProfile.rom_caps: { observable, side, max_rad }[]` (additive, optional).
2. **Relative targets.** A scoring component may use
   `target: { from: fraction_of_envelope, observable: thigh_elevation, fraction: 0.85 }` so that a
   "head-height" kick is asked only of an athlete whose controlled range contains it.
3. **Reference fit warning.** When a selected reference requires more range than the envelope in
   any targeted component, that component switches to its `personal` target and says so.

Without an envelope (no mobility history) range components are `null` with reason `no_target`;
nothing is assumed from population norms.

### 5.3 What is not personalised

Detection thresholds of a definition are the same for everyone in normalised units; an athlete who
cannot trigger a phase (a chamber below the knee-flexion gate) gets the `progression` ladder of the
definition, whose lower levels relax named constraints, not a private copy of the technique.

## 6. Measurable, estimable, not measurable

The authoritative list is [05 §11](05-technique-library.md); the per-configuration view is
[22](22-measurement-feasibility.md). In one line: positions, orientations and their first
derivatives are measurable with adequate sensing; balance and coordination are estimable as
proxies and timings; forces, muscle activity, pressure and load are not available without
instruments in the target or under the feet ([30](30-instrumented-equipment.md)).

## 7. Footwork and spatial movement

| Item | Footwork and spatial movement |
|---|---|
| Use case and value | Stance, foot placement, pivots, steps, travel and turns are checked in kata, poomsae, Muay Thai footwork and VR drills. Measurable value: per-step stance, facing and travel results, and step length and direction against the definition |
| Existing support | Triangulation and measured root on simulated cameras; the calibration board that fixes the floor origin and forward axis; stance and transition definition kinds; the sequence grammar |
| Needed now | Stances and transitions as ordinary definitions (specified); a floor frame that persists across sessions (`floor_plane`); the two observables `step_length` and `step_direction`. No separate footwork engine |
| Dependencies | A calibrated multi-camera rig; a pose model with heel and toe for foot angle and pivot; anchor tags to detect rig drift |
| Feasibility and limits | Feasible at `multi_view`. Not assessed with one camera. The rear foot is often hidden by the lead leg; foot angle is unavailable with the current COCO-17 model |
| Priority | P0 architecture; built with Phase 4 and package 6c |
| Smallest milestone | Stance and step checks of a four-step line drill, on the simulator and then on the rig |
| Evidence before development | V-02 and V-03 (volume and floor accuracy), V-13 (ankle positions) on the physical rig |

First-class by construction, not by a separate subsystem:

| Need | Realisation | Reference |
|---|---|---|
| Persistent floor coordinate system | `floor_plane` profile: origin, up and forward fixed by the floor board; anchor tags detect drift of the rig | [09 §7.3](09-optical-markers-and-calibration.md), [09 §9](09-optical-markers-and-calibration.md) |
| Stance width, length, lead side | Observables `stance_width`, `stance_length`, `lead_side` | [04 §6.2](04-movement-domain-model.md) |
| Foot placement and angle | Heel and toe keypoints; `foot_yaw`; foot contact | Same |
| Pivoting | `kick.support_pivot`; `foot_yaw` range per transition | [05 §10.1](05-technique-library.md) |
| Steps, travel, turns | `transition` definitions; `step_length`, `step_direction` (§3.2); `form.travel_error`, `form.facing_error` | [05 §1](05-technique-library.md), [05 §10.4](05-technique-library.md) |
| Movement patterns | Sequences of transitions in the shared grammar; a floor-plan trace of root and feet in the Technique workspace and VR form guide | [06 §4](06-exercise-and-workout-engine.md) |
| Foot occlusion | Foot keypoints fall back to anchored FK (`derived`) or become `null`; stance checks become `not_assessed` | [08 §7.3](08-tracking-and-sensor-fusion.md) |
| World-position drift | Root is used for travel only with provenance `measured`; in VR, headset-versus-camera disagreement above 10 cm stales `xr_alignment` | [11 §7.3](11-vr-architecture.md) |

Footwork needs `multi_view` ([22 §4.3](22-measurement-feasibility.md)); at `basic` it is not
assessed. Its smallest milestone is the stance and step checks of a four-step karate line drill on
the simulator, then on the rig.

## 8. Validation

V-12 and V-13 for angles and heights; V-16 for the timing resolution that sequencing lags depend
on; V-33 for the envelope's repeatability; a unit test per addition of §3.2 on scripted simulator
motion with a known lag.
