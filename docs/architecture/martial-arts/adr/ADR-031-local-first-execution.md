# ADR-031 — Local-first: everything on one machine; optional token-protected LAN mode

| | |
|---|---|
| Status | **Accepted**; the LAN mode is implemented for the Android client ([ADR-042](ADR-042-android-client.md), 18 November 2026) |
| Date | 17 November 2026 |
| Specified in | [17 §2](../17-security-privacy-and-safety.md), [14 §1](../14-runtime-and-api-contracts.md), [31](../31-deployment-and-device-lifecycle.md) |

## Context

The engine binds to loopback, has an origin allow-list and no authentication (existing R14). The
product records video and movement data of people at home. A standalone headset cannot run the
engine and would have to reach it over the local network.

## Decision

1. Engine, analytics store, content and both clients run on **one machine** by default, talking
   over `127.0.0.1`.
2. **No cloud**: no account, telemetry, update check or remote call. Ordinary training never needs
   a network.
3. A **LAN mode** is opt-in: a non-loopback bind is refused unless `api.lan.enabled` is set, and
   then every HTTP request and WebSocket upgrade needs a bearer token paired through the desktop
   UI.
4. The engine provides **no TLS**; LAN mode is for a trusted home network and is labelled so.
5. Sharing with another person is **file export**, never a live service
   ([ADR-041](ADR-041-coaching-exchange-by-bundles.md)).
6. The operating-system account is the security boundary; there is no access control between
   local athlete profiles.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| Cloud backend for sync and analytics | Rejected: accounts, operations and a privacy burden for no local benefit |
| LAN service by default | Rejected: an unauthenticated control API on the network |
| TLS with self-signed certificates | Rejected for now: certificate trust on headsets and browsers is its own project; reconsidered with WebXR or untrusted networks |
| mDNS discovery | Deferred: pairing by code is enough for one headset |
| Peer-to-peer sync between installations | Rejected: conflict resolution for no stated use case |
| Per-profile passwords | Rejected: theatre without encryption at rest; the OS account is the real boundary |
| Application-level encryption at rest | Rejected for now: key management; full-disk encryption is recommended instead |

## Rationale

The data never leaving the machine is the simplest privacy guarantee to state and to verify, and it
removes whole classes of failure during training.

## Advantages

- Works offline; nothing to operate.
- Privacy by construction; erasure is local deletion.

## Disadvantages

- No backup or multi-device continuity unless the user copies the data directory.
- A shared family computer exposes profiles to each other.

## Risks

A user enabling LAN mode on an untrusted network. Guard: explicit warning, token required, no
silent fallback.

## Consequences

Remote coaching and dataset collection are designed around exported bundles. Supported deployment
models are enumerated in [31](../31-deployment-and-device-lifecycle.md).

## Reconsider when

- A standalone headset becomes the primary target (TLS and discovery).
- A real multi-user need appears (a school installation).
