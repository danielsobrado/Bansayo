# ADR-033 — VR: Godot 4 with OpenXR on PC, presentation only

| | |
|---|---|
| Status | **Pending Validation** — the client runs on Godot 4.5.1 in desktop mode (commit `21828e3`); it has not run in a headset, and frame and parse budgets are unmeasured |
| Date | 17 November 2026 |
| Extends | ADR-010 in [docs/12](../../../12-decisions.md) |
| Specified in | [11](../11-vr-architecture.md), [25](../25-mixed-reality-and-avatars.md) |

## Context

A Godot 4 project with OpenXR and GDScript exists and consumes the engine's stream; it was written
without Godot installed. VR must be a first-class client of the same training session, safe for
kicking and striking, and must not duplicate analysis.

## Decision

1. **Godot 4 + OpenXR + GDScript** is kept (ADR-010).
2. First target is **PC VR**: the headset is driven by the PC that runs the engine; the client
   talks to loopback.
3. **WebXR is rejected** for the product.
4. The client **renders, guides and enforces space safety**; it computes no kinematics, metrics,
   scores or recognition. Targets are placed and judged by the engine.
5. **No first-person body** is rendered from the engine's stream; only the runtime's own hands or
   controllers appear in first person.
6. Vendor extensions (passthrough, hand tracking, body tracking) are used **only behind capability
   checks** and never required.
7. Headset-side safety (boundary, zone, tracking loss) is handled **in the client**, without
   waiting for the engine.
8. A standalone headset build waits for LAN mode ([ADR-031](ADR-031-local-first-execution.md)).

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| WebXR in a browser, reusing the three.js viewport | Rejected: secure-context requirement against a local engine, no tracker roles or body extensions, garbage-collection pauses, uneven passthrough |
| Unity or Unreal | Rejected: licence and toolchain weight; no existing investment |
| A native Rust OpenXR client | Rejected: ecosystem maturity unverified; discards the Godot work |
| godot-rust native extension now | Deferred (ADR-010): a native build per platform for no functional gain until parsing cost is measured |
| An OpenVR driver feeding other applications | Out of scope |
| First-person full-body avatar | Rejected: body stream latency of the order of a tenth of a second under the user's own head |

## Rationale

The client's work is parsing JSON, setting transforms and drawing; a scripting-level engine with
OpenXR support is sufficient and already present. PC VR keeps cameras, inference, engine and
headset on one machine with no network exposure.

## Advantages

- Same session and content as the desktop.
- Runtime-portable through OpenXR.

## Disadvantages

- Tethered or streamed headset required at first.
- Nothing shared with the desktop UI except the contract.
- GDScript performance ceiling.

## Risks

Client never run ([19 §3](../19-risks-and-open-questions.md) O-10); GPU contention with inference
(R32); runtime differences in tracker and passthrough support (existing R5, O-14).

## Decision criteria

Accepted when, in a headset: the session starts through OpenXR; application frame time and message
parse time meet [15 §2.3](../15-performance-and-latency.md) (L-07); the boundary gates and
tracking-loss pause behave as specified. If parse time exceeds 2 ms per frame message, the
native-extension fallback of ADR-010 is taken.

## Consequences

XR devices can later be a tracking source through the ingest socket
([ADR-022](ADR-022-sensor-ingestion.md)).

## Reconsider when

A standalone headset becomes the primary target and LAN mode with TLS exists (WebXR and native
Android export are then compared again).
