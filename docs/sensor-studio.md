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
- Signals: drag to reorder within the instrument grid, keyboard move buttons and wide docking. Panel order persists. Pinned instruments can float within the Studio viewport; no cross-workspace or native OS-window docking.
- Analysis: mean-removed Hann FFT, one-sided periodogram power spectral density, overlapping Allan deviation from the latest contiguous approximately uniform run, and magnetic ellipsoid fit with positive-definite and coverage checks plus a held-out radius residual. Fits are advisory, no automatic firmware write.
- Optical: orbitable calibrated frustums, observed rays, nearest-ray residual and solver-reported RMS; exact aligned input rays and final residuals, with a clearly labelled 40 ms preview fallback for older frames. Valid matching profiles are required for camera frustums.

## Physical acceptance and performance
1. Flash the BIMU v2 Mosaico firmware and mount-calibrate. Enable magnetic diagnostics. Confirm orientation, gyro and accel rates independently at 100 and 200 Hz.
2. Capture 60 seconds of stationary telemetry, then 10 minutes or until the row limit. Compare captured sequence gaps, journal loss, device drops and expected per-channel counts. Test a Wi-Fi interruption and local recalibration; verify no incompatible reference is merged.
3. Compare tracking FPS, stage p95, CPU, memory, GPU frame time and drop counters with capture off/on and 1/4 trackers. Record hardware, software revision, camera count, transport and configured rates. No performance threshold is assumed until a baseline is measured.
4. Run known-frequency vibration and stationary noise tests; verify FFT resolution and Allan deviation stability against independent reference tools. Rotate each magnetometer through varied orientations; inspect held-out radius errors and compare with a reference magnetometer before applying coefficients.
5. With calibrated cameras, move a ChArUco board or known rigid target through the capture volume; inspect reprojection RMS and rejected views. Compare against surveyed dimensions, not visual agreement alone.
6. Test draggable instruments, export, browser memory, phone layout and GPU cleanup. Keep screenshots and test reports under a dated local acceptance directory.

TODO: physical acceptance, native OS-window and cross-workspace docking, validated metrological calibration and hardware performance baselines.

The independent browser test uses deterministic synthetic telemetry, not real hardware: `cd apps/desktop && npx playwright install chromium && npx playwright test -c playwright.sensor-studio.config.ts`. CI uploads screenshots for desktop dark/light and phone views. Narrow-screen overrides are scoped to Sensor Studio; the other workspaces retain their desktop minimum width. This is a UI acceptance test, not a measurement-accuracy test or pixel-baseline regression suite.

## Persistent recording and pinned instruments
Capture → Persistent engine recording is separate from browser capture. One engine-owned job, up to four non-camera devices, 1–3600 seconds (UI presets 1–60 min), 256 MiB per file and 32 directory entries maximum. Files are under `engine.data_dir/diagnostics`. The worker renews journal leases, writes incremental JSONL batches off the tracking thread, flushes periodically, synchronizes on clean completion and renames the partial file. A crash or write error can leave a recoverable partial file; never interpret a missing footer as a complete capture. Stop or engine shutdown closes the file; a reference reset terminates capture. Archive files manually to free capacity. The API reports the latest job in memory; previous files remain on disk. No automatic file upload or deletion.

The pinned instrument shelf supports up to three independent device/channel or room views, remains visible across Studio tabs and persists panel choices/order. Drag or use the reorder buttons; unpin to release resources. On narrow screens it stacks below the active workspace. Each pinned instrument can float inside the application, move and resize with pointer handles or arrow keys, and dock back to the shelf. Geometry persists, clamps to the viewport, and falls back to inline panels on narrow screens. The active window is raised; floating instruments remain visible when the shelf is collapsed. Native OS windows and cross-workspace docking are not supported.

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

Capture and import reject duplicate, missing or out-of-order journal cursors unless the missing range is explicitly reported by the server as lost. The browser rejects reference/lease changes rather than merging incompatible samples. Captures and imports with reported losses remain useful for diagnosis but are not evidence of uninterrupted hardware sampling.

