# Sensor Studio
Devices → Sensor Studio replaces the long diagnostic card dashboard with task-oriented instrumentation views, a searchable component explorer, persistent inspector/explorer visibility and widths, Bench/Wide/Focus presets, compact health readouts and a common visual language based on the existing themes.

## Views
- Overview: selected device health, live reference orientation and the main inertial signals.
- Signals: capability-aware component selection, individual axis visibility, synchronized time base and shared cursor, 1/2/5-second windows, history offset, auto or fixed symmetric ranges, maximized chart statistics and CSV export. Freeze the input before measuring a fixed instant. Missing cursor samples stay unavailable.
- Calibration: existing mounting and advisory bench tools, a six-face visual stepper with progress and per-face results, separate rotation and drift tests.
- Timing: packet interval histograms, magnetic magnitude and XY/XZ/YZ scatter plots, range warnings and battery history.
- Compare: existing independent solver and two-tracker comparison, including bounded capture reports, shared orthographic projections and orbitable single-scene 3D comparison, root-aligned overlay and selectable segment disagreement bars.
- Optical: selected camera preview and marker overlays. Camera calibration remains in Cameras.
- Events: bounded browser-observed device state, assignment, calibration and clock status changes; filter and export. This is not the engine log or a synchronized sensor event stream.

The 3D instrument uses existing Three.js and OrbitControls, with a generic board mesh, sensor axes, fixed reference-up arrow and normalized measured specific-force direction, paired within 40 ms of orientation. Magnetometer components remain in their reported frames until sensor-axis alignment is validated. It renders only on data, resize, theme or camera changes. A wireframe remains available if WebGL cannot initialize. Vectors are directions, not magnitude or gravity-subtracted linear acceleration. A reference assignment map is not the fused body pose.

Normal inspection remains decimated (256 points/channel, nominal 50 Hz maximum). Capture requests activate an independent per-device journal for ten seconds, renewed by polling. The journal retains 16,384 supported observations per device and returns at most 4,096 per incremental request. Cursors expose overruns and epoch changes; link transitions and replay invalidate the journal. Capture records accepted data, not rejected packets or raw FIFO reads. Magnetic auxiliary observations are included; images, camera landmarks, battery and radio telemetry are not in this journal. Use existing session recording for general motion evidence. Browser capture is memory-only until exported; navigating out of Sensor Studio or closing the page discards it. No unbounded server-side file recording is introduced.

## Acceptance
Open Devices → Sensor Studio using a simulated or physical rig. Verify filtering, group expansion, channel selection, freeze/resume, cursor alignment, independent axes, fixed ranges, maximize/restore, CSV/JSON export, inspector toggles, camera selection and comparison. Check Graphite, Onyx and Silver at 1440×900, 1024×768 and a phone-sized viewport. Inspect GPU/context cleanup after repeated device and workspace changes.

## Advanced bench
- Capture: up to four non-camera devices, 10/60/300/600 s requested duration, 500,000 total observation limit. Stop on errors or epoch resets; report journal losses. Export JSON before closing.
- Timeline: session-relative min/max envelope, per-device/channel selection, time-window navigation and operator event markers at the latest received session timestamp (not a hardware trigger).
- Signals: drag to reorder within the instrument grid, keyboard move buttons and wide docking. Panel order persists. No arbitrary floating windows or cross-workspace docking.
- Analysis: mean-removed Hann FFT, one-sided periodogram power spectral density, overlapping Allan deviation from the latest contiguous approximately uniform run, and magnetic ellipsoid fit with positive-definite and coverage checks plus a held-out radius residual. Fits are advisory, no automatic firmware write.
- Optical: orbitable calibrated frustums, observed rays, nearest-ray residual and solver-reported RMS; fresh 2D preview within 40 ms is not necessarily the solver's exact observation. Valid profiles and matching image size required.

