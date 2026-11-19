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
The live avatar is a separate figure. Room setup reports whether XR alignment is assumed.

Set `BANSAYO_ENGINE_URL=http://127.0.0.1:7700` to override the `bansayo/engine_url` project setting.
Reconnect uses capped backoff. A 50 ms presentation buffer interpolates positions and Hamilton
xyzw rotations, preserving provenance transitions. This is a buffer setting, not measured latency.
The buffer is bounded to 16 frames. No pose extrapolation occurs; data clears after a 500 ms gap.

Validation commands, when Godot is installed:

```sh
godot --headless --path apps/vr --editor --quit
godot --headless --path apps/vr --xr-mode off --script tests/contract_smoke.gd
godot --headless --path apps/vr --xr-mode off --script tests/spatial_ui_smoke.gd
```

## Training view

Setup shows connection, motion tracking and camera readiness. **Tracking diagnostics** contains
camera rates, joint angles, peaks and diagnostic counts. The Rust training session supplies the
clinic's rep count, rest timer, readiness gates and coaching. During an active set, a small
world-space display shows the exercise, progress and one cue. Open the controls to pause.

In the headset, aim either controller and press the trigger to select. Point at the panel and
hold grip to move it. Use the thumbstick while pointing at a column to scroll it. **Recenter**
places the panel in front of you at the selected height. The 3D panel uses a Godot SubViewport
and a depth-tested quad in both XR and desktop preview. A native OpenXR composition layer
could improve text sharpness and needs a headset comparison against the current panel.
[Godot's composition layer guide](https://docs.godotengine.org/en/4.5/tutorials/xr/openxr_composition_layers.html)

| Action | Headset | Desktop |
|---|---|---|
| Open/dismiss controls | Menu or B/Y | Esc |
| Open clinic | Setup button or left stick click | Setup button or T |
| Select | Aim + trigger | Click, or Tab / Enter in the clinic |
| Scroll panel | Aim + thumbstick | Aim + mouse wheel |
| Move panel | Aim + hold grip | Recenter button |
| Align the room | Right stick click | — |
| Start/stop recording | Tracking diagnostics | R |
| Replay newest session / back to live | Tracking diagnostics | P |

**Preferences** saves panel size and height, the Performance Studio or Light Dojo palette, avatar style and feedback sounds
locally. The mannequin uses the engine's segment proportions and provenance. White indicates
measured segments, cyan indicates derived segments and translucent violet indicates inferred
segments. Inferred hands and feet show their origins as markers. Unavailable segments are
hidden. The technical skeleton is also available in preferences.

**Room alignment.** With calibrated cameras, the engine's world is the calibration board's frame
(origin at its first inner corner, +X along its first row, +Y up). Lay the right controller on
that corner pointing along +X and click the thumbstick: the figure is then placed where the
cameras see you, at the headset's floor height. The alignment is saved in
`user://xr_alignment.cfg`; without it the figure stands at the room origin in front of you.

### Pose mirror

Open **Pose mirror** from setup or the Taekwondo clinic. In desktop preview, press **M**.
The reflection card mirrors the selected camera's 2D landmarks horizontally. The reference
card shows an illustrated pose. Use **Switch camera** to pick the view that best matches the
guide. Low-confidence points are omitted and the reflection clears when camera frames stop.

**Follow session** shows the current item in Full Body in 20 or the current phase and side
of the front or roundhouse kick, including in the clinic. **Workouts**, **Techniques**, and **Flexibility** also work before a session. Use **Next guide** to browse.
Technique previews have separate phase and side controls. The flexibility previews include
overhead reach, side bend, and calf stretch.

On physical cameras configured for body detection, the engine uses YOLOX to locate the person
and RTMPose to estimate the landmarks. The mirror works with one camera and draws an
approximate silhouette around its detected joints. The guide is an unscored 2D example.
Camera viewpoint and occlusion affect what the reflection can show.
Both avatars use dark silhouettes with colored bones and white keypoint markers. Missing
landmarks leave gaps in the skeleton. Framing uses a slowly updated torso scale, and the
confidence label summarizes the available keypoints. [Redesign screenshot and matching camera settings](../../docs/validation/2026-11-19-vr-coaching-visuals.md)

### Taekwondo clinic

Open the clinic, pick an athlete, kick and side, and use **Sets** and **Rest** to configure the
rounds. Each set has four repetitions. Review the safety notes and confirm the clear area
before starting. The headset panel plots the reported boundary against the required radius
around you. It also reports headset tracking and room alignment. Check your physical clearance,
including the floor and overhead, as part of confirming the area. The studio mat provides a
visual floor reference.
[Godot XR play-area documentation](https://docs.godotengine.org/en/4.5/classes/class_xrinterface.html#class-xrinterface-method-get-play-area)
The panel follows the engine's counts, rest clock and coaching. After finishing, choose
**Review sets & replay ghost** or **Practise again**. The keyboard controls run the same flow
in desktop preview.

With motion-data consent, the clinic saves motion on the engine PC. **Load previous practice
ghost** uses a saved attempt for the same kick and side. The ghost starts at half speed on a separate display. Review controls let you pause, restart,
or choose quarter, half or full speed. Its foot path and peak marker require a graded height in metres. Use it
as a visual guide while practising in place. Desktop and Android show the same stored set
comparison. Actual headset bounds, input and rendering still need a headset trial.

#### Floating kick targets

Align the headset to the calibration board with the right stick, then turn **Targets** on in
the clinic menu. Prepare the clinic and hold the right controller at a comfortable target
point. Aim the left controller at **Place target & start** and press its trigger. A clear floor, calibrated tracking
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

In desktop preview, the **Left**, **Right**, and **Down** arrow keys simulate the head
movements while the preview camera stays fixed.

The live clinic smoke runs against a fresh engine started with `config/sim-training.yaml`.
It creates a test athlete, selects controls via spatial rays, pauses and resumes practice,
completes two sets, controls the review ghost, and starts a repeat session.

```sh
godot --headless --path apps/vr --max-fps 60 --xr-mode off --script tests/clinic_smoke.gd -- --desktop
```

To try the camera-only skeleton without hardware, run the engine with
`config/sim-cameras-skeleton.yaml` (four simulated cameras, no trackers).

**Headset validation is pending.** The desktop preview and simulator smokes exercise the
application's ray intersection and clinic state. A headset trial is still needed to check real
controller bindings, stereo text clarity, panel placement, room scale and floor height, guard
transitions, and frame times at the runtime's selected refresh rate.