Inspection snapshots expose an opaque reference generation. Guided bench captures and tracker comparison baselines are invalidated on reference changes; a prior orientation must never be compared to a new reference. Triangulation only interpolates frames with matching landmark schema, count and calibrated resolution.

BMAG sequence filtering now follows every accepted sample rather than the decimated chart, extends wrapping counters, and splits diagnostic journals on a detected auxiliary reboot. Legacy BMAG v1 lacks a firmware reference identifier; prefer BMAG v2. Legacy optical preview rays require the reconstruction's calibration profile IDs and compatible schema/resolution.

The CLI recovery analyzer now applies the same structural checks as the browser (device membership, channel shape, finite samples, contiguous cursor or explicit loss, reference and footer counts). A cleanly closed but truncated-at-limit or reset-terminated capture is not proof of continuous sampling.

Browser capture explicitly finalizes with one last incremental poll when the operator stops. A full final batch reports possible remaining backlog; duration is still a browser request boundary, not a hardware-synchronized trigger. Capture continues across Studio tabs, but hidden timelines and spectral plots do not render; the persistent recorder status poll runs only while Capture is visible. Ordered timeline channels use binary range lookup; out-of-order imports retain the safe scanning path.

BMAG v2 (40 bytes) appends the same little-endian nonzero u64 reference as BIMU v2 at offset 32. Firmware sends magnetic samples only after AHRS initialization. The backend accepts v2 magnetic packets only for the active BIMU reference, maps their wrapping sample clock relative to the most recent IMU sample, and orders them through the same actor queue as reference changes. Legacy v1 is accepted only with an established v1 BIMU stream and retains arrival timestamps; upgrade the engine before flashing firmware. This is clock-mapped sampling, not a hardware synchronization guarantee.

Triangulated landmarks optionally carry exact aligned per-view detections, final projections, residuals and the original world-space rays. Rejected views are reprojected against the final accepted point, not the intermediate outlier fit. The optical lab prefers this evidence; older frames retain the explicitly labelled preview fallback. This additive wire field increases landmark message size and should be included in performance acceptance.

Persistent recordings activate all selected device cursors before establishing a common engine-clock start. Stop captures an engine timestamp, duration uses that clock, and the recorder drains for up to 250 ms after the boundary. Raw journal rows and cursor integrity remain intact; the browser import selects the half-open interval [window_start_us, window_end_us), and the CLI reports raw channel statistics plus in-window counts. Clock-estimation uncertainty, late observations beyond the drain and physical triggering remain hardware-dependent. Both recorders use mapped engine time windows; persistent recording avoids browser polling latency when establishing stop.

The native UDP integration now sends valid and foreign-reference BMAG v2 samples to verify filtering through the engine actor. The real-time image-folder test suite runs its three camera fixtures serially to reduce competing decode and scheduler load without reducing measurement acceptance requirements.

Quick browser capture also reads the live engine's monotonic clock after arming all journals, and clips its exported samples to a common half-open mapped-time window. It detects engine epoch changes and drains late arrivals before finalizing. Operator stop is timestamped by the next successful engine clock response, not a physical trigger; use persistent recording for lower-jitter engine-owned start/stop. Older engines without the clock endpoint must be upgraded.

Physical acceptance procedure and explicit gate evaluator: [sensor-hardware-acceptance.md](sensor-hardware-acceptance.md). The new CLI is covered by synthetic positive/negative evidence tests; it does not declare simulated results a physical pass.

## Multi-device timing and signal health

Capture or import a completed multi-device recording. Mount trackers rigidly on the same object; use distinctive, non-periodic movement. Choose gyroscope or accelerometer, magnitude (orientation-independent) or a common aligned axis, and a reference tracker. Each other recorded tracker is compared independently. Health metrics include observed rate, median/p95 mapped sample intervals, long and non-increasing intervals, and signal mean, population standard deviation and RMS. Motion statistics are not stationary sensor noise.

