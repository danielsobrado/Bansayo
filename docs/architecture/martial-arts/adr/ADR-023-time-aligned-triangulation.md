# ADR-023 — Time-aligned triangulation for free-running cameras

| | |
|---|---|
| Status | **Pending Validation** — decided by experiment V-08 |
| Date | 17 November 2026 |
| Would amend | ADR-017 (views combined only within a configured skew) in [docs/12](../../../12-decisions.md) |
| Specified in | [08 §4.3](../08-tracking-and-sensor-fusion.md), [16 §3](../16-accuracy-and-validation.md) |

## Context

ADR-017 combines views only when their frame times lie within `triangulation.max_skew_ms`
(default 20) and explicitly rejected interpolating 2D landmarks, because with simulated cameras
there was nothing to tune against and exclusion is visibly honest.

For martial arts the arithmetic changes the picture. A foot at 15 m/s moves 15 mm per millisecond.
Two free-running 60 fps cameras differ in phase by up to ±8.3 ms. The 20 ms gate therefore admits
displacement errors of the order of 0.1–0.3 m on a kicking foot, and a gate tight enough to bound
the error to centimetres would discard most frame pairs.

## Decision (proposed)

1. A per-camera **`time_sync` profile** (`offset_us`) is applied at ingest.
2. The solver keeps the last four landmark sets per camera and **interpolates each camera's
   landmarks linearly to a common instant** (the oldest of the cameras' newest frame times) when two
   frames bracket it within 1.5 nominal periods; otherwise the view is left out, as today.
3. Interpolated views are flagged, lose confidence, and add `speed × sync sigma` to the point's
   uncertainty.
4. `triangulation.mode: gated | aligned`. **`gated` stays the default** until validation passes.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| Keep the gate only (status quo) | Remains the fallback; insufficient for fast limbs by the arithmetic above |
| Tighten the gate to a few milliseconds | Rejected: yield collapses with free-running cameras |
| Require hardware-triggered cameras | Rejected as a requirement: cost and availability; supported where present (`time_quality: synchronized` skips alignment) |
| Triangulate each pair at its own time, then interpolate in 3D | Rejected: pairwise solutions are already displaced |
| Per-keypoint Kalman filter with each 2D view as an asynchronous measurement | Deferred: principled, but needs a motion model tuned on real strikes; it is a rung of the fusion ladder ([ADR-025](ADR-025-sensor-fusion-ladder.md)) |
| Continuous-time (spline) reconstruction | Rejected for live use: complexity; possible later for offline re-analysis |
| Interpolate with a higher-order curve | Rejected until linear is shown insufficient; more frames of latency |

## Rationale

Linear interpolation over at most one frame period removes the first-order skew error at the cost
of the residual curvature term, which is second order in the frame period. Whether that residual is
acceptable at striking accelerations is an empirical question — hence the status.

## Advantages (expected)

- Bounded skew error without discarding views.
- Uncertainty that grows with speed, so grades fall where the method is weak.

## Disadvantages

- Up to one frame period of added latency.
- Interpolated 2D points are not observations; they must be flagged everywhere they flow.
- Reverses a stated rejection in ADR-017.

## Risks

At peak limb acceleration, linear interpolation under-estimates curvature and biases the apex
inwards. Offset estimation by cross-correlation may itself be too coarse
([19 §3](../19-risks-and-open-questions.md) O-15).

## Decision criteria

From V-08, on a rigid wand swung in view of the real rig: `aligned` becomes the default if, above
5 m/s tip speed, its wand-length RMS error is at most half of `gated`'s, its static error is not
worse, and its yield is not lower. From V-07: the offset procedure is accepted at mean absolute
error ≤ 2 ms. If either fails, `gated` stays the default, speed and apex metrics remain capped by
their sigma, and the camera guidance recommends triggered or higher-rate cameras.

## Consequences

Triangulated landmarks gain `sigma_m` and `time_interpolated_views`. `triangulation.*` and
`evidence.sync_sigma_ms` are part of `tracking_config_id`, so switching mode starts a new
tracking configuration, never a silent change in a series.

## Reconsider when

- V-08 fails: evaluate the asynchronous-measurement filter.
- Hardware-synchronised cameras become the norm for the user base.
