# Sensor hardware acceptance

This is a physical acceptance protocol, not a completed result. Run it on the actual Windows/ESP Mosaico/camera rig. Keep the commit, build flags, camera profiles, firmware configuration (redact credentials), board revision and test evidence. The automatic gates below are initial engineering targets, not a claim about measured device accuracy.

## 1. Firmware and engine

- Update the engine first; BMAG v2 is not understood by older engines. Back up the factory board firmware. Build/flash the pinned ESP-IDF/BSP firmware using its README, with diagnostics enabled. Use the station MAC shown by firmware, not the display or Bluetooth MAC.
- Copy config/mosaico.example.yaml into a local configuration, set the MAC and enable diagnostics timing. Run a release engine. Select matching firmware and backend nominal rates, first 100 Hz then 200 Hz. Perform stationary AHRS and Bansayo mounting calibration.
- Confirm two BMM150 streams and the three inertial channels. Record the firmware UART log and engine state. Keep magnetic test materials, straps, magnets and metal away initially.

## 2. Automated capture and evaluation

For each rate and rig configuration, repeat at least three times with unique output paths:

```sh
node scripts/bench-sensor-studio.mjs --devices MOSAICO-01 --seconds 60 --label mosaico-200hz-single --out benchmark-200hz.json
node scripts/analyze-diagnostic-recording.mjs data/diagnostics/capture-REPLACE.jsonl capture-report-200hz.json
node scripts/check-sensor-hardware.mjs benchmark-200hz.json capture-report-200hz.json acceptance-200hz.json --rate 200 --mag-rate 10 --min-rate-ratio 0.95 --max-gap-ratio 0.01 --max-fps-regression 0.10 --min-seconds 60
```

Use the actual recording file printed in the benchmark report. For 100 Hz change the rate and output names. With 4 devices use their exact comma-separated IDs. The checker evaluates measured window counts, sequence gaps, monotonic times, clock settled status, journal loss, frame throughput and stage timing evidence; a passing report is not a physical accuracy certificate. It rejects simulated descriptors. Raw capture includes samples outside the common window; the report includes in-window counts.

| Condition | Required evidence |
|---|---|
| One tracker, 100 Hz and 200 Hz | benchmark, journal, analyzer, acceptance, UART |
| Four trackers, 100 Hz and 200 Hz | same, separate result for each tracker |
| Two then four calibrated cameras plus trackers | same, calibrated camera profiles, optical residual screenshots |
| Ten-minute continuous capture | journal, analyzer; confirm cap/truncation status, CPU/GPU memory and temperatures |

## 3. Disruption checks (manual)

- Start recording, interrupt Wi-Fi briefly, reconnect. Inspect mapped timestamps, sequence gaps, drop counters and device state. Confirm no stale pose masquerades as live.
- Start capture, press local recalibration, hold still, then remount. Confirm journal stops on reference change, mounting becomes stale, a new capture has new reference and magnetic diagnostics recover.
- Repeat with an old BMAG v1 sender only on a legacy BIMU v1 stream. With v2, delayed or foreign references must not enter new measurements.
- Start capture on 1–4 devices, add event markers, stop, reopen engine journal. Confirm start/end match the report, excluded drain samples are visible in metadata, and export is reproducible. A crash/partial file must not be reported complete.

## 4. Metrology and physical synchronization (manual)

- Mount tracker rigidly to a surveyed angle fixture. Capture six faces, 90-degree rotations around each axis and stationary drift over 10 minutes. Repeat after mounting, with actual straps/magnets installed. Compare to a reference instrument; retain raw data and uncertainty.
- For magnetometers capture 3D orientations away from ferrous objects, inspect holdout error, repeat in the final enclosure. Never apply the advisory coefficients automatically.
- Capture a calibrated rigid target at multiple distances and directions. Compare estimated distances to independently measured target geometry. Inspect exact solver residuals and rejected camera views. Record all failures, not only successful poses.
- For clock alignment use a common observable physical event: a rigid-mounted tracker visible to all cameras and a controlled motion event, or an external trigger/logic analyzer if the camera and board expose suitable documented interfaces. Compare observed event times and independent reference timestamps. Software clock windows are not hardware trigger synchronization. Do not assume USB/Wi-Fi camera exposure or IMU sampling is simultaneous.
- Record CPU/GPU utilization and frame timings with capture on/off; record actual motion-to-photon latency using a high-speed camera or equivalent external reference. Repeat with VR running.

## 5. Evidence and disposition

Store raw evidence and a summary in a dated local folder; do not commit credentials, medical/body footage or large raw recordings to the public repository. Automated PASS applies only to the explicit engineering thresholds. A physical pass requires reviewed angle/distance, clock, interruption, thermal and VR evidence. If the recording is terminated by cap or reset, repeat the planned uninterrupted run. TODO: execute on physical rig and establish validated accuracy and latency requirements.
