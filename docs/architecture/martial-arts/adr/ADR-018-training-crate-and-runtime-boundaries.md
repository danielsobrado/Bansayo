# ADR-018 — Training logic in a pure crate inside the one engine process

| | |
|---|---|
| Status | **Accepted** |
| Date | 17 November 2026 |
| Extends | ADR-001 (modular monolith in two crates), ADR-011 (ordered solver stages) in [docs/12](../../../12-decisions.md) |
| Specified in | [14 §1–3](../14-runtime-and-api-contracts.md), [01 §5](../01-product-vision.md) |

## Context

The engine is one Rust process with two crates: `bansayo-contracts` and `bansayo-engine`. Training
adds observables, recognition, exercise execution, metrics, scoring, coaching and progress
analytics. The brief prefers Rust and Go, suggests eighteen candidate responsibilities, and
requires that exercise logic be independent of sensors and of presentation, without creating
languages or services that have no concrete need.

## Decision

1. The engine stays **one process**. No new service, broker or backend language.
2. A third crate, **`bansayo-training`**, holds all training logic as a pure library: no Tokio, no
   file or network I/O, no clock. Of the workspace crates it depends only on `bansayo-contracts`;
   external crates are allowed when they are pure in the same sense (amended 17 November 2026 by
   the owner: `serde_json` and other pure dependencies are permitted).
3. Inside the engine a **training runner** actor hosts that library. Its only tracking input is
   `MotionSample` plus `CapabilityReport`, delivered over a bounded channel from the pipeline
   runner. It shares no mutable state with tracking.
4. Fusion ends at `BodyState`. Nothing in `bansayo-training` can name a device, source, detector or
   transport, because it cannot import the engine.
5. Go is not introduced. Python is not used at runtime.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| Training as modules inside `bansayo-engine` | Rejected: the sensor-independence rule would rest on review alone |
| A separate training process (Rust or Go) over IPC | Rejected: adds an IPC boundary carrying 120 Hz samples, a second place for session state and, for Go, a second toolchain and a second definition of contract types, for no capability gained on one machine |
| Go for orchestration, device and session management | Rejected: that work is a few Tokio tasks in the process that already owns the devices |
| Training logic in the clients | Rejected: two implementations (TypeScript, GDScript) of recognition and scoring; violates one-engine-two-presentations |
| Move every pure module to a `bansayo-core` crate now | Deferred: ADR-001's trigger (a second consumer of the solver mathematics) has not occurred. Only the joint-angle functions move, because training needs them |
| One crate per responsibility | Rejected: boundaries without a compiler-checked reason |

## Rationale

The one boundary worth a crate is the one the product promise depends on: technique, workout and
scoring logic must be unable to see hardware. A crate makes that structural at zero runtime cost.
Purity also gives deterministic replay and fast table tests for free.

## Advantages

- Adding a camera, tag or IMU cannot require a change in exercise logic.
- The training library is testable without the runtime and reusable by an offline tool.
- A slow or failed training runner cannot delay tracking.

## Disadvantages

- One more crate to version; contract types needed by both sides must live in `bansayo-contracts`.
- `pipeline/joints.rs` now depends on the training crate for angle formulas — one edge pointing
  "upward" in the conceptual pipeline.
- A crash in training code still takes the process down (as any module would).

## Risks

`MotionSample` becoming a dumping ground for device-specific fields. Guard: it is a contract type;
every field is reviewed against "would this exist with a different sensor?".

## Consequences

`MotionSample`, `BodyState`, `Evidence`, `CapabilityReport` and all training wire types are added
to `bansayo-contracts`. Re-analysis is the same library driven by a recording. Long CPU work (DTW
over long clips, form alignment) runs on blocking threads and reports through the runner.

## Reconsider when

- Inference or camera instability must not take training state down: move sources, not training,
  into a sidecar (ADR-006's packaging clause).
- A second consumer needs solver mathematics without the runtime: create `bansayo-core`.
- A deployment needs training analysis on a different machine from capture.
