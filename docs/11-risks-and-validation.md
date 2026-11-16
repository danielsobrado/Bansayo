# 11 — Risks and validation

## 1. Risks

| # | Risk | Consequence | Mitigation | Needs hardware |
|---|---|---|---|---|
| R1 | Real six-axis IMUs have arbitrary and drifting heading | Segment orientations are wrong about the vertical axis; the PoC's assumed heading alignment does not hold | Heading alignment procedure is roadmap item 2; the `assumed` state is visible everywhere it matters; the simulator can inject a heading offset to show the effect | Yes |
| R2 | Reference-pose calibration depends on the user holding the pose accurately | Constant offset in every angle derived from that tracker | Stillness check; immediate visual result; later, functional calibration from movement | Yes |
| R3 | The SolarXR data feed may not expose what the adapter needs, or may change between SlimeVR versions | Real tracker input is delayed | Read the schema and server source before designing the adapter; pin a tested SlimeVR Server version | Yes |
| R4 | SlimeVR firmware support for the exact boards (ESP8266 + LSM6DSR, ESP32-C3 + ICM-45686) is assumed from its documentation | A board may need firmware work first | Flash and test each board with SlimeVR Server before starting the adapter | Yes |
| R5 | OpenXR runtimes differ in whether they expose tracker roles | VR path 1 works on SteamVR only | Path 2 does not depend on it; verify on the actual headset and runtime | Yes |
| R6 | Camera capture on Windows: crate maturity, MJPEG decode cost, USB bandwidth with several cameras | Lower frame rates or dropped cameras | Spike `nokhwa` and `cameras` on the target machine before choosing; the adapter boundary isolates the choice | Yes |
| R7 | ONNX Runtime with CUDA on Windows needs matching driver, CUDA and cuDNN versions; `ort` 2 is still a release candidate | Inference fails to start, or the API changes | Pin exact versions; configured fallback to another execution provider; state `unavailable` with the cause | Yes |
| R8 | Licence terms of pretrained pose weights and their datasets | Cannot distribute a model with the application | Review before distribution; models are user-supplied files until then | No |
| R9 | Cameras without hardware sync | Triangulation error during fast movement | Views beyond `triangulation.max_skew_ms` of the newest frame are left out (default 20 ms, not measured); `time_sync` profile designed; start with slow movements; unsynchronised sets show as fewer views or missing landmarks with reasons | Yes |
| R10 | Default body proportions are population averages | Joint positions and the floor-contact root height are approximate | Marked `inferred` and `assumed`; body-proportion calibration on the roadmap | Yes |
| R11 | The floor-contact rule assumes a foot is on the floor | Wrong during jumps, sitting, lying | Root position is labelled `inferred`; a positional source replaces it | Yes |
| R12 | JSON streaming cost with many clients or landmark streams | Engine CPU, client parse time | Frames are serialised once and shared; measure first; binary encoding is an additive change | No |
| R13 | The desktop shell hosts the engine in-process | An engine crash closes the window | Acceptable for the PoC; sidecar is a packaging change (ADR-006) | No |
| R14 | No authentication on the local API | Anything on the machine can control the engine; exposure if bound to a network address | Loopback by default; origin allow-list; warning on non-loopback bind. Authentication is required before any network use | No |
| R15 | New major versions in the toolchain (TypeScript 7, Vite 8, React 19) | Tooling incompatibilities | Pin exact versions in the lockfile; Agent 2 reports any forced downgrade | No |
| R16 | Development here is on macOS; the target is Windows 11 | Platform-specific build or runtime issues go unseen | Nothing platform-specific is in the PoC; build and run on Windows is a validation task | Yes |
| R17 | Simulated data proves plumbing, not accuracy | Over-reading the demo | Everything simulated is labelled; no accuracy figure is produced | — |
| R18 | Sessions are read fully into memory for playback | Large sessions refused | Size limit with a clear error; indexed reads when camera data arrives | No |
| R19 | Camera intrinsics and extrinsics are declared by the simulator; no calibration solver exists | Real cameras need profiles from a procedure that is not built; small calibration errors displace every triangulated landmark | Profiles carry `method` and `origin`; the reprojection error is published per landmark; a calibration procedure is roadmap item 4 | Yes |
| R20 | Triangulation was exercised only with simulated pixel noise | Real detector noise (jitter, left/right swaps, occlusion guesses) may exceed what the linear solve and the reprojection check absorb | Outlier views are dropped and reported per landmark; thresholds are configuration; no nonlinear refinement or filtering until real data shows the need | Yes |
| R21 | The measured root position is used as-is | Camera noise goes straight into the skeleton root, with no smoothing | Provenance is `measured` and the source is the published landmark; blending with the IMU estimate waits for fusion stage 4 | Yes |

