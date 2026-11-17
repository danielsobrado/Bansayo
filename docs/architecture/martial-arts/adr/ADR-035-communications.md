# ADR-035 — Communications: bounded channels inside, additive JSON topics outside

| | |
|---|---|
| Status | **Accepted** |
| Date | 17 November 2026 |
| Extends | ADR-003, ADR-004, ADR-006 in [docs/12](../../../12-decisions.md) |
| Specified in | [14 §3–9](../14-runtime-and-api-contracts.md) |

## Context

Clients receive server-push JSON over WebSocket and control the engine with JSON over HTTP, under
one `protocol_version`. Training adds a session shared by desktop and VR, live observables, event
history, content, athletes, progress, references, calibration procedures and, later, client-side
tracking input.

## Decision

1. **Inside the process:** in-process calls within an actor; typed **bounded channels** between
   actors and threads, each with a written overflow policy. The analytics channel never drops.
2. **Engine ↔ clients:** JSON over WebSocket and HTTP is **kept**. Training is added as **new
   topics and routes under `/api/v1`**; `protocol_version` stays 1; `hello.capabilities` lists
   feature flags so clients hide what an engine lacks.
3. **State is authoritative, events are increments.** `training_state` is replace-only;
   `training_event` carries a per-session sequence number; a client that sees a gap refills over
   HTTP.
4. **Client → engine tracking input** uses a separate ingest WebSocket carrying ordinary
   `SourceEvent`s.
5. **Camera preview** is a low-rate multipart JPEG HTTP stream, never on the WebSocket.
6. A **binary encoding** of high-rate messages remains reserved as an additive, negotiated option
   after a measured bottleneck (ADR-003).
7. Types are defined once in Rust; TypeScript is generated; golden examples guard the Godot client
   (ADR-004).

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| gRPC / gRPC-web | Rejected (existing): no native browser or Godot support |
| Protobuf, MessagePack or CBOR over WebSocket now | Rejected: no measured bottleneck; loses terminal readability |
| Server-sent events | Rejected: a second streaming mechanism beside the WebSocket |
| WebRTC data channels and video | Rejected: signalling and NAT machinery for a loopback link |
| Shared memory for frames | Rejected: clients never need full-rate frames |
| MQTT or another broker | Rejected (existing) |
| Tauri IPC | Rejected (ADR-006) |
| Events only, no state snapshot | Rejected: late joiners and reconnects would need full replay |
| Bump `protocol_version` to 2 | Rejected: every change is additive; older clients ignore unknown types |

## Rationale

The existing contract already tolerates unknown messages and fields, so training can be added
without breaking the tracking clients. The training stream is low-rate; JSON cost is dominated by
`frame`, which is unchanged.

## Advantages

- One contract for browser, native shell and Godot.
- Reconnect is trivial: hello, state, refill.

## Disadvantages

- JSON size for `observables` and `frame` at high rates.
- Two transports (stream and HTTP) for one session.

## Risks

Serialisation cost with landmark streams from several cameras and several clients (existing R12).

## Consequences

Golden examples are added for every new message. The `diagnostics` topic exposes per-stage timing.

## Reconsider when

Stage timing ([15 §4.1](../15-performance-and-latency.md)) shows serialisation or client parsing
over budget: add the negotiated binary encoding for `frame`, `landmarks_*` and `observables`.
