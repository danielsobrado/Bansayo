# Sensor Studio
Devices → Sensor Studio replaces the long diagnostic card dashboard with task-oriented instrumentation views, a searchable component explorer, persistent inspector/explorer visibility, compact health readouts and a common visual language based on the existing themes.

## Views
- Overview: selected device health, live reference orientation and the main inertial signals.
- Signals: capability-aware component selection, individual axis visibility, synchronized time base and shared cursor, 1/2/5-second windows, auto or fixed symmetric ranges, maximized chart statistics and CSV export. Freeze the input before measuring a fixed instant. Missing cursor samples stay unavailable.
- Calibration: existing mounting and advisory bench tools, grouped in one workspace.
- Timing: packet interval histograms, magnetic magnitude, range warnings and battery history.
- Compare: existing independent solver and two-tracker comparison, including bounded capture reports.
- Optical: selected camera preview and marker overlays. Camera calibration remains in Cameras.

The 3D instrument uses existing Three.js and OrbitControls, with a generic board mesh, sensor axes, fixed reference-up arrow and normalized measured specific-force and magnetic directions. It renders only on data, resize, theme or camera changes. A wireframe remains available if WebGL cannot initialize. Vectors are directions, not magnitude or gravity-subtracted linear acceleration. A reference assignment map is not the fused body pose.

The current backend is intentionally unchanged: up to 256 decimated points per channel, nominal 50 Hz maximum, polled about four times per second. The shared cursor uses real session timestamps and does not claim full-rate recording or interpolation. SVG is retained for this bounded workload; measure before adding a chart dependency.

## Acceptance
Open Devices → Sensor Studio using a simulated or physical rig. Verify filtering, group expansion, channel selection, freeze/resume, cursor alignment, independent axes, fixed ranges, maximize/restore, CSV/JSON export, inspector toggles, camera selection and comparison. Check Graphite, Onyx and Silver at 1440×900, 1024×768 and a phone-sized viewport. Inspect GPU/context cleanup after repeated device and workspace changes.

TODO: full-rate incremental acquisition/recording, longer timeline with event markers, saved multi-panel layouts and resize handles, calibrated magnetic ellipsoid, FFT/Allan deviation, optical residual instruments, shared 3D comparison camera and hardware acceptance. Do not represent these as available until implemented and validated.
