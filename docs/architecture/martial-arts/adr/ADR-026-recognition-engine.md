# ADR-026 — Recognition: declarative phase machines first, templates second, learning last

| | |
|---|---|
| Status | **Accepted** for stage 1; stages 2 and 3 are Proposed with entry conditions |
| Date | 17 November 2026 |
| Specified in | [07 §1–5](../07-recognition-and-coaching.md), [05 §6](../05-technique-library.md) |

## Context

The platform must segment movement, count repetitions, recognise techniques, follow sequences and
report unknown or incomplete movements with confidence. No labelled martial-arts dataset exists in
this skeleton and sensor mix. In every guided mode the expected technique is already known.

## Decision

1. An **activity segmenter** independent of any technique bounds candidate movements.
2. **Stage 1:** one **deterministic phase machine** per expected technique and side, compiled from
   the technique's YAML predicates over catalogue observables. Unavailable observables make a
   condition unknown, never true.
3. An **arbiter** turns candidates into exactly one of: a repetition, `unknown`, `ambiguous`,
   `incomplete`, `wrong_side`. These are first-class, stored results.
4. **Stage 2:** DTW against confirmed templates may **lower** confidence; it never creates a
   detection.
5. **Stage 3:** a learned classifier may be added behind the arbiter's candidate interface, for
   free mode only, when ≥ 200 confirmed repetitions per class from ≥ 10 athletes exist **and**
   measured stage-1/2 recall is below target.
6. Sequences use one automaton ([ADR-027](ADR-027-exercise-engine.md)); whole forms are scored by
   **symbolic global alignment of tokens**, with DTW only inside matched steps.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| Learned temporal model first (graph-convolutional, temporal-convolutional, transformer) | Rejected: no data; opaque failures; sensor-mix dependent |
| DTW template matching as the primary detector | Rejected: needs templates before the first repetition; poor at rejecting non-movements; no phase semantics |
| Hidden Markov models per technique | Rejected: need training data for what a hand-written machine states directly |
| Technique detectors hard-coded in Rust | Rejected: every technique would be an engine change |
| Feature-based classifier (hand features + tree/SVM) | Deferred to stage 3: same data requirement |
| Vision-language or large language models on video | Rejected: not reproducible, not explainable, not local at frame rate |
| DTW over a whole form | Rejected: one omitted step shifts everything after it |

## Rationale

A phase machine explains itself ("the chamber was not detected"), is testable against scripted
simulator motion, works from the first repetition and yields the phase intervals that metrics need.
Recorded inputs let later stages be evaluated on old sessions.

## Advantages

- Recognition is content, reviewable by an instructor ([20](../20-technique-authoring.md)).
- Closed-set modes are robust; open-set mode prefers `unknown` to a wrong label.
- Deterministic replay.

## Disadvantages

- Thresholds are authored and may not transfer across athletes and styles.
- Open-set recall will be modest.
- Each technique needs a definition and a scripted test motion.

## Risks

Brittleness across bodies ([19 §1](../19-risks-and-open-questions.md) R27). Mitigated by
body-length normalisation, optional phases, variants and V-19 on several athletes.

## Consequences

Technique files carry the detection grammar. The Technique workspace stores instructor annotations
that later become templates and labels ([28](../28-dataset-and-regression.md)).

## Reconsider when

V-19 or V-21 show stage 1 below target for a technique family after its definitions were revised
with real recordings.
