# ADR-022 — Sensor ingestion: every input is a source adapter; no plugin loader

| | |
|---|---|
| Status | **Accepted** for the structure; each new adapter is Planned or Proposed until built |
| Date | 17 November 2026 |
| Extends | ADR-008 (SlimeVR first) in [docs/12](../../../12-decisions.md) |
| Specified in | [08 §1–2, §5–6](../08-tracking-and-sensor-fusion.md), [11 §6](../11-vr-architecture.md) |

## Implementation note (19 November 2026)

`slimevr_solarxr` is built (`sources/slimevr/`). Verified against SlimeVR's own sources rather
than assumed: SlimeVR Server serves SolarXR on `ws://127.0.0.1:21110` (`WebsocketAPI.java`), one
FlatBuffers `MessageBundle` per binary message; a `StartDataFeed` must set
`DeviceDataMask.device_data` or no tracker is sent. A data feed is a **snapshot** of the server's
latest state with **no sample timestamp**, and the server never fills `raw_angular_velocity` or
`raw_acceleration` (`DataFeedBuilder.kt`), so O-08 is answered: SolarXR gives fused orientation,
status, tps, battery and RSSI only. Consequences: `time_quality: receive_time` with a configured
`latency_ms`; repeated snapshots are not republished (100 ms keep-alive); angular velocity stays
`unavailable` for SlimeVR trackers. Rotations are right-handed +Y up in both systems, so the
conversion is validation only; the reference frame's heading is calibrated as for any tracker.
Identity is the server's `hardware_identifier` plus tracker number; trackers without one are
ignored rather than given an identity that changes on reconnect.

## Context

The engine has a source task model with bounded ingest, validation, supervision and a session
clock, exercised only by simulated and replay sources. Training needs cameras, IMUs, fiducial
clusters and, optionally, XR devices, depth, ranging and contact sensors — each optional, each
individually enabled, monitored and calibrated. Reading the code found defects of fit (T-01 – T-07).

## Decision

1. Every input, including XR devices and marker clusters, enters as a **`Source` emitting
   `SourceEvent`s** through the existing bounded ingest. No second path into the solver exists.
2. Sources are compiled in and selected by `kind` in configuration: **one match arm per kind** in
   the composition root. No dynamic loading, no `dyn Source`.
3. The registry keeps the **latest observation per (device, observation kind)** (fixes T-01).
4. Every device clock is mapped through the existing **offset estimator**; sources without one
   stamp arrival time minus a configured latency and say so in `time_quality`.
5. IMUs: **SlimeVR Server through SolarXR first** (ADR-008 kept); **`esp32_udp`** second, when raw
   rates, synchronised time or auxiliary sensors are needed. The engine does not re-fuse raw IMU
   data into orientation.
6. XR devices are ingested from the VR client over a **client → engine WebSocket**
   (`client_ingest`) as ordinary observations in frame `xr_stage`.
7. Marker clusters are **devices announced by an internal source**, so they get state, assignment
   and calibration like trackers.
8. Depth cameras, UWB, time-of-flight and contact sensors are **named extension points** with their
   observation kinds reserved; none is built until owned and wanted.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| Dynamic plugins (shared libraries or WASM) for sources | Rejected: ABI and sandbox machinery for a handful of adapters maintained in this repository |
| An MQTT broker between ESP32 trackers and the engine | Rejected (existing ADR-003 reasoning): infrastructure with no benefit on one machine |
| Own SlimeVR tracker protocol implementation | Rejected (existing ADR-008): undocumented protocol |
| The engine as an OpenXR application to read the headset | Rejected: only one process can own the XR session comfortably; the VR client already has the poses |
| An OpenVR driver to read or inject trackers | Out of scope; evaluated separately if SteamVR interop is ever wanted |
| OSC tracker feeds | Not chosen: no timestamps, lossy semantics; could be added as a source kind |
| Bluetooth LE trackers | Not chosen: rate and latency; could be added as a source kind |
| Raw IMU fusion in the engine | Rejected: the sensor's own fusion already solves tilt; re-fusing gains nothing without a reason |

## Rationale

One ingestion path means one validation, one clock, one recording format and one supervision model
for everything, and replay covers every sensor type for free.

## Advantages

- A new sensor is one adapter; the solver and training are untouched.
- Every input is recorded and replayable, including XR poses and tag corners.
- Disabling a source is one configuration flag.

## Disadvantages

- Adding a source kind needs a rebuild.
- XR input depends on the VR client being connected.

## Risks

SolarXR may not expose angular rates ([19 §3](../19-risks-and-open-questions.md) O-08); Wi-Fi loss
and jitter for UDP trackers; ingest socket abuse in LAN mode
([17 §2.2](../17-security-privacy-and-safety.md)).

## Consequences

`SourceContext` is split into a common part and a simulator extension (T-02). New observation kinds
`fiducials` and `pose`; new device kinds. Validation keeps the 1000 Hz limit; faster IMUs decimate
in firmware.

## Reconsider when

- Third parties need to add sources without forking: revisit a plugin boundary.
- A sensor requires sub-millisecond coupling with the solver that sample-and-hold cannot give.
