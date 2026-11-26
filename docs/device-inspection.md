# Live device inspection

Open the web UI's Devices workspace and select **Live diagnostics**. Select a single device in the left panel. This isolates the presentation, not the tracking solver. The engine continues to ingest all configured devices.

For native ESP32 trackers, check device health and clock uncertainty, compare the isolated orientation with physical rotation, inspect acceleration and angular velocity separately and select an individual axis. The local reference cube displays the sensor's reported quaternion, before mounting or body fusion. A stationary-window indicator is advisory; it is not a validated calibration result. Use the Mosaico's touchscreen for board bias calibration, and the targeted reference-pose capture for one mounted tracker. The latter uses the existing engine calibration validation. Other device types show only available measurements, with camera preview and AprilTag overlays reusing the Cameras workspace.

Freeze traces to examine a moment, and export the bounded snapshot as JSON. Health metrics continue updating even when traces are frozen. For full-rate capture, use the existing session recorder. The inspection endpoint uses the authenticated/origin-checked HTTP router and returns only accepted observations, decimated to at most 50 Hz per component, 256 samples per channel. No camera images are retained by this path. Data is bounded per announced device, reset on link transitions and playback start, and live inspection is disabled during replay.

## Bench checks

1. Select the Mosaico. Compare measured versus configured rate, dropped count, clock quality and RSSI; a local UDP send is not an acknowledgment.
2. Keep the unit stationary for several seconds. Confirm acceleration magnitude near 9.81 m/s², small gyro readings and stationary tilt. A stationary yaw drift test requires a longer recording.
3. Rotate each axis slowly, then through known angles. Compare the isolated cube, quaternion and gyro plots. Freeze and export unexpected observations.
4. Mount the device and capture the reference pose for that tracker. Confirm the body segment evidence in the regular Devices view and Studio.
5. Select a camera to inspect landmarks and tags; use Cameras for intrinsics/extrinsics, exposure, and frame timing. Compare with the tracker in Studio. Repeat for each source.
6. Disconnect and reconnect the tracker; stale traces should not appear live. Check clock resets and drop counters. Confirm behavior with simulated input first.

## Advanced bench

- **Solver comparison:** check individual device IDs and run an independent, cold-start solver on that subset. IMU-only, camera-only and hybrid solve the same input snapshot. The normal solver, recording and training are unchanged. Results are shown at 2 Hz and do not accumulate heading correction. Segment rotation disagreement is reported only for non-inferred evidence with timestamps within 40 ms; it is not ground truth.
- **Tracker comparison:** align two fresh orientation histories by mapped session timestamps (40 ms maximum skew). Compare relative orientation change from the first paired sample, not absolute heading. This is a short-window comparison, not a full-rate recording.
- **Guided tests:** six-face acceleration check, manual 90-degree angle magnitude check, stationary net orientation drift, and downloadable advisory report. No firmware calibration coefficients are applied. Continuous stillness and reference angle are not automatically validated.
- **Timing:** last 256 full-rate accepted orientation intervals, sample-clock and pipeline-acceptance interval histograms and cumulative sequence gaps. Acceptance interval includes scheduling/queue effects, not just Wi-Fi jitter.
- **BMAG v1:** optional 32-byte auxiliary UDP packets, same port and MAC allowlist as BIMU; two BMM150 compensated integer channels. They use receive-time session stamps and raw wrapping device time, not synchronized hardware capture times. Values are intentionally labeled BSP units until hardware scaling is verified. Both magnitude plots and a magnitude-difference readout help identify changes; they are not a calibrated interference detector. BMAG is diagnostic-only and is not included in session recordings.
- **Saturation:** flags sampled values near the Mosaico's configured ±16 g / ±2000 deg/s range. Other boards may have different ranges. The decimated view can miss transient clipping.
- **Battery:** short browser-local history of reported charge; not a power measurement.

TODO: hardware acceptance, authenticated bidirectional firmware control (including bias recalibration), continuous long-duration synchronized capture, full magnetic calibration and quantitative optical reference validation. Live source ablation is deliberately a separate preview, not a mutation of training or recording state.
