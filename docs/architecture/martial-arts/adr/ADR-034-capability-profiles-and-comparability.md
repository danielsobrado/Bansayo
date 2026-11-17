# ADR-034 — Observability decides features; comparability classes protect history

| | |
|---|---|
| Status | **Accepted** |
| Date | 17 November 2026 |
| Specified in | [08 §9](../08-tracking-and-sensor-fusion.md), [12 §5](../12-data-and-progress.md), [32](../32-measurement-provenance.md) |

## Context

The platform must run on one camera and improve with more sensors, and must show progress over
months. A reading that changes because a camera was added, a model replaced or the floor
recalibrated is not progress. The brief suggests quality tiers but warns that they are not
accuracy levels.

## Decision

1. **Features follow observables.** The engine publishes a `CapabilityReport` listing every
   observable's availability, method and reasons. Training logic reads that; no code branches on
   device counts.
2. A **capability profile** (`basic`, `inertial`, `multi_view`, `marker_assisted`, `hybrid`,
   `extended`) is a **label derived** from the same facts for display. An override may only lower
   it.
3. Every session records a **`tracking_config_id`** and a **`calibration_set_id`** (content hashes
   of what was connected and how it was solved, and of the calibration in use).
4. Every measurement belongs to a **comparability class**: a hash of metric id and version, method
   class, detector id and normalisation basis.
5. Trends, baselines, records, relative goals and scoring targets are computed **only inside one
   class**; different classes are separate series; a calibration-set change inside a class draws a
   **break marker**.
6. After a tracking upgrade, **continuity re-analysis** re-solves new recordings with the added
   sources masked, continuing the old series on identical motion and yielding a paired **method
   difference** that is displayed and **never applied** to stored values.
7. Assessments declare a required profile and accepted methods; a run outside them is `invalid`.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| Device-count tiers that switch features on | Rejected: three badly placed cameras are not better than two good ones; observability is the fact |
| One accuracy score per setup | Rejected: accuracy is per metric and operating range, and unmeasured |
| Merge series with a calibrated cross-method offset | Rejected: the offset is athlete-, technique- and range-dependent; applying it would manufacture data |
| Annotate changes but keep one series | Rejected: a line through a method change reads as progress |
| Class includes the engine version | Rejected: would fragment history on every release; algorithm changes that alter numbers change the metric version or the tracking configuration instead |
| Learned harmonisation across sensors | Rejected: no data; unexplainable |

## Rationale

Refusing to join incomparable numbers is cheaper and more defensible than any attempt to correct
them. Recorded inputs make the continuity mechanism possible.

## Advantages

- "Improved movement or improved sensing?" has an operational answer.
- Graceful degradation is automatic: a lost sensor changes the class of later repetitions only.

## Disadvantages

- History fragments when the setup changes ([19 §1](../19-risks-and-open-questions.md) R29).
- Continuity re-analysis needs input recordings and background compute.
- More concepts for the athlete to understand.

## Risks

A change that alters numbers without changing any hashed component. Guard: the golden-session
regression of [ADR-038](ADR-038-golden-sessions.md) fails when stored values move.

## Consequences

The Progress screen draws per-class series, break markers and method differences. Reference
movements record their own profile.

## Reconsider when

V-34 shows that two method classes agree within their minimum detectable change for a metric over
the tested range: those two classes may then be declared equivalent **for that metric**, by a
versioned table, with the report cited.