The timing analysis uses the latest shared interval up to eight seconds, a grid no faster than 200 Hz or the slower median rate, fixed overlap length across ±200 ms lags, linear interpolation and Pearson correlation. Positive lag means the test device's feature occurs later than the reference on the mapped engine clock. Gaps, non-increasing times, insufficient variation, journal loss and reference resets prevent reporting a lag. Weak, competing or boundary peaks are explicitly inconclusive. The unshifted common-time overlay and full correlation curve are included in the JSON report. Thresholds are initial heuristic screening criteria; no anti-alias filter, automatic time correction or metrological claim. Repeat against independently timestamped physical events and vary movement to assess reliability.

## Stationary acceptance (P0)

Warm up the IMU, fix it to a stationary non-vibrating surface and capture at least 60 seconds. In Capture, confirm the entire recording was stationary. A report is evaluated independently per device using gyroscope, accelerometer and orientation channels. The common observed duration, measured channel rates, gyro bias/peak, gravity magnitude error, worst axis standard deviations and maximum orientation excursion are checked against editable positive limits. Mean, population standard deviation, RMS, fitted linear trend per minute and final orientation difference are reported. Limits persist locally and are embedded in the exported JSON. The default values are preliminary engineering screens (90 Hz minimum channel rate, 30 s common duration), not hardware specifications; adjust for the configured sensor and test goal. Missing or irregular samples, losses, resets, an incomplete recording, no common engine window, insufficient duration, unconfirmed stationary conditions or excessive motion yield INCONCLUSIVE rather than a pass. Slow orientation drift and motion can be indistinguishable from actual fixture movement; use an independent reference. No automatic firmware calibration or physical certification is performed.

## Camera-to-IMU timing (P0)

Select one camera and two stable landmark indices in Capture before starting browser full-rate capture. Use a fixed camera, a rigid target with two detectable, consistently identified landmarks, and an IMU fixed to that target. Rotate slowly in the image plane (camera optical axis), using distinctive non-periodic motion. Keep both landmarks visible, separated by at least 25 pixels, and confidence at least 0.7. The selected camera's received 2D landmark messages are captured without a polling loop, reduced to timestamped projected angles (no images stored), and clipped to the IMU's mapped engine time window. Angular speed uses wrapped angle differences at the midpoint of successive frame timestamps. Compare its magnitude against the selected gyro magnitude. A positive lag means the camera's feature occurs later in mapped engine time. Receive-time camera timestamps, schema/resolution/detector/origin changes, time/sequence resets, low confidence, large rotation steps, excessive frame gaps or invalid capture integrity prevent a reliable result. Existing weak/ambiguous/boundary correlation screening applies. Export retains the selected camera, landmarks, origin, timestamps, correlation curve and advisory result. This is not an exposure-time measurement or a hardware clock offset; projected angle can be distorted by perspective, rolling shutter, detection noise and frame omissions. The browser may miss intermediate frames and engine-only journals do not store optical observations. Physical timing acceptance requires independently timestamped events; no automatic clock correction is applied.

## Reliability and calibration quality (P1)

Reliability records a bounded browser-local history of device status while Sensor Studio is open and receiving live state. It samples at most once per five seconds, stores at most 3,000 rows across devices for up to seven days, and persists locally. Collection run, device identity, origin, state, last seen timestamp, engine observation counters and reported clock status are retained. Export all rows as JSON or clear with confirmation. Tracking fraction is the fraction of observed states, not uptime. Counter deltas and chart lines split at run changes, long collection gaps, reset changes or decreasing counters/timestamps. No background/engine-owned persistence is implied.

The engine's existing continuous minimum-delay clock estimator is authoritative: drift needs at least 20 s of device-clock evidence; positive ppm means the device runs slow. Timing shows the current estimate, while Reliability charts drift, offset, uncertainty, rate and engine drop deltas. The estimator cannot separate oscillator drift from changing minimum transport delay. Unsupported or unsettled clocks show no estimate.

