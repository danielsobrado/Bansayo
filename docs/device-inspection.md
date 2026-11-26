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

Limitations: BIMU v1 has no raw magnetometer channel, so both BMM150 plots remain unavailable until a versioned telemetry extension. View isolation does not mute or change solver inputs. True source ablation, hardware calibration commands, longer synchronized multi-device bench captures and quantitative optical-versus-IMU residual plots are follow-up work. Firmware, backend and browser hardware acceptance remain required.
