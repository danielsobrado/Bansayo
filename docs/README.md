# Kinexa documentation

Kinexa is an open-source, local-first platform for capturing, reconstructing, analysing and
visualising human motion from wearable IMUs, cameras, or both.

**Status: design approved by the architect, implementation not started.** Nothing described here
exists as code yet. Each document marks what the first PoC implements and what is only an
extension point.

## Reading order

| # | Document | Answers |
|---|---|---|
| 01 | [Vision and scope](01-vision-and-scope.md) | What we are building, what the PoC proves, the roadmap |
| 02 | [Architecture](02-architecture.md) | Processes, modules, dataflow, technology choices, repository layout |
| 03 | [Domain model](03-domain-model.md) | Coordinates, time, skeleton, provenance, sensor/camera capability model |
| 04 | [Contracts](04-contracts.md) | Wire protocol, HTTP API, configuration schema, future firmware telemetry |
| 05 | [Calibration and fusion](05-calibration-and-fusion.md) | Calibration procedures, profiles, invalidation, fusion roadmap |
| 06 | [Recording and playback](06-recording-and-playback.md) | Session format, replay semantics |
| 07 | [Desktop UI](07-desktop-ui.md) | Navigation, screens, wireframes, visual language, 3D viewport |
| 08 | [VR client](08-vr-client.md) | Godot client, coordinate handling, retargeting, SlimeVR/SteamVR path |
| 09 | [Robustness and quality](09-robustness-and-quality.md) | State definitions, failure and recovery model, performance rules |
| 10 | [PoC plan](10-poc-plan.md) | Stages, agent briefs, file ownership, acceptance criteria, prerequisites |
| 11 | [Risks and validation](11-risks-and-validation.md) | Open risks, decisions that need hardware, deferred validation |
| 12 | [Decisions](12-decisions.md) | Architecture decision records |

## Conventions used in these documents

- **PoC** — implemented in the first vertical slice.
- **Designed** — specified here, deliberately not implemented in the PoC.
- **Extension point** — a named place where a later capability plugs in; no code is created for it yet.
- Type shapes are written in TypeScript notation because it describes JSON precisely. The Rust
  crate `kinexa-contracts` is the implementation source of truth once it exists; TypeScript types
  are generated from it and never written by hand.

## One rule above all others

The system never presents a value as better than it is. Every number shown to a user or sent to a
client carries its origin (live or simulated), its provenance (measured, derived, inferred or
unavailable) and the calibration it depends on. A missing input produces a labelled gap, never a
plausible-looking fabrication.
