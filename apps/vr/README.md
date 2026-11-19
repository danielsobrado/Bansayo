# Bansayo training room

Godot 4 (standard build), GDScript. The room consumes the same protocol-v1 WebSocket stream as
Studio. No tracking rules or coordinate conversion live here.

Start the engine from the repository root, then run:

```sh
cargo run -p bansayo-engine -- --config config/sim-full-body.yaml
godot --path apps/vr --xr-mode off -- --desktop
```

With the simulator, capture a reference pose in Studio before selecting a moving motion. Drag the
left mouse button to orbit and use the wheel to zoom. `--xr-mode off` stops Godot from starting OpenXR at boot (otherwise
it shows an alert when no headset is connected). Without `--desktop`, the client attempts
OpenXR and falls back to the desktop rig. The engine's head bone never drives the user's camera.
The supplied skeleton is a separate figure; XR world alignment is labelled assumed identity.

Set `BANSAYO_ENGINE_URL=http://127.0.0.1:7700` to override the `bansayo/engine_url` project setting.
Reconnect uses capped backoff. A 50 ms presentation buffer interpolates positions and Hamilton
xyzw rotations, preserving provenance transitions. This is a buffer setting, not measured latency.
The buffer is bounded to 16 frames. No pose extrapolation occurs; data clears after a 500 ms gap.

Validation commands, when Godot is installed:

```sh
godot --headless --path apps/vr --editor --quit
godot --headless --path apps/vr --xr-mode off --script tests/contract_smoke.gd
```

## Training view

The panel beside the figure shows tracking quality, each camera's state and rate, whether
triangulation and the skeleton are available (and why not), live knee/hip/trunk angles with
session peaks, squat and per-leg kick counts, maximum ankle height and minimum pelvis height.
Counts are hysteresis crossings of the engine's angles (squat: mean knee flexion above 70° then
below 25°; kick: hip flexion above 60° then below 20° while the other hip stays below 30°); no
angle is computed in the client.

| Action | Headset | Desktop |
|---|---|---|
| Start/stop recording | A or X | R |
| Replay newest session / back to live | B or Y | P |
| Open Taekwondo clinic | left thumbstick click | T |
| Next / select in clinic | A or X / B or Y | Tab / Enter |
| Reset counts and peaks | — | C |
| Align the room | right thumbstick click | — |

**Room alignment.** With calibrated cameras, the engine's world is the calibration board's frame
(origin at its first inner corner, +X along its first row, +Y up). Lay the right controller on
that corner pointing along +X and click the thumbstick: the figure is then placed where the
cameras see you, at the headset's floor height. The alignment is saved in
`user://xr_alignment.cfg`; without it the figure stands at the room origin in front of you.

### Taekwondo clinic

Open the clinic, pick an athlete, kick and side, and use **Sets** and **Rest** to configure the
rounds. Each set has four repetitions. Review the safety notes and confirm the clear area
before starting. The headset needs a reported boundary wide enough for the selected kick.
[Godot XR play-area documentation](https://docs.godotengine.org/en/4.5/classes/class_xrinterface.html#class-xrinterface-method-get-play-area)
The panel follows the engine's counts, rest clock and coaching. After finishing, choose
**Review sets & replay ghost** or **Practise again**. The keyboard controls run the same flow
in desktop preview.

With motion-data consent, the clinic saves motion on the engine PC. **Load previous practice
ghost** uses a saved attempt for the same kick and side. The ghost plays at half speed on a
separate display. Its foot path and peak marker require a graded height in metres. Use it
as a visual guide while practising in place. Desktop and Android show the same stored set
comparison. Actual headset bounds, input and rendering still need a headset trial.

#### Floating kick targets

Align the headset to the calibration board with the right stick, then turn **Targets** on in
the clinic menu. Prepare the clinic and hold the right controller at a comfortable target
point. Select **Place target & start** with the other hand. A clear floor, calibrated tracking
and space around the target are required. **Start without target** is available in the same menu.

An aqua ring appears at the selected point during active sets. When the cameras observe the
kicking ankle inside the ring during extension, the ring gives a brief amber spark burst and
a directional chime. The clinic counts reaches and stores the result with each completed rep.
The ring clears during rest, pauses and tracking loss.

#### Ghost Walls

Choose **Ghost walls** in the clinic menu while the engine is between sessions. Confirm the
clear space and keep your feet in place. The drill runs two rounds of five side or overhead
panels, with 12 seconds of rest between rounds. Read the cue in the opening and lean left,
lean right, or duck comfortably. The headset position determines the clear, with a chime
and an in-room tally. If headset tracking or the play-area guard drops, the panels clear
and the drill pauses.

In desktop preview, use **T**, **Tab**, and **Enter** to open and navigate the clinic.
The **Left**, **Right**, and **Down** arrow keys simulate the head movements while
the preview camera stays fixed.

The live clinic smoke runs against a fresh engine started with `config/sim-training.yaml`.
It creates a test athlete, completes two sets, loads the ghost and starts a repeat session.

```sh
godot --headless --path apps/vr --max-fps 60 --xr-mode off --script tests/clinic_smoke.gd -- --desktop
```

To try the camera-only skeleton without hardware, run the engine with
`config/sim-cameras-skeleton.yaml` (four simulated cameras, no trackers).

**Validated on Windows 11 with Godot 4.5.1 (17 November 2026):** headless import, the contract
smoke script, and desktop-mode rendering against a live simulated engine (`sim-full-body.yaml`,
front kick, 12/19 observed segments) captured with `--write-movie`. The OpenXR runtime
(Virtual Desktop) was found but no headset was connected, so in-headset rendering, scale and floor
height are still unverified.

Room alignment, controller buttons and the panel have been exercised only in desktop mode and
by script (record/replay round trip against a live engine). Next: confirm in a headset room scale/floor, add calibrated XR alignment,
then avatar retargeting and native tracker roles. Current bones and joint measurements are
simulated and not clinically validated.
