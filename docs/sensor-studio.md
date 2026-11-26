# Sensor Studio
Devices → Sensor Studio replaces the long diagnostic card dashboard with task-oriented instrumentation views, a searchable component explorer, persistent inspector/explorer visibility, compact health readouts and a common visual language based on the existing themes.

## Views
- Overview: selected device health, live reference orientation and the main inertial signals.
- Signals: capability-aware component selection, individual axis visibility, synchronized time base and shared cursor, 1/2/5-second windows, history offset, auto or fixed symmetric ranges, maximized chart statistics and CSV export. Freeze the input before measuring a fixed instant. Missing cursor samples stay unavailable.
- Calibration: existing mounting and advisory bench tools, a six-face visual stepper with progress and per-face results, separate rotation and drift tests.
- Timing: packet interval histograms, magnetic magnitude and XY/XZ/YZ scatter plots, range warnings and battery history.
- Compare: existing independent solver and two-tracker comparison, including bounded capture reports, shared orthographic projections, root-aligned overlay and selectable segment disagreement bars.
- Optical: selected camera preview and marker overlays. Camera calibration remains in Cameras.

The 3D instrument uses existing Three.js and OrbitControls, with a generic board mesh, sensor axes, fixed reference-up arrow and normalized measured specific-force direction, paired within 40 ms of orientation. Magnetometer components remain in their reported frames until sensor-axis alignment is validated. It renders only on data, resize, theme or camera changes. A wireframe remains available if WebGL cannot initialize. Vectors are directions, not magnitude or gravity-subtracted linear acceleration. A reference assignment map is not the fused body pose.

The current backend is intentionally unchanged: up to 256 decimated points per channel, nominal 50 Hz maximum, polled about four times per second. The shared cursor uses real session timestamps and does not claim full-rate recording or interpolation. SVG is retained for this bounded workload; measure before adding a chart dependency.

## Acceptance
Open Devices → Sensor Studio using a simulated or physical rig. Verify filtering, group expansion, channel selection, freeze/resume, cursor alignment, independent axes, fixed ranges, maximize/restore, CSV/JSON export, inspector toggles, camera selection and comparison. Check Graphite, Onyx and Silver at 1440×900, 1024×768 and a phone-sized viewport. Inspect GPU/context cleanup after repeated device and workspace changes.

TODO: full-rate incremental acquisition/recording, longer timeline with event markers, saved multi-panel layouts and resize handles, calibrated magnetic ellipsoid, FFT/Allan deviation, optical residual instruments, orbitable 3D comparison camera and hardware acceptance. Do not represent these as available until implemented and validated.

The independent browser test uses deterministic synthetic telemetry, not real hardware: `cd apps/desktop && npx playwright install chromium && npx playwright test -c playwright.sensor-studio.config.ts`. CI uploads screenshots for desktop dark/light and phone views. Narrow-screen overrides are scoped to Sensor Studio; the other workspaces retain their desktop minimum width. This is a UI acceptance test, not a measurement-accuracy test or pixel-baseline regression suite.
