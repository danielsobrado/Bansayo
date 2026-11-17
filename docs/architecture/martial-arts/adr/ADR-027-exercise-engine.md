# ADR-027 — One pure exercise engine; every mode is a plan

| | |
|---|---|
| Status | **Accepted** |
| Date | 17 November 2026 |
| Specified in | [06](../06-exercise-and-workout-engine.md), [03](../03-training-modes.md) |

## Context

Seven training modes, two clients. The brief forbids duplicated exercise engines and asks for
declarative sequences with branches, repeats, sides, timing, tempo and rest, executable unchanged
on desktop and in VR.

## Decision

1. A single **`ExerciseEngine`** in `bansayo-training`: a pure state machine stepped with ticks,
   observable frames, movement events, repetition results, commands and capability changes. It
   returns training events and exposes an authoritative `TrainingState`.
2. **Every mode is a plan kind.** A single technique is a one-block, one-item workout: one
   execution path.
3. Combinations and forms share **one sequence grammar** (`step`, `repeat`, `choice`, `optional`,
   `rest`) compiled to a finite automaton with bounded nesting and length.
4. Progression rules, goal comparisons and condition keys are **closed sets**.
5. The engine **owns the session**. Clients send commands; any client may; commands are applied in
   arrival order. Time is only the session-clock time the engine is given.
6. Safety limits are enforced here, not in clients.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| One engine per mode | Rejected: duplicated lifecycle, counting and storage |
| Exercise logic in each client | Rejected: two implementations; a session could not be shared |
| Behaviour trees | Rejected: more expressive than needed; harder to validate and to show as a preview |
| Embedded scripting (Lua, Rhai) in workouts | Rejected: unrestricted rule programming; untestable content ([ADR-030](ADR-030-configuration-and-content.md)) |
| A general statechart or workflow library | Rejected: the machines are small; a dependency would add its own semantics |
| Event log only, clients rebuild state | Rejected: every client would re-implement the reducer; state is published, events are increments |

## Rationale

Purity makes sessions replayable and the machines table-testable. One path means a feature added
for workouts exists for every mode. Closed rule sets keep content checkable by a validator and
previewable by the builder.

## Advantages

- A workout saved on the desktop runs in VR unmodified.
- Identical inputs reproduce identical event logs.
- The Workout Builder can enumerate every possibility.

## Disadvantages

- A need outside the closed sets is a code change.
- The engine must anticipate presentation needs (targets, beat grid, instructions) as data in
  `TrainingState`.

## Risks

`TrainingState` growing with presentation-specific fields. Guard: a field is added only if both
clients need it to stay consistent.

## Consequences

Recognition receives a `RecognitionContext` from the engine and returns results as inputs; the two
never call each other. Sessions are not resumed after an engine restart.

## Reconsider when

- Reaction and decision drills ([27](../27-reaction-and-decision-drills.md)) need stimulus timing
  the tick cannot express.
- Instructors repeatedly need a sequence construct the grammar lacks.
