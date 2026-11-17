# ADR-039 — Adaptive training by closed rules over graded measurements; language models may only explain

| | |
|---|---|
| Status | **Proposed** (P1) for the rules; **Experimental** for the explainer |
| Date | 17 November 2026 |
| Extends | [ADR-027](ADR-027-exercise-engine.md), [ADR-028](ADR-028-scoring-and-coaching.md) |
| Specified in | [24](../24-adaptive-training.md) |

## Context

Athletes will expect the platform to notice recurring weaknesses and adapt. The stored history
(cues, component scores, trends, asymmetry, completion, effort ratings) supports this. Language
models make conversational coaching tempting, and they can state biomechanical judgements that no
measurement supports.

## Decision

1. Cross-session adaptation is a **catalogue of recommendation rules in code**, parameterised in
   configuration, reading only stored, graded evidence inside one comparability class.
2. A rule's output is a **`Recommendation`**: a proposal with its evidence records. The athlete
   accepts or dismisses; nothing is changed or started automatically.
3. Rules may lower difficulty or suggest rest on a single signal; raising difficulty needs
   sustained evidence and respects safety limits and the athlete's range envelope.
4. A calibration or class break coinciding with a trend change produces a recalibration proposal
   and suppresses training advice for that metric.
5. A **language model never produces a measurement, score, cue selection or recommendation**. It
   may optionally **rephrase** a structured, already-verified summary. Its output is mechanically
   checked (no number absent from the input; banned wording), labelled as generated, never stored
   as evidence, and replaced by template text on any check failure.
6. The explainer is off by default and runs locally or against an endpoint the user explicitly
   configures.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| Learned policy (bandit or reinforcement learning) choosing drills | Rejected: no outcome data; unexplainable; could optimise the measurable at the expense of the athlete |
| Language model as coach with access to raw data | Rejected: invents judgements; unreproducible; cannot be validated against V-24 |
| Language model selecting among rule outputs | Rejected: the selection is the judgement |
| Automatic program generation | Rejected: prescribing load is outside the product's claim ([17 §1](../17-security-privacy-and-safety.md)) |
| Automatic application of recommendations | Rejected: the athlete knows what the sensors cannot |
| No adaptation; programs only | Kept as the fallback: fixed programs remain fully supported |
| Rules written in content with expressions | Rejected ([ADR-030](ADR-030-configuration-and-content.md)) |

## Rationale

A rule with listed evidence can be audited, tested on synthetic histories and evaluated
retrospectively. Keeping the model at the phrasing layer captures its real benefit — readable
explanation — without giving it authority it cannot justify.

## Advantages

- Every proposal is explainable from stored rows.
- Rules are testable and can be evaluated against history before being enabled.
- No dependency on any model or network.

## Disadvantages

- Rules are coarse; they will miss what a human coach would see.
- New adaptation ideas need code.

## Risks

Rules amplify wrong cues (guard: V-24 precision gate before a cue may drive a rule); proposals
read as medical or load advice (guard: wording rules).

## Consequences

Recommendations and their outcomes are stored. The explainer contract is "structured summary in,
checked prose out".

## Reconsider when

Enough recommendation outcomes exist (hundreds, across athletes) to evaluate a learned ranking of
rule outputs against the rule order — still with rules as the only source of candidates.
