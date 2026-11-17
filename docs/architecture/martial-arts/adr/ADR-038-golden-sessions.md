# ADR-038 — Golden sessions: recorded inputs with expected results as the regression gate

| | |
|---|---|
| Status | **Accepted**; synthetic golden sessions first, real ones when cameras are connected |
| Date | 17 November 2026 |
| Extends | ADR-005, ADR-016 in [docs/12](../../../12-decisions.md); [ADR-036](ADR-036-measurement-status-and-immutable-provenance.md) |
| Specified in | [28](../28-dataset-and-regression.md) |

## Context

Tracking, recognition, metrics and scoring will change for years. Sessions already record inputs
and replay deterministically, and replay regression tests exist. What is missing is a curated,
labelled set with expected outcomes at every layer, and a rule tying numeric changes to version
identifiers.

## Decision

1. A **golden session** is an ordinary recorded session plus a manifest entry stating expected
   results: repetition counts, labels against annotations, phase-boundary error, metric statistics
   with tolerances, grade shares, cue limits — optionally under **source masks**.
2. **Synthetic** golden sessions (simulator, scripted truth) live in the repository and run in CI
   on every change. **Real** golden sessions live on the validator's machine, never in the
   repository, and run before releases and whenever a solver stage, observable or shipped
   definition changes.
3. The runner is a **subcommand of the engine binary** reusing the re-analysis job.
4. Expected values are written by the runner on an explicit **bless** step and reviewed as a diff.
5. A changed metric value must be accompanied by a bump of one of the identifiers of
   [ADR-036](ADR-036-measurement-status-and-immutable-provenance.md); the report lists changed
   identifiers.
6. Tuning uses a tuning split; reported figures come from **held-out athletes**.
7. Promotion needs the performer's dataset-use consent; sharing needs a further per-export consent.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| Unit tests on synthetic data only | Rejected: proves mathematics, not behaviour on real noise |
| Record solved output and compare | Rejected: cannot evaluate a changed solver; inputs are the invariant |
| Record video and re-run everything | Kept as an option per session: needed only to evaluate a new pose model; large and privacy-heavy |
| Public benchmark datasets | Rejected: licences unreviewed, different skeletons and sensors |
| A separate evaluation framework or service | Rejected: the engine already contains the pipeline; a subcommand suffices |
| Real recordings in the repository | Rejected: personal data and size |
| Snapshot tests with exact equality on real data | Rejected: tolerances are needed where floating-point or ordering may legitimately differ across platforms |

## Rationale

The inputs are the only thing that stays true when algorithms change. A small, well-labelled set
replayed on every change turns "did this break anything?" into a report.

## Advantages

- Regressions are caught before release, per layer.
- Validation recordings are reused instead of repeated.
- Makes the provenance invariants mechanically checkable.

## Disadvantages

- Curation and annotation effort.
- Golden sessions recorded with a since-changed detector lose value for layers below the
  landmarks unless video was kept.
- Real golden data is tied to one machine and one set of consenting people.

## Risks

A golden set that is too small or too clean gives false confidence; blessing becomes a reflex.
Guard: the diff is reviewed; negative and degraded examples are mandatory.

## Consequences

`AthleteProfile.consent` gains `dataset_use_utc`. A `golden/synthetic/` directory and a
`bansayo.golden/v1` manifest schema. Lab recordings ([29](../29-tracking-hardware-lab.md)) are
captured to golden standard from the start.

## Reconsider when

Several validators need the same real golden set: define an exchange through bundles
([ADR-041](ADR-041-coaching-exchange-by-bundles.md)) with consent recorded per session.