## 2. Decisions that need hardware before they are final

| Decision | What settles it |
|---|---|
| Heading alignment procedure and its second pose | Trials with real trackers on each segment |
| Staleness, disconnect and degraded thresholds | Observed Wi-Fi behaviour of real trackers |
| Camera capture crate | The Windows spike |
| Pose model size and input resolution | Measured inference rate on the RTX 4080 |
| Whether custom firmware is needed at all | Whether SlimeVR firmware data is sufficient for fusion |
| The ESP32 telemetry draft in [04 §7](04-contracts.md) | The firmware milestone |
| VR interpolation delay | Measured latency in the headset |
| How fused poses reach SteamVR | Evaluation of an OpenVR driver against routing through SlimeVR |
| Binary wire encoding | A measured bottleneck |
| Triangulation defaults: skew, reprojection limit, minimum views, minimum ray angle | Measured clock offsets, noise and calibration error of the real camera rig |

## 3. What the PoC will not have verified

Stated here so that the final report cannot drift from it.

- Any physical device: trackers, cameras, headset.
- The OpenXR session path of the VR client.
- Build and run on Windows 11.
- Performance: latency, throughput, memory, behaviour over long runs.
- Accuracy of any joint angle. Angles from the simulator match the simulator's script; that is a
  test of the mathematics, not of measurement.
- Behaviour with more than a handful of clients.

## 4. Deferred validation tasks

| Task | When |
|---|---|
| Windows 11 build and smoke test of engine, desktop app and VR client | Immediately after the PoC |
| Hardware-in-the-loop test with SlimeVR trackers | With the SolarXR adapter |
| Camera capture and inference rate on the target GPU | With the camera milestone |
| Real camera calibration: intrinsics and extrinsics for a physical rig, and reprojection error on it | With the camera milestone and a calibration procedure |
| Triangulation with real detector noise: landmark error against a reference, outlier-view behaviour, effect on the measured root | With the camera milestone |
| Camera synchronisation on real cameras: measured skew between frames, and whether the default 20 ms limit or a `time_sync` profile is needed | With the camera milestone |
| Latency measurement, sensor to screen and sensor to headset | With real devices |
| Long-running stability and memory | Before first release |
| Joint-angle accuracy against a reference (goniometer or optical system) | Before any measurement claim; until then `validated` stays `false` |
| Accessibility audit with assistive technology | Before first release |
| Cross-runtime OpenXR test | With the headset |

## 5. Accepted technical debt in the PoC

| Debt | Why it is acceptable | Paid when |
|---|---|---|
| Heading alignment assumed identity | Correct for the simulator; visibly labelled | Roadmap item 2 |
| Session files read into memory | Sessions are small without video | Camera recording |
| No session database | Directory listing suffices | Session analysis |
| Time offset estimate is one-way | Good enough without two-way capable devices | Custom firmware |
| Solver stages are hard-wired functions | Only one implementation of each exists | Second implementation of any stage |
| Triangulation is a linear solve with no refinement or temporal filtering | Nothing real to tune against; the reprojection check bounds the damage | Real cameras show noise it cannot absorb |
| Debug skeleton instead of an avatar in VR | Proves the stream and coordinates | VR milestone |
| Root `README` is the only user guide | The product is a PoC | First release |
