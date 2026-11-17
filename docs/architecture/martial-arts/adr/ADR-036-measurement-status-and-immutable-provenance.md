# ADR-036 — One status vocabulary; stored results are immutable and carry their provenance

| | |
|---|---|
| Status | **Accepted** |
| Date | 17 November 2026 |
| Extends | [ADR-019](ADR-019-canonical-movement-representation.md), [ADR-034](ADR-034-capability-profiles-and-comparability.md) |
| Specified in | [32](../32-measurement-provenance.md), [22](../22-measurement-feasibility.md) |

## Context

The platform's central promise is defensible progress over time. That requires every number to say
how it was obtained, a clear distinction between observed, estimated, predicted and unavailable,
and a guarantee that later algorithms and models do not silently change what old results mean.
The pieces exist across the design (provenance, grades, classes, snapshots); nothing stated them
as one binding rule.

## Decision

1. The four user-facing statuses map onto the **existing `Provenance` enum** — observed =
   `measured`, estimated = `derived` (and `inferred` for model fill, which is never stored as a
   metric), unavailable = `unavailable` with reasons. **No `predicted` member exists**, because no
   engine component may emit a prediction.
2. Every stored measurement carries or links to: value, unit, grade, provenance, method,
   confidence, sigma and its basis, reasons, metric id and version, comparability class, and —
   through its session — engine version, observable catalogue revision, tracking configuration,
   calibration set with QA, and content snapshots.
3. A session **does not start** if its tracking configuration and calibration set cannot be
   written.
4. Stored measurements, scores and assessment results are **never updated**. Re-analysis writes a
   new linked session.
5. Any change able to alter a number must change one of: metric version, observable catalogue
   revision, detector id, tracking configuration id. Golden-session regression enforces it
   ([ADR-038](ADR-038-golden-sessions.md)).
6. Nothing writes measurements or scores except versioned metric kinds and scoring models.
7. Explanations are assembled from stored fields; the engine generates no prose.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| A separate four-value "status" field beside provenance | Rejected: two vocabularies that can disagree |
| Add `predicted` to the enum for completeness | Rejected: an unused value invites use; adding it later is a deliberate, reviewable act |
| Recompute history in place when algorithms improve | Rejected: history would change meaning silently; comparisons with remembered values break |
| Store only values, derive provenance from the session date and release notes | Rejected: not auditable; fails for mid-session changes |
| Sign records cryptographically | Rejected: no adversary in scope; integrity here is against accident, not tampering |
| Full per-sample lineage graph | Rejected: the session-level records plus recorded inputs already allow recomputation |

## Rationale

Immutability plus recorded inputs gives both properties at once: old results stay what they were,
and anything can be recomputed under new algorithms as a separate, labelled result.

## Advantages

- Every displayed number has a complete, queryable explanation.
- Algorithm and model changes are visible as class or version changes.

## Disadvantages

- More stored context per session; re-analysis multiplies rows.
- Developers must bump an identifier for any numeric change, including bug fixes.

## Risks

An identifier not bumped for a change believed harmless. The golden runner is the guard; its
coverage is only as good as the golden set.

## Consequences

One additive route returns a session's joined provenance. The readiness panel and the feasibility
matrix use the same status words.

## Reconsider when

A display-side predictor is wanted in the engine (for example latency compensation for VR): add
`predicted` by ADR, with the rule that predicted values never reach metrics.
