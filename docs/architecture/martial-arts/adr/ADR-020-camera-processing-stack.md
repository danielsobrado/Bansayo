# ADR-020 — Camera processing stack: native capture threads, no OpenCV

| | |
|---|---|
| Status | **Pending Validation** — the capture crate, decoder and manual exposure control are untested on the target machine |
| Date | 17 November 2026 |
| Extends | ADR-009 (camera path) in [docs/12](../../../12-decisions.md) |
| Specified in | [08 §3](../08-tracking-and-sensor-fusion.md), [15 §2.1](../15-performance-and-latency.md) |

## Implementation note (17 November 2026)

Commits `ea9b5d7` and `9aa9904` built items 1, 2, 3 (decoding through the `image` crate) and 6 of
the decision below, plus image-folder cameras; see
[08 §1.2](../08-tracking-and-sensor-fusion.md). No physical camera has been connected. Item 4,
manual exposure, is not built. The status therefore stays Pending Validation, now on hardware
rather than on a spike.

## Context

When this record was drafted no real camera code existed. ADR-009 deferred the capture crate to a Windows spike. Martial-arts
movement adds requirements ordinary webcam use does not have: short manual exposure, the highest
frame rate the camera offers, several cameras at once, per-frame arrival timestamps and no frame
queueing.

## Decision

1. One **OS-thread chain per camera** (capture → decode → fiducials / pose crop), outside the async
   runtime, with a one-slot latest-frame mailbox between capture and decode.
2. Capture through **`nokhwa`** on Media Foundation; fallback, inside the same module boundary, to
   a thin wrapper over the `windows` crate.
3. **MJPEG** at the camera's highest frame rate for the chosen resolution; decode with
   **`zune-jpeg`**; `turbojpeg` only if a 1280×720 decode measures above 4 ms.
4. **Manual exposure set by the engine**; auto-exposure off.
5. **No OpenCV.** Fiducial detection, calibration and triangulation are separate small components
   ([ADR-024](ADR-024-fiducial-markers-and-board-calibration.md), existing ADR-014).
6. Preview is a separate low-rate JPEG HTTP stream, served only on request.
7. RTSP cameras are accepted for slow work only and excluded from triangulation without a
   `time_sync` profile.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| OpenCV (`opencv` crate) for capture, decode, ArUco and calibration | Rejected: a large C++ dependency and build burden on Windows to obtain four functions that have small alternatives; its capture layer hides timestamps and buffering |
| GStreamer pipelines | Rejected: a runtime to install and configure; more than is needed for UVC |
| `ffmpeg` as a subprocess | Rejected: pipe latency, no exposure control, timestamps lost |
| A Python capture process (as FreeMoCap's camera layer does) | Rejected: Python at runtime; AGPL code cannot be reused (ADR-014) |
| Media Foundation directly from the start | Kept as the fallback; more code than `nokhwa` if `nokhwa` suffices |
| Capture on the async runtime | Rejected: blocking device calls; the Windows timer granularity already forced a ticker thread |

## Rationale

The stack must give control over exposure and expose arrival time, and must drop stale frames
rather than queue them. Threads with a mailbox do exactly that with little code. Avoiding OpenCV
keeps the build what it is today: `cargo build`.

## Advantages

- Short, inspectable latency path; back-pressure by replacement, counted.
- The capture backend can be swapped behind one module.

## Disadvantages

- Camera control properties vary by device and driver; a matrix of tested cameras will be needed.
- Own code for things OpenCV bundles.

## Risks

`nokhwa` may not expose manual exposure or high-frame-rate MJPEG modes reliably (existing R6;
[19 §3](../19-risks-and-open-questions.md) O-03). USB bandwidth with several cameras on one
controller.

## Decision criteria (the spike)

Accepted as written if, on the target machine with the owner's cameras: manual exposure can be set
and read back; the advertised frame rate is delivered within 5 %; four cameras run together; decode
meets its budget. Otherwise the fallback wrapper replaces `nokhwa`, and this record is updated.

## Consequences

Camera requirements become product guidance ([08 §3.1](../08-tracking-and-sensor-fusion.md)).
Video recording stores the camera's own MJPEG frames ([ADR-029](ADR-029-session-and-analytics-storage.md)).

## Reconsider when

- A camera class that matters (industrial, hardware-triggered) needs a vendor SDK: add a source
  kind beside `camera`.
- Linux or macOS becomes a target: the backend is per platform.
