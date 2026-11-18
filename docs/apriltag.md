# Optional AprilTag 3 camera detection

Status: Phase A (raw detection and visualization). This is **not** wearable pose estimation,
camera calibration, or validated sensor fusion. The engine's existing ChArUco calibration procedure
remains unchanged.

## Installation

AprilTag detection is disabled by default. Build the upstream C library from
https://github.com/AprilRobotics/apriltag at a pinned upstream commit using CMake:

```sh
cmake -S apriltag -B apriltag/build -DCMAKE_BUILD_TYPE=Release
cmake --build apriltag/build --config Release
```

Specify the built shared library via `fiducials.library_path`. It must match the
machine's architecture and must export the AprilTag 3 C API. On Windows, an
MSVC-compatible build and its runtime dependencies are required. Upstream only
officially supports Linux; Windows must be validated on the actual target PC.

AprilTag's BSD-2-Clause notice must accompany distributed library binaries.

## Configuration

Use the existing nested camera-source syntax. Add this section alongside
`engine`, `sources`, and the other top-level keys:

```yaml
fiducials:
  enabled: true
  library_path: ./native/apriltag.dll  # Windows; use a .so or .dylib on other platforms
  family: tag36h11
  max_detection_hz: 15
  max_hamming: 1
  min_decision_margin: 30
  allowed_ids: []                      # empty permits all decoded IDs
sources:
  - id: cams
    kind: camera
    models_dir: ./models
    cameras:
      - id: CAM-FRONT
        device: 0
        resolution_px: [1280, 720]
        rate_hz: 30
        fiducials: true
```

The normal camera and ONNX pose path remains active. AprilTag runs in its own
best-effort worker using bounded latest-frame buffers, so slow detections do not
queue up and increase pose inference latency. When the native library is absent
or fails to initialize, the source sends an `apriltag_unavailable` notice and
normal camera processing continues.

## Observe tags

The desktop **Cameras** workspace has a *Show AprilTag markers* checkbox. It
draws the decoded tag corners and IDs over the preview while enabled.

The raw inspection API is:

`GET /api/v1/cameras/CAM-FRONT/fiducials`

A successful response contains `ok: true` and `data: {camera_id, t_us,
image_size_px, tags}`. Each tag includes its family, ID, four corners in original
image pixels, corrected Hamming bits, and decision margin. A 404 means no fresh
frame has been processed within 500 ms; it does not imply the camera is broken.

Do not interpret the returned corners as calibrated 3D location or body
orientation. The endpoint serves transient inspection only. **TODO:** add
the observation contracts, MCAP replay path, rigid-body pose/mounting calibration,
multi-view quality gates, and IMU fusion in the later marker milestones.

## Validation checklist

- Existing non-AprilTag camera tests and desktop tests still pass.
- A known `tag36h11` print is detected with a reported ID and four corners.
- Unknown registered IDs can be rejected using `allowed_ids`.
- Test static tags at multiple distances, resolutions, lighting levels, and oblique angles.
- Measure the fiducial stage p95 against the 8 ms engineering target on real cameras.
- Confirm two to four simultaneous cameras maintain their existing pose frame rate.
- Unplug one camera and check that all other camera streams continue.
- Verify startup with `fiducials.enabled: false` never requires the native library.
