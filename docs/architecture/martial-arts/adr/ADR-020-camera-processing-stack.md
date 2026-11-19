# ADR-020 — Camera processing stack: native capture threads, no OpenCV

| | |
|---|---|
| Status | **Pending Validation** — validated on one webcam (18 November 2026, below); four cameras together not yet run |
| Date | 17 November 2026 |
| Extends | ADR-009 (camera path) in [docs/12](../../../12-decisions.md) |
| Specified in | [08 §3](../08-tracking-and-sensor-fusion.md), [15 §2.1](../15-performance-and-latency.md) |

## Implementation note (17 November 2026)

Commits `ea9b5d7` and `9aa9904` built items 1, 2, 3 (decoding through the `image` crate) and 6 of
the decision below, plus image-folder cameras; see
[08 §1.2](../08-tracking-and-sensor-fusion.md). No physical camera has been connected. Item 4,
manual exposure, is not built. The status therefore stays Pending Validation, now on hardware
rather than on a spike.

## Evidence on the first physical camera (18 November 2026)

Roadmap Phase 2, on the owner's Windows 11 PC with one UGREEN USB webcam (VID 0C45, PID 6369;
offers MJPEG and NV12 up to 1920×1080, at most 30 fps). Commits `04dc99f`, `a11831f`, `56080aa`.

| Criterion | Result |
|---|---|
| Manual exposure set and read back | **Met, through the fallback.** `nokhwa` 0.10 cannot switch a Media Foundation control from automatic to manual (its `set_camera_control` keeps the current flag), so item 2's fallback was built for controls only: `sources/camera/controls.rs` activates the device's media source beside nokhwa's stream and sets `IAMCameraControl` exposure (whole log2 seconds) and `IAMVideoProcAmp` gain to manual, then reads them back. 0.004 s requested → 1/256 s in effect; 0.002 s → 1/512 s; also changed live over `PUT /cameras/{id}/settings`. A setting the device or an image folder cannot take leaves the camera `degraded` with the property named. Capture itself stays on `nokhwa` |
| Advertised rate within 5 % | **Met at 30 fps**: 30.0 frames/s raw capture and 30.0 Hz end to end (release build, manual exposure). `nokhwa`'s `Closest` request picked a 1 fps mode, and its reported rate is the ratio's denominator; the engine now requests the exact format first and does not log nokhwa's rate |
| Four cameras together | Not run: one camera attached |
| Decode budget (≤ 4 ms, 1280×720) | **Borderline**: p50 2.5 ms, p95 4.3 ms, p99 6.5 ms over 10 s (`image` crate with `zune-jpeg`). Consequence: the `turbojpeg` trigger of item 3 is reached at p95 only; it is not taken until four-camera load is measured, and the stage-over-budget notice reports it when it happens. In debug builds the decoder is compiled optimised (dev profile), without which capture fell to 7–10 fps |

## RTSP cameras (19 November 2026)

Item 7 is built for the owner's Reolink and Aqara Wi-Fi cameras: a camera entry with `rtsp:` is
read through an `ffmpeg` process (TCP transport, 5 s socket timeout, MJPEG on stdout scaled to the
declared size), decoded in-process like USB MJPEG. This uses `ffmpeg` as a subprocess, which the
table below rejects for USB capture (pipe latency, no exposure control, timestamps lost): for RTSP
those costs are already in the stream itself (the camera sets its own exposure, the stream adds
0.2–0.5 s and gives no capture time), and an H.264/H.265 decoder in-process would be a much larger
dependency. RTSP cameras stay out of triangulation without a `time_sync` profile, as written.

## Implementation note (19 November 2026): camera identity and lifecycle

Cameras are matched by identity at every open, never by list index (`sources/camera/identity.rs`):
serial (only when Windows exposes a real one; composite UVC interfaces never do), then the device
interface path, then `vid/pid` when unique. Two identical cameras named only by model are refused.
A per-camera profile (`<data_dir>/cameras/<id>.json`) records the hardware the calibration
belongs to; a different unit under the same id keeps 2D pose but its 3D calibration counts as
stale until the user confirms (`POST /cameras/{id}/identity/confirm`). Modes are chosen among the
advertised ones (presets, `camera_management.fallback_fps`); an unsupported rate is never
requested as if available. nokhwa 0.10 reports Media Foundation rates by their denominator and
drops non-integer rates, so after a non-exact open the negotiated rate is unknown and the
measured rate (`camera_runtime.timing`) is the authority. Linux V4L2 identity is not built.

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
