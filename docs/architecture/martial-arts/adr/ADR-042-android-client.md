# ADR-042 — Android client: phone camera, live viewer and admin over LAN mode

| | |
|---|---|
| Status | **Accepted** (owner decision, 18 November 2026); validation on a phone pending |
| Date | 18 November 2026 |
| Extends | [ADR-031](ADR-031-local-first-execution.md) (implements its LAN mode), [ADR-020](ADR-020-camera-processing-stack.md), [ADR-021](ADR-021-pose-estimation-integration.md) |
| Specified in | [17 §2](../17-security-privacy-and-safety.md) (network row), [14 §4.1](../14-runtime-and-api-contracts.md) |

## Context

The owner wants an Android app that (1) serves the phone's camera to the engine as one more camera,
(2) shows the live pose, training state, counts and cues, and (3) administers the engine: athletes,
sessions, camera settings and device status. The engine runs on the PC with the GPU and the models.
Before this decision the roadmap had no phone client; Android appeared only as the standalone
headset build of Phase 6f, and LAN mode (ADR-031 item 3) was Proposed.

Owner choices (18 November 2026): **same Wi-Fi only** for now; **pose estimation on the PC**;
**native Kotlin**; first version with **camera source, live viewer and admin**.

## Decision

1. **LAN mode is implemented as ADR-031 and 17 §2 specify.** `api.lan.enabled` must be set for a
   non-loopback bind (a configuration error otherwise). With it, every HTTP request and WebSocket
   upgrade from a non-loopback peer needs `Authorization: Bearer <token>`; the token comes from
   `api.lan.token_file` (default `<data_dir>/lan-token`, created with 128 random bits on first
   start), is compared in constant time, and a wrong or missing token answers HTTP 401 with the
   additive `ApiErrorCode` `unauthorized`. Loopback peers (the desktop on the same PC) keep today's
   access: the operating-system account is the boundary (ADR-031 item 6). No TLS; the desktop and
   this record say the mode is for a trusted home network.
2. **Pairing** is shown in the desktop (Devices): a QR code and text for
   `bansayo://pair?host=<lan address>&port=<port>&token=<token>`, served by
   `GET /api/v1/lan/pairing` to loopback peers only. The Android app opens that link (from any QR
   scanner) or takes the three values typed in. No discovery (mDNS stays deferred).
3. **The phone camera is a camera source, not a `client_ingest` source.** Because pose runs on the
   PC, the phone sends **JPEG frames**, not the `SourceEvent` landmarks 14 §4.1 describes. A camera
   entry `{ id: PHONE1, network: true, resolution_px, rate_hz }` declares it; the phone connects to
   `GET /api/v1/ingest/camera/{id}` (WebSocket, token in LAN mode) and sends one binary message per
   frame. The engine feeds them into the same capture → decode → pose chain as a USB camera: same
   detector id, same comparability class, `time_quality: receive_time`, `latency_ms` from
   configuration. A one-frame buffer drops stale frames and counts them. No frame for 2 s marks the
   camera `disconnected`; reconnecting resumes it. The 14 §4.1 landmark ingest socket stays for
   clients that bring their own pose (XR devices).
4. **Exposure of a phone camera is set on the phone** (Camera2 through CameraX, in the app's camera
   screen). `exposure_s` / `gain` on a network camera are refused by validation, so no setting is
   requested that the engine cannot apply.
5. **Viewer and admin use the existing API unchanged**: `/stream` (frame, state, landmarks_2d,
   training topics), `/cameras/{id}/preview.jpg`, athletes, content index, training sessions and
   commands, camera settings. The app computes no kinematics, grades or recognition (10 §4 rule).
6. **Stack**: Kotlin, platform Android views (no Compose or AppCompat), CameraX for capture, OkHttp
   for HTTP and WebSocket, `org.json` from the platform. `minSdk` 26. Source in `apps/android`.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| Pose on the phone, landmarks over the 14 §4.1 ingest socket | Not chosen by the owner: a mobile model needs its own terms review and forms a separate comparability class; kept possible through `client_ingest` |
| RTSP from a phone streaming app | Rejected: no pairing or token, no app for viewing or admin, extra transport in the engine |
| A virtual-webcam app (phone driver on Windows) | Works without code for the camera part only; no viewer or admin; not controllable |
| Remote access over the internet | Deferred by the owner; when wanted, a private VPN (WireGuard/Tailscale) keeps ADR-031's model; public exposure would need TLS and accounts |
| Jetpack Compose / React Native | Compose is not in the local build caches and adds weight for a few screens; React Native needs native modules for frame streaming |

## Consequences

- The engine accepts network connections only with LAN mode on and a token; the default
  configuration is unchanged (loopback, no token).
- Phone-camera timing is `receive_time` (Wi-Fi jitter of several milliseconds): timing metrics from
  a phone are graded as from any receive-time camera; L-01 for a phone is an owner measurement.
- The phone's frames cross the home network unencrypted; the desktop warns when LAN mode is on.

## Reconsider when

- Remote access is wanted (VPN first).
- A phone pose model with reviewed terms is wanted (landmark ingest).
- More than a few phones stream at once (Wi-Fi bandwidth, about 2–6 MB/s per 720p stream).
