# ADR-037 — Technique definitions are authored from annotated examples and reviewed by a second person

| | |
|---|---|
| Status | **Accepted** for the data model and review rules; the editor is Proposed (P1/P2) |
| Date | 17 November 2026 |
| Extends | [ADR-026](ADR-026-recognition-engine.md), [ADR-028](ADR-028-scoring-and-coaching.md), [ADR-030](ADR-030-configuration-and-content.md) |
| Specified in | [20](../20-technique-authoring.md) |

## Context

Recognition, metrics, scoring targets and cues are all content. If that content is written by a
developer from intuition, the platform encodes one person's reading of three martial arts and
presents it with the authority of measurement. Tracking a limb is the easy part; a credible
technique model is the hard one.

## Decision

1. Every shipped definition starts as **`draft`** and is labelled so in the UI.
2. A definition becomes **`reviewed`** only with: an **example set** of annotated recordings
   (positive, acceptable variations, named errors, confusables), **evaluation results** of the
   definition on that set, and a **reviewer who is not the author**, named with their style.
3. Thresholds are **proposed by the engine from annotated examples** (percentile bands) and
   accepted or edited by a person.
4. Disagreement between instructors produces **separate definitions**, never averaged ones.
5. Annotations are stored beside engine output and never overwrite it.
6. The authoring tool is a **mode of the Technique workspace** using the existing review
   components; the engine gains an evaluate and a suggest function. Raw YAML editing stays valid.
7. Published versions are immutable once referenced; `deprecated` retires a definition without
   breaking history.
8. Example sets of reviewed definitions become golden sessions.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| Developer-authored definitions shipped as authoritative | Rejected: the failure this record exists to prevent |
| Learn technique models from expert recordings automatically | Rejected: needs far more data than exists; opaque; still needs expert labels |
| A single canonical definition per technique across styles | Rejected: styles differ legitimately ([05 §2](../05-technique-library.md)) |
| Crowd-sourced definitions with voting | Rejected: no community, no moderation, no accounts |
| Verify instructor credentials | Rejected: not verifiable offline; the name and stated authority are recorded as claims |
| A separate authoring application | Rejected: duplicates timeline, traces and viewport |
| Free-form coaching notes instead of structured cues | Rejected as the primary form: not testable against examples; notes remain as annotations |

## Rationale

A definition tested against examples it must accept and reject, signed by a named second person,
is the most credibility obtainable without a federation's endorsement — and it is honest about
being exactly that.

## Advantages

- Detection quality and cue precision are measured per definition before it is trusted.
- Instructors can author without touching engine code.
- Review evidence doubles as regression data.

## Disadvantages

- Shipped content stays `draft` until instructors are found.
- Authoring requires recordings on a working rig.
- A closed catalogue limits what an instructor can express.

## Risks

Reviews done carelessly lend false authority; a definition tuned on few athletes overfits them
([19 §1](../19-risks-and-open-questions.md) R27). The minimum evidence rule and held-out athletes
address both partially.

## Consequences

`bansayo.technique/v1` gains a `review` block, `variations`, `common_errors` and the `deprecated`
status (additive). A new content kind `bansayo.examples/v1`.

## Reconsider when

A governing body or school offers an authoritative technique description the project may encode:
its definitions would carry that attribution and the same review mechanics.