## Physical acceptance and performance
1. Flash the BIMU v2 Mosaico firmware and mount-calibrate. Enable magnetic diagnostics. Confirm orientation, gyro and accel rates independently at 100 and 200 Hz.
2. Capture 60 seconds of stationary telemetry, then 10 minutes or until the row limit. Compare captured sequence gaps, journal loss, device drops and expected per-channel counts. Test a Wi-Fi interruption and local recalibration; verify no incompatible reference is merged.
3. Compare tracking FPS, stage p95, CPU, memory, GPU frame time and drop counters with capture off/on and 1/4 trackers. Record hardware, software revision, camera count, transport and configured rates. No performance threshold is assumed until a baseline is measured.
4. Run known-frequency vibration and stationary noise tests; verify FFT resolution and Allan deviation stability against independent reference tools. Rotate each magnetometer through varied orientations; inspect held-out radius errors and compare with a reference magnetometer before applying coefficients.
5. With calibrated cameras, move a ChArUco board or known rigid target through the capture volume; inspect reprojection RMS and rejected views. Compare against surveyed dimensions, not visual agreement alone.
6. Test draggable instruments, export, browser memory, phone layout and GPU cleanup. Keep screenshots and test reports under a dated local acceptance directory.

TODO: physical acceptance, unrestricted floating-window docking, exact solver-input residual trace, validated metrological calibration and hardware performance baselines.

The independent browser test uses deterministic synthetic telemetry, not real hardware: `cd apps/desktop && npx playwright install chromium && npx playwright test -c playwright.sensor-studio.config.ts`. CI uploads screenshots for desktop dark/light and phone views. Narrow-screen overrides are scoped to Sensor Studio; the other workspaces retain their desktop minimum width. This is a UI acceptance test, not a measurement-accuracy test or pixel-baseline regression suite.

## Persistent recording and pinned instruments
Capture → Persistent engine recording is separate from browser capture. One engine-owned job, up to four non-camera devices, 1–3600 seconds (UI presets 1–60 min), 256 MiB per file and 32 directory entries maximum. Files are under `engine.data_dir/diagnostics`. The worker renews journal leases, writes incremental JSONL batches off the tracking thread, flushes periodically, synchronizes on clean completion and renames the partial file. A crash or write error can leave a recoverable partial file; never interpret a missing footer as a complete capture. Stop or engine shutdown closes the file; a reference reset terminates capture. Archive files manually to free capacity. The API reports the latest job in memory; previous files remain on disk. No automatic file upload or deletion.

The pinned instrument shelf supports up to three independent device/channel or room views, remains visible across Studio tabs and persists panel choices/order. Drag or use the reorder buttons; unpin to release resources. On narrow screens it stacks below the active workspace. This is constrained docking, not a general floating-window manager.

## Repeatable performance baseline
Enable `diagnostics: { timing: true }` in the engine YAML. Start the engine with the actual hardware configuration (or use `config/sim-six-trackers.yaml` for a clearly labelled simulation). With no active capture, run:

```sh
node scripts/bench-sensor-studio.mjs --devices MOSAICO-01 --seconds 30 --label mosaico-200hz --out mosaico-200hz.json
node scripts/analyze-diagnostic-recording.mjs data/diagnostics/capture-REPLACE.jsonl mosaico-capture-report.json
```

The benchmark compares a warm baseline against engine recording, collects received frame FPS/intervals, stage timing windows, device drops and clock status, then finalizes its recording. Do not change the rig or run another recorder concurrently. For simulated testing use the configured SIM-T01 etc. IDs and a simulation label. The analyzer reads JSONL incrementally and checks the footer, counts and per-channel rates. It does not infer physical accuracy, CPU/GPU utilization or one-way latency. For those use external process/GPU sampling and a synchronized physical reference. Repeat each condition at least three times (one and four trackers, cameras on/off), retaining each report, revision and configuration.

CI runs `node scripts/bench-sensor-studio-sim.mjs tmp/studio-sim-benchmark` with four simulated trackers, an isolated data directory and debug engine. The artifact contains the raw JSONL, benchmark and channel-integrity report. It is a reproducible smoke benchmark, not a performance target or hardware result.

Completed persistent JSONL journals can be opened in Capture for timeline, FFT, Allan and magnetic analysis, up to the existing 500,000-observation browser limit. The importer validates the header/footer counts and cursor/reference continuity. Large and interrupted files are handled by the streaming CLI analyzer instead; no silent truncation or incomplete-file success claim.

The recording controls suspend status polling during commands to prevent a stale GET response from overwriting a start/stop response. Stopping is asynchronous: the final filename is published only after the footer, sync and rename complete. The browser test intercepts both the recording resource and its stop subresource.
