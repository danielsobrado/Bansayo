# 01 — Vision and scope

## Product vision

One tracking engine that turns whatever motion inputs a user happens to own into a single,
honest, canonical description of their body, and a set of clients that consume it.

A user with six SlimeVR trackers, a user with one webcam, and a user with twelve custom ESP32
trackers plus two calibrated cameras all run the same engine and the same applications. Features
appear and disappear according to what the available inputs can actually support.

## Goals

1. **Input independence.** Camera count and tracker count are independent, configurable, and may
   both be zero. Hardware identity is separate from body placement.
2. **One body model.** A single canonical skeleton and pose stream is shared by the desktop
   application, the VR application, recordings and exports.
3. **Honest measurement.** Origin, provenance, coordinate frame, units, timestamps and calibration
   dependency travel with every value.
4. **Local-first.** Everything runs on one machine with no cloud service, broker or account.
   Python is not needed at runtime.
5. **Evolvable.** Moving from a webcam demo to hybrid camera + IMU fusion adds adapters and
   solver stages. It does not change the contracts or the applications.

## Use cases the architecture must carry

PCVR full-body tracking, stretching and flexibility measurement, martial-arts technique analysis,
joint-angle and range-of-motion measurement, balance and posture, recording and comparison,
exercise detection and repetition counting, real-time feedback, camera correction of IMU drift.

Only a small part of this is implemented first. The table in the next section is the boundary.

## Non-goals

- Clinical or medical measurement claims. Joint angles are unvalidated until a validation study
  exists; the contract carries a `validated` flag that is `false` everywhere in the PoC.
- Cloud sync, accounts, multi-user sessions.
- Distributed deployment. The engine is a modular monolith in one process.
- Hand and finger tracking, face tracking.

## What the PoC proves

| Claim | How the PoC demonstrates it |
|---|---|
| The engine independently generates and streams canonical pose data | `bansayo-engine` runs standalone from a YAML file and serves WebSocket + HTTP |
| A frontend visualises it in real time | The desktop UI renders the skeleton with three.js from the live stream |
| Camera and sensor inputs are represented independently | Simulated trackers and a simulated camera are separate devices with separate capabilities, rates and streams |
| Variable input configurations work | Three example configs: twelve trackers with two cameras, six trackers, camera only. Same binary, same UI |
| Desktop and VR consume the same data | The Godot client connects to the same WebSocket endpoint and parses the same messages |
| Recording and playback reproduce a session | Recorded observations replay through the same pipeline; two replays yield the same frame sequence |
| New hardware does not rewrite the applications | Devices are described by capability; the UI renders from descriptors, not from device types |

## PoC scope boundary

| Area | PoC | Designed, not implemented |
|---|---|---|
| Sources | Simulated body driving N virtual IMU trackers and M virtual cameras; replay of recorded sessions | SlimeVR (SolarXR) adapter, ESP32 UDP adapter, real camera capture, distance/pressure/UWB |
| Camera processing | Simulated 2D landmarks, clearly labelled; simulator-declared camera models; multi-camera triangulation of the simulated landmarks | ONNX pose inference, camera calibration (solver), time synchronisation of real cameras |
| IMU processing | Mounting calibration by reference pose, segment orientation, staleness handling | Heading alignment procedure, bias calibration, drift correction |
| Skeleton | 19-segment body model, forward kinematics, floor-contact root height, or a measured root from a triangulated pelvis (simulated cameras) | Measured body proportions, positional root from XR |
| Measurements | Knee, hip, elbow flexion, shoulder elevation, trunk lean | Exercise detection, repetition counting, movement phases |
| Quality | Per-segment provenance and confidence, device state machine, feature availability with reasons | Validated accuracy figures |
| Recording | Record inputs to MCAP, list sessions, play, pause, seek, speed | Frame stepping, session comparison, export formats |
| Desktop | Shell, Overview, Studio, Devices, Cameras, Sessions, reference-pose calibration flow | Training analysis workspace, full calibration workspace, video overlay |
| VR | Godot project with training room, debug skeleton, stream consumer, desktop dev mode | Avatar retargeting, virtual mirror, headset alignment calibration, SlimeVR tracker roles |
| PCVR output | — | SlimeVR/SteamVR tracker output |

### Why the camera path is simulated in the PoC

The brief prefers a genuine webcam with a real pose model if it can be completed reliably. It
cannot be verified in this iteration: the target is Windows 11 with CUDA, capture needs an
interactive camera permission, and no agent can observe a real camera image. An unverified capture
and inference path would be exactly the kind of claim this project forbids. The PoC therefore
ships a simulated camera that projects the simulated body through a declared pinhole camera, with
`origin: "simulated"` on every landmark. The real path is specified (decision ADR-009) and is the
first item on the roadmap.

No prerecorded real-landmark fixture exists, and none is invented.

## Roadmap after the PoC

Priorities follow the brief, with one change: heading alignment moves ahead of real IMU input
because real six-axis trackers are unusable without it.

1. **Real camera capture and pose inference** — capture adapter, ONNX Runtime with CUDA, RTMPose,
   preview stream, 2D landmark overlay on video.
2. **IMU heading alignment and bias calibration** — the two-pose procedure in
   [05](05-calibration-and-fusion.md).
3. **Physical SlimeVR tracker input** — SolarXR client against a running SlimeVR Server.
4. **Calibration workflows** — full calibration workspace, camera intrinsics and extrinsics,
   floor plane.
5. **Triangulation on real cameras** — the geometry exists and runs on simulated cameras; what
   remains is real calibration, time synchronisation and validation.
6. **Camera + IMU fusion** — heading and drift correction from cameras, and estimation beyond the
   measured root position.
7. **Exercise metrics and session analysis** — training workspace, repetition detection, session
   comparison, SQLite session index.
8. **Expanded VR** — avatar retargeting, mirror, headset alignment, guidance.
9. **Custom ESP32 firmware and extra sensors** — UDP telemetry, time sync, ToF, pressure, UWB.
10. **SteamVR tracker output** from the engine's fused pose.