Native BIMU UDP inspection now exposes socket-level transport counters and the latest 256 consecutive packet interval pairs. Timing displays inferred missing sequence slots, accepted datagrams, rejected ordering/reference datagrams, absolute receive-minus-device interval residual and its p95, and a bounded counter-delta gap-ratio chart. It excludes BMAG and malformed/unconfigured datagrams. It resets at reference changes, publishes roughly once per second while accepted datagrams arrive, and cannot distinguish sender-side omissions from network loss. Socket timestamps include scheduling and buffering; this is not hardware NIC timing. Engine observation counters remain distinct. Other transports retain their existing pipeline/camera diagnostics.

Guided hardware tests calculate an equal-weight advisory score only once all ten checks have evidence: six accelerometer faces, three 90° rotation axes and a drift endpoint test of at least 30 seconds. Existing face and rotation tolerances apply; the endpoint limit is 2°. Incomplete evidence produces no numeric score. Export includes individual gates. Device calibration profile readiness is separately the fraction of reported profiles marked valid, not a measurement-accuracy score. Guided tests are decimated and operator-driven; stationary endpoints do not establish continuous stillness. Physical references and independent validation remain necessary.

## Engine performance and reliability archive (P2)

Opt in using schema version 2:

```yaml
schema_version: 2
diagnostics:
  timing: true
  resources: true
  reliability: true
```

Restart the engine. Performance shows engine-process CPU (100% = one logical core), RSS/working set and the existing 10-second pipeline stage percentiles and budgets. Resources are sampled every two seconds outside the tracking pipeline and at most 300 points are retained. Linux 64-bit and Windows process CPU counters are supported; unavailable counters are null. GPU utilization/VRAM are not measured. Export the current sample window as JSON.

Reliability additionally exposes an opt-in engine-owned SQLite archive in the engine data directory at diagnostics/reliability.sqlite. It stores device status, stream counters, clock status, battery and signal, without athlete or motion data. It samples live state every five seconds independently of the browser, skips replay, and retains at most 20,000 snapshots or seven days (whichever limit is reached first). SQLite may retain freed pages for reuse. The API returns 120 snapshots per page; browse older pages and export individual pages. Disabled collection does not delete previous data. This is sampled evidence, not continuous uptime.

## Diagnostic integrity review

Timing estimates and stationary acceptance require a successfully completed common-window recording with no journal losses or reference resets. Stationary metrics use the common observed interval of the three required channels; sample health still checks the full captured channels. Calibration rotation recapture invalidates the previous result for that axis until a fresh endpoint is captured; a drift endpoint requires the configured minimum duration.

The engine performance endpoint exposes stage summaries even when resource sampling is disabled. Stage samples are bounded and copied under the timing mutex; sorting is outside the lock. The reliability archive uses read-only queries and an insertion-ID cursor (before_id) rather than wall-clock timestamps, so equal UTC timestamps do not drop rows during pagination. Exported archive rows include the cursor ID. The browser clears stale page results when the query changes or goes offline. UDP jitter p95 uses the nearest-rank percentile.

The real-time training integration tests serialize their simulator rigs to avoid artificial contention on shared CI runners. No production recognition threshold or test assertion has been relaxed; replay count failures still require separate investigation if reproduced.

## Core engine review follow-up

Source supervision cancellation interrupts queued failure reporting. WebSocket subscriptions continue when optional stage diagnostics are disabled, replay the most recent available diagnostic packet on subscription, and reject rates outside 1–1000 Hz before converting them into timer periods. State-only subscriptions do not create frame ticker threads. Desktop connection attempts have a ten-second hello deadline and ignore repeated valid hello messages.

Recording writers exclusively create the MCAP file and require explicit finalisation before publishing completed metadata. Playback rejects zero-size/unfinalised metadata, decreasing log timestamps, registry-rejected events and overflowing duration. Existing compressed golden recordings retain their original metadata size, so exact file-size equality is not enforced by the full loader. Replay rejects invalid seek positions or nonpositive/nonfinite speed, resets planar history on seek and uses the rounded solver tick boundary.
