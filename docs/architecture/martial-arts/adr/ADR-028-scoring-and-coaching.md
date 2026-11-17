# ADR-028 — Scoring as components against stated targets; coaching through a limited scheduler

| | |
|---|---|
| Status | **Accepted** for the structure; tolerance values and limits are starting values |
| Date | 17 November 2026 |
| Specified in | [07 §6–8](../07-recognition-and-coaching.md), [05 §8](../05-technique-library.md) |

## Context

Athletes want to know whether an attempt was better. The system observes kinematics with varying
quality, has no universal standard of correctness across styles, and can do harm with a confident
wrong correction.

## Decision

1. Every metric measurement has a **grade** (A, B, C, X) from one deterministic rule; the grade
   decides what the value may be used for.
2. A score is a **set of components**, each tied to one measurement and one **target** whose source
   is stated: `reference` (a selected demonstration), `personal` (the athlete's own grade-A
   history) or `definition` (an explicit value with a written rationale).
3. A component without an adequate measurement or target is **`null` with reasons**, never zero.
4. The only composite is **`reference_match`**, published with its coverage, shown in five bands,
   and only when required components and minimum coverage are present.
5. Cues are declared in content with a **basis** (measurement, reference, history, style) and pass
   an **evidence gate** (grade and persistence over several repetitions).
6. A **scheduler with hard limits** decides what is shown: at most one cue after a repetition,
   two corrective plus one positive after a set, three focus points per session; only safety and
   tracking notices in real time.
7. Scores are compared only inside one comparability class
   ([ADR-034](ADR-034-capability-profiles-and-comparability.md)).

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| A single 0–100 technique score | Rejected: arbitrary weights presented as objective quality |
| Universal joint-angle targets per technique | Rejected: no biomechanical justification across bodies and styles |
| DTW distance to a reference as the score | Rejected: similarity is not correctness; unexplained number |
| A learned quality model | Rejected: no labelled data; not explainable |
| Language-model generated corrections | Rejected for judgement; may later phrase verified results ([24](../24-adaptive-training.md)) |
| Corrective audio during the strike | Rejected: unusable at combination speed and unsafe |
| Show every deviation | Rejected: overload; motor-learning practice favours few cues |

## Rationale

A component with its target label answers "against what?". Null instead of zero keeps missing
observation from reading as failure. Hard limits are simpler to verify than a tuned relevance
model.

## Advantages

- Every score and cue can be explained from stored rows.
- Style differences are data (targets, tolerances), not code.
- Poor tracking produces silence, not noise.

## Disadvantages

- No headline number to chase.
- Personal targets need a few sessions before components become available.
- Authoring tolerances is expert work.

## Risks

Wrong cues ([19 §1](../19-risks-and-open-questions.md) R28); tolerances chosen by a developer
rather than an instructor (addressed by the review workflow of
[ADR-037](ADR-037-technique-authoring-and-review.md)).

## Consequences

Scoring models and cues are versioned content; a model version change starts a new comparability
class. The UI component `ScoreBars` never renders a single large number.

## Reconsider when

V-24 shows cue precision below 0.8 with the evidence gate as specified; or instructors show that a
composite other than `reference_match` is meaningful for a discipline.
