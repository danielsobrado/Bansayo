# ADR-025 — Sensor fusion as a ladder: closed-form arbitration first, filters only on evidence

| | |
|---|---|
| Status | **Accepted** as policy; each rung beyond the first is built only when its entry condition is met |
| Date | 17 November 2026 |
| Extends | ADR-011, ADR-013, ADR-016 in [docs/12](../../../12-decisions.md) |
| Specified in | [08 §7](../08-tracking-and-sensor-fusion.md) |

## Implementation note (19 November 2026)

Rung 1 is built in `pipeline/solve.rs`, `pipeline/align.rs` and `pipeline/fusion.rs`, method id
`bansayo.fusion.anchored_fk.v1` (part of `tracking_config_id`):

- **Time alignment:** the solver reconstructs at `t_ref`, the oldest of the fresh trackers' newest
  sample times; every tracker is SLERPed to it from a bounded history (64 samples), never
  extrapolated, never across `time_alignment.max_imu_gap_ms` or a reconnect.
- **Arbitration per segment:** time-aligned mounted IMU, else the camera skeleton fit, else the
  parent's rotation (`inferred`). One tracker no longer disables the camera fit of the other
  segments (the previous camera-fit-only-without-trackers split is gone).
- **Complementary heading:** mounting captured with heading assumed identity makes the true
  rotation `A·(R·M)·A⁻¹`, a conjugation by the tracker's heading `A`. `A` is visible only in the
  direction of tilted segments with a vertical rest axis, compared with the camera fit **at the
  camera's capture time** (the tracker is SLERPed to it). Rate-limited (15 °/s), time constant 1 s,
  outliers above 30° rejected after convergence, held while upright. A converged tracker's
  segments carry `imu_optical_fused`.
- **Anchored FK:** the camera root anchors forward kinematics while its age is within
  `time_alignment.max_optical_anchor_age_ms` (150 ms); its `sigma` grows by
  `fusion.anchor_speed_bound_m_s × age`; past the limit the root is floor contact (`inferred`).
  When the pelvis drops out of a still-current camera set, the last anchor is held within the same
  limit and reported `root_anchor_held`.
- **Plausibility:** a converged tracker that disagrees with a confident camera segment by more than
  `fusion.imu_camera_disagree_deg` at the same instant is reported (`imu_camera_disagree`) and its
  confidence halved.
- **Modes:** `tracking.mode: hybrid_auto | camera_only | imu_only` selects evidence without
  restarting sources.

Synthetic comparison: [docs/validation/2026-11-19-hybrid-synthetic.md](../../../validation/2026-11-19-hybrid-synthetic.md).
**Validation criterion for real hardware** (owner checklist H-05/H-06): on a recorded front kick
with a 30 fps camera and 100 Hz trackers, the hybrid knee angle stays within 10° of a 120 fps
camera reference at publication time, while camera-only 30 fps does not. Rungs 2–4 remain unbuilt;
their entry conditions are unchanged.

## Context

Inputs differ in rate, latency, uncertainty and failure mode. ADR-011 keeps the solver an ordered
list of plain functions and defers any estimator until real data exists. There is still no real
data. The brief warns against choosing a sophisticated algorithm because it is available, and
against integrating IMU acceleration to position.

## Decision

1. Fusion is **per-segment arbitration** over evidence (axis and twist separately), a **one-state
   complementary heading correction** per IMU towards optical heading, **plausibility gates** on
   triangulated points, and **anchored forward kinematics** for short occlusions.
2. **Acceleration is never integrated to a published position.** Position comes from an optical or
   ranging observation, or from kinematics anchored to one.
3. **The engine never predicts.** No extrapolation, no gap filling; smoothing is limited to the
   documented analysis low-pass on observables.
4. Further rungs, each with an entry condition:

| Rung | Algorithm | Built when |
|---|---|---|
| 1 | Gates, arbitration, complementary heading, anchored FK | Now (roadmap Phases 4, 6a, 6b) |
| 2 | Per-keypoint constant-velocity Kalman filter, 2D views as asynchronous measurements | Gates measurably fail on recorded real sessions (V-30, V-31), or [ADR-023](ADR-023-time-aligned-triangulation.md) fails V-08 |
| 3 | Error-state Kalman filter coupling IMU orientation and optical keypoints per limb | A stated metric requirement cannot be met through occlusions of the length actually observed (V-26, V-28) |
| 4 | Sliding-window factor-graph smoother with skeletal constraints | Offline re-analysis only, and only if rung 3 is insufficient |

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| A full-body EKF/UKF from the start | Rejected: process and measurement models would be tuned on simulated data and prove nothing |
| Factor graph (GTSAM-style) from the start | Rejected: a C++ dependency and a tuning problem before any failure has been observed |
| Learned fusion | Rejected: no data |
| Inverse-kinematics optimisation per frame | Rejected while every segment has two measured ends; closed-form fit suffices |
| Inertial dead-reckoning between optical fixes | Rejected: bias and gravity-leak arithmetic in [08 §5.1](../08-tracking-and-sensor-fusion.md) |
| Hold-last or predicted poses through gaps | Rejected (ADR-013): a frozen or invented limb looks like data |

## Rationale

Each rung-1 mechanism is deterministic, explainable to the athlete and testable on synthetic
geometry. Recorded inputs make it possible to add a higher rung later and re-analyse old sessions,
so nothing is lost by waiting for evidence.

## Advantages

- Every published value traces to named observations.
- Sensors come and go without retuning.
- Recorded sessions decide the next rung, not opinion.

## Disadvantages

- No optimal weighting of redundant sensors; arbitration picks, it does not blend.
- Noise passes through unsmoothed to the published pose.
- Longer occlusions produce `inferred` values or gaps where a filter would produce an estimate.

## Risks

Rung 1 may be visibly jittery on real cameras. That is accepted: jitter is measured and drives the
decision, instead of being hidden.

## Consequences

A stage trait arrives with the second implementation of any stage (ADR-011). Method ids record
which mechanism produced a value, so a later rung is a new method and a new comparability class.

## Reconsider when

The entry condition of a rung is met by a validation report in `docs/validation/`.
