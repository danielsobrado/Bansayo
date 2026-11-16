# ADR-019 — Canonical movement representation: body state with evidence, closed observable catalogue

| | |
|---|---|
| Status | **Accepted** |
| Date | 17 November 2026 |
| Extends | ADR-002 (conventions), ADR-013 (rest pose on loss) in [docs/12](../../../12-decisions.md) |
| Specified in | [04](../04-movement-domain-model.md) |

## Context

Training definitions must work with one webcam, a calibrated rig, tags, IMUs or a headset, and must
not mention any of them. The engine already has a 19-segment skeleton (`bansayo.body.v1`), forward
kinematics, five joint measurements, and three labels per value (origin, provenance, confidence).
Pose detectors publish dataset-specific landmark sets; IMUs give rotations without positions;
cameras give positions without twist.

## Decision

1. `bansayo.body.v1` remains the canonical skeleton. A canonical **keypoint set** of 26 points
   (`bansayo.body.v1.points`) is added: the 19 segment origins plus head top, hand tips, toes and
   heels.
2. Detector schemas are converted by **schema maps** (data tables in code, closed list).
3. Every value above the solver carries **`Evidence`**: provenance, method, confidence, sigma with
   its basis, observation time and sources. A combined value takes the weakest of its inputs.
4. Segment evidence is split into **axis** and **twist**.
5. Training reads **observables**: a closed, versioned catalogue of named scalar time series
   implemented in Rust (`bansayo.observables.v1`), each with built-in suppression rules. Content
   references observables by id and cannot define new ones.
6. With one camera there is no `BodyState`; a **`PlanarBody`** per camera feeds the image-plane
   realisations of those observables that have one.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| Adopt a detector schema (COCO-17, Halpe-26, MediaPipe-33) as canonical | Rejected: ties definitions to one model; none carries rotations |
| A parametric body model (SMPL family) | Rejected: licence terms and fitting cost; shape parameters are not needed for the metrics in scope |
| Rotations only (BVH or glTF-rig style) | Rejected: cameras measure positions; forcing them through rotations discards what was measured and invents twist |
| Positions only | Rejected: IMUs and tags measure rotation; twist observables would be impossible |
| ISB joint coordinate systems and clinical angle sets | Rejected for now: they need anatomical landmarks no sensor here observes; the model-axis angles are stated as such |
| One confidence number instead of evidence | Rejected: cannot express "axis measured, twist unknown", nor drive comparability |
| Observables defined in YAML by formulas | Rejected: becomes an expression language ([ADR-030](ADR-030-configuration-and-content.md)); formulas need unit tests and suppression rules |

## Rationale

The representation has to be honest about *what kind* of thing was observed. Splitting axis from
twist and carrying the method lets one catalogue serve every sensor mix while refusing to report,
for example, forearm pronation from two joint centres.

## Advantages

- Sensor-independent definitions; one place for each formula.
- The same repetition is realisable by different methods, each graded on its own evidence.
- Ill-conditioned quantities are suppressed at the source.

## Disadvantages

- A new observable is a code change and a catalogue revision.
- Evidence on every value enlarges messages and recordings.
- Model-axis angles are not clinical angles; this must be said in the UI.

## Risks

Sigma formulas are propagation rules, not validated accuracies; mistaken for the latter they would
mislead. Guard: `sigma_basis` and the UI wording rules ([16 §9](../16-accuracy-and-validation.md)).

## Consequences

Existing joint measurements keep their ids; their mathematics moves to `bansayo-training`. The
`trunk_forward_lean` sign is reconciled ([19 §3](../19-risks-and-open-questions.md) O-01). The
recommended detector schema becomes Halpe-26 because COCO-17 has no feet.

## Reconsider when

- Hands, fingers or a finer spine enter scope (new skeleton and keypoint schema ids).
- Validation shows a model-axis angle is systematically unusable for a metric that an anatomical
  definition would rescue.
