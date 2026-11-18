# 07 — Desktop UI

The desktop application is a client of the engine. It renders what the engine reports and sends
commands. It contains no tracking logic, no simulation and no rules about which features are
available.

## 1. Design direction

**A laboratory instrument, not a dashboard.** The reference is motion-capture and signal-analysis
software: docked panels separated by hairlines, a dominant viewport, dense numeric readouts in a
monospaced face, colour reserved for meaning.

| Decision | Choice |
|---|---|
| Theme | Graphite by default; Silver and Onyx are visual alternatives. Tracking and provenance colours remain semantic. |
| Layout | Docked panels with hairline dividers. No floating cards on a background, no card grids as the default layout |
| Typography | IBM Plex Sans for interface text, IBM Plex Mono for every number, identifier and timestamp. Self-hosted; the app works offline |
| Colour | Semantic only. Hue means state or provenance; nothing is coloured for decoration |
| Depth | Flat panels. Only things that float (drawer, dialog, picture-in-picture) cast a shadow |
| Motion | 120 ms ease-out on state changes. A slow pulse on `acquiring` only. Honours reduced-motion |
| Density | 13 px base, compact rows, generous numerals for the values that matter |

### Colour

Colour is never the only carrier of meaning: provenance is also line style, state is also icon
shape and text.

| Token | Value | Use |
|---|---|---|
| `--surface-0` | `oklch(16% 0.012 250)` | Window, viewport background |
| `--surface-1` | `oklch(20% 0.014 250)` | Panels |
| `--surface-2` | `oklch(24% 0.016 250)` | Hover, raised controls |
| `--surface-3` | `oklch(30% 0.018 250)` | Selected rows |
| `--line` | `oklch(33% 0.015 250)` | Hairlines |
| `--ink-1` | `oklch(96% 0.005 250)` | Primary text, values |
| `--ink-2` | `oklch(80% 0.010 250)` | Secondary text |
| `--ink-3` | `oklch(66% 0.012 250)` | Labels, units |
| `--accent` | `oklch(78% 0.13 205)` | Selection, focus, primary action |
| `--state-tracking` | `oklch(78% 0.16 150)` | Tracking |
| `--state-degraded` | `oklch(80% 0.15 80)` | Degraded |
| `--state-acquiring` | `oklch(74% 0.13 250)` | Acquiring |
| `--state-disconnected` | `oklch(58% 0.010 250)` | Disconnected |
| `--state-unavailable` | `oklch(66% 0.19 25)` | Unavailable, errors |
| `--prov-measured` | `oklch(96% 0.005 250)` | Measured: solid, brightest |
| `--prov-derived` | `oklch(78% 0.13 205)` | Derived: solid |
| `--prov-inferred` | `oklch(72% 0.14 300)` | Inferred: dashed, translucent |
| `--prov-unavailable` | `oklch(50% 0.010 250)` | Unavailable: dotted outline or a dash |

Contrast of text tokens against `--surface-1` meets WCAG AA; Agent 2 verifies the final values
and adjusts lightness, not hue, if any pair falls short.

### Encoding the three labels

| Label | Encoding |
|---|---|
| **Origin** (`simulated`, replay) | A hatched neutral badge reading `SIMULATED` or `REPLAY`, always visible in the status bar and in the viewport corner while any contributing source is not live. No hue — it must not be confused with state |
| **Provenance** | Line and fill style plus the provenance hue: solid for measured and derived, dashed and translucent for inferred, an em dash in place of a number for unavailable. Every readout has a small provenance tag |
| **State** | A shaped dot (filled circle tracking, half circle degraded, ring acquiring, hollow square disconnected, cross unavailable) in the state hue, with the word |

### Type and spacing

| Scale | Size | Use |
|---|---|---|
| Label | 11 px, uppercase, +0.04 em | Panel titles, column headers |
| Small | 12 px | Secondary rows, tags |
| Base | 13 px | Body |
| Emphasis | 15 px | Section headings |
| Title | 18 px | Workspace title |
| Readout | 24 and 32 px mono, tabular figures | Primary numeric values |

Spacing follows a 4 px grid: 2, 4, 8, 12, 16, 24, 32. Panel padding is 12. Radii: 2 px for tags,
4 px for controls, 0 for docked panels, 6 px for floating surfaces. All of this is CSS custom
properties in `styles/tokens.css`; components use tokens only.

## 2. Navigation

A left navigation rail and a persistent top status bar frame every workspace.

| Key | Workspace | PoC |
|---|---|---|
| 1 | Overview | Yes |
| 2 | Studio | Yes — the primary workspace |
| 3 | Devices | Yes |
| 4 | Cameras | Yes |
| 5 | Sessions | Yes |
| — | Analysis | Designed. Not shown in the PoC navigation |
| — | Calibration | Designed as a workspace. In the PoC, calibration is a flow opened from Devices and Overview |

Workspaces that are not implemented do not appear in the navigation. The active workspace is in
the URL hash (`#/studio`), so reload and browser navigation work and a view can be linked.

### Shell

```text
┌────┬───────────────────────────────────────────────────────────────────────────────────┐
│ K  │ ● Engine connected   ▨ SIMULATED   ◐ Degraded · 11/19 segments   ⚠ Calibration   │  status bar
├────┼───────────────────────────────────────────────────────────────────────────────────┤
│ ▣  │                                                                                   │
│ ◈  │                                                                                   │
│ ⌬  │                               active workspace                                    │
│ ◉  │                                                                                   │
│ ▤  │                                                                                   │
├────┴───────────────────────────────────────────────────────────────────────────────────┤
│ Motion [Squat ▾]  1.0×  ⏸      │      ● Record      │      live                       │  transport bar
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

**Status bar**, left to right: engine connection; origin badge; system tracking state with body
coverage; calibration readiness (ready, action needed, or running on assumptions); recording
indicator with elapsed time. Each item opens the place where it can be acted on.

**Transport bar** has two forms.

- Live: simulation controls when a simulated source exists (motion, speed, pause), and Record.
- Replay: exit replay, play/pause, speed, scrubber with current and total time, loop.

## 3. Workspaces

### Overview

Purpose: answer "is the system ready, and if not, what do I do?" in one glance.

```text
┌──────────────────────────────────────────────┬──────────────────────────────────────────┐
│ SYSTEM                                       │ BODY COVERAGE                            │
│ Tracking 11 of 19 segments from 12 simulated │            ○                             │
│ trackers. 1 simulated camera.                │          ──┼──      solid  = derived     │
│                                              │            │        dashed = inferred    │
│ NEEDS ATTENTION                              │           ╱ ╲                            │
│ ⚠ SIM-T08 disconnected          → Devices    │          ╱   ┆                           │
│ ⚠ Heading alignment is assumed  → Calibrate  │                                          │
├──────────────────────┬───────────────────────┼──────────────────────────────────────────┤
│ SENSORS              │ CAMERAS               │ FEATURES                                 │
│ 11 tracking          │ 1 tracking            │ ● Skeleton pose                          │
│ 1 disconnected       │ 0 calibrated          │ ◐ Joint angles — 2 unavailable           │
├──────────────────────┼───────────────────────┤ ○ 3D landmarks — needs two calibrated    │
│ CALIBRATION          │ SESSION               │   cameras                                │
│ Mounting  11 / 12    │ Not recording         │ ○ Camera + IMU fusion — not implemented  │
│ Heading   assumed    │ 3 sessions            │                                          │
└──────────────────────┴───────────────────────┴──────────────────────────────────────────┘
```

- The system sentence is composed from counts in `SystemState`.
- "Needs attention" lists the reasons from devices, features and calibration, most severe first,
  each linking to the workspace or flow that addresses it. When empty it says so in one line.
- Body coverage is the **body map**: a front view generated from the skeleton definition's rest
  pose, each segment drawn in its provenance style. It is data-driven; a different skeleton
  produces a different map with no UI change.
- Features are listed with their reasons verbatim from the engine.

### Studio

The main working view.

```text
┌──────────────────┬──────────────────────────────────────────────────┬─────────────────────┐
│ LAYERS           │ ▨ SIMULATED                    Front Side Top 3D │ INSPECTOR           │
│ ☑ Skeleton       │                                                  │ Left knee flexion   │
│ ☑ Floor          │                                ┌──────────────┐  │                     │
│ ☑ Trackers       │                                │ SIM-CAM1     │  │   75.5°             │
│ ☐ Cameras        │               3D viewport      │ 2D landmarks │  │   derived · conf .95│
│ ☐ Trails         │                                │ ▨ simulated  │  │   not validated     │
│ ☑ Angle arcs     │                                └──────────────┘  │   ▁▂▄▆▇▆▄▂▁▂▄▆▇     │
│                  │                                                  │   SIM-T03 · SIM-T04 │
│ SEGMENTS         │                                                  ├─────────────────────┤
│ ● pelvis     der │                                                  │ JOINT ANGLES        │
│ ◌ spine      inf │                                                  │ L knee      75.5°   │
│ ● chest      der │                                                  │ R knee        —     │
│ ● l_thigh    der │   coverage 11/19 · pose age 9 ms                 │ L hip       61.2°   │
│ ◌ r_shin     inf │                                                  │ Trunk lean   8.4°   │
└──────────────────┴──────────────────────────────────────────────────┴─────────────────────┘
```

- **Left panel.** Layer toggles, then the segment list in hierarchy order with provenance and
  source device. Selecting a row selects the segment in the viewport, and the reverse.
- **Viewport.** Described in §4.
- **Camera inset.** When cameras exist, a picture-in-picture panel shows the selected camera's
  landmarks on its image plane. With several cameras, a thumbnail strip selects which one. The
  inset can be enlarged to share the viewport half-and-half.
- **Inspector.** For the selected joint measurement: value in degrees, provenance, confidence,
  the "not validated" tag, a rolling trace of the last ten seconds, contributing devices. For a
  selected segment: provenance, source device, device state, calibration state, with a link to
  the device. Below it, the table of all joint measurements; unavailable ones show a dash and
  their reason on hover and focus.

Angles are shown in degrees with one decimal. The wire unit is radians; conversion happens in one
formatting helper.

### Devices

Purpose: see every device, place trackers on the body, and act on problems.

```text
┌─────────────────────────────────────────────────────────────┬───────────────────────────┐
│ DEVICES  12 trackers · 1 camera        [ Cards | Table ]    │ BODY MAP                  │
│ ┌───────────────────────────┐ ┌───────────────────────────┐ │           ○               │
│ │ ● SIM-T01     ▨ simulated │ │ ▢ SIM-T08    ▨ simulated  │ │         ──┼──             │
│ │ Simulated IMU             │ │ Simulated IMU             │ │           │               │
│ │ Segment  [pelvis      ▾]  │ │ Segment  [right_shin  ▾]  │ │          ╱ ╲              │
│ │ 99.8 / 100 Hz   0 dropped │ │ Disconnected 14 s ago     │ │                           │
│ │ Battery 82%   −54 dBm     │ │ Battery —     — dBm       │ │ Unassigned segments: 7    │
│ │ Mounting ● valid          │ │ Mounting ● valid          │ │                           │
│ │ Heading  ◌ assumed        │ │ Heading  ◌ assumed        │ │ [ Calibrate mounting ]    │
│ │ Quality  ▇▇▇▇▇▇▇▇▇░ 0.97  │ │ [ Reconnect ] (simulated) │ │                           │
│ └───────────────────────────┘ └───────────────────────────┘ │                           │
└─────────────────────────────────────────────────────────────┴───────────────────────────┘
```

Each device shows what the brief asks for: hardware type, connection state, battery, sampling
rate measured against nominal, signal strength, firmware when known, segment assignment,
calibration state per kind, data quality. Fields the device does not report show a dash; they are
not hidden, so cards align.

- **Assignment** is a select on the card listing the skeleton's segments, free ones first.
  Changing it calls the engine; the card and body map update from the next state message. The UI
  states the consequence before confirming a change that will invalidate calibration.
- **Body map** highlights the hovered or focused device's segment, and lists unassigned segments.
- **Fault controls** (disconnect, degrade) appear only on simulated devices and are labelled as
  simulation controls.
- **Calibrate mounting** opens the reference-pose flow (§5).
- Above twelve devices the default view becomes a table with the same fields as columns.

### Cameras

```text
┌───────────────────────────────────────────────┬─────────────────────────────────────────┐
│ ┌───────────────────────────────────────────┐ │ SIM-CAM1                 ● tracking     │
│ │                                           │ │ Source      simulated                   │
│ │        2D landmarks on the image plane    │ │ Resolution  1280 × 720                  │
│ │        ▨ simulated — no image             │ │ Frame rate  29.9 / 30 fps               │
│ │                                           │ │ Landmarks   19 of 19 detected           │
│ └───────────────────────────────────────────┘ │ Detector    simulated-projection        │
│  SIM-CAM1                                     │ Time        estimated                   │
│                                               │ CALIBRATION                             │
│                                               │ Intrinsics  declared by simulator       │
│                                               │ Extrinsics  declared by simulator       │
│                                               │ Sync        single camera               │
└───────────────────────────────────────────────┴─────────────────────────────────────────┘
```

- The preview shows one camera at a time, chosen from a tab row of camera ids. Beside it is the
  **3D reconstruction view** (below). The sketch above shows the single-camera layout.
- Landmarks are drawn from the camera's `LandmarkSchema` (names and edges come from the engine).
  Confidence drives point opacity; undetected points are not drawn.
- A simulated camera has no image. Its preview says so and shows landmarks on a neutral field. It
  never shows a fake video frame.
- The detail panel shows source, resolution, measured frame rate, landmark count, detector, time
  quality, calibration states and synchronisation status.
- **3D reconstruction view.** Shown when at least two cameras report usable intrinsics and
  extrinsics (read from the calibration profiles in `SystemState`, which carry the camera data for
  this purpose) and the cameras declare a landmark schema. It draws a wire frustum per calibrated
  camera, labelled with its id and highlighting the selected one, and the triangulated landmarks
  from the `landmarks_3d` stream as markers joined by the schema's edges, over the floor grid. Marker
  opacity and size follow confidence; a wire octahedron marks a reprojection error above 5 px. A
  set older than half a second is not drawn, and the view says it is waiting. With fewer than two
  calibrated cameras it shows an empty state saying how many report usable calibration. The
  feature state and its reasons from the engine appear under the view.
- **Landmark inspector.** Selecting a landmark, in the 3D view or in the landmark list, answers why
  a joint is misplaced or missing: its status (`accepted`, `high error`, `not reconstructed`), world
  position, number of views used, reprojection RMS and confidence, and a row per camera with its
  role (`used`, `rejected`, `not used`) and its own 2D observation (pixels and confidence, "not
  detected", or "no 2D data"). The selected landmark is also highlighted in the camera preview.
  Status is shown as a glyph and a word, never colour alone.
- Studio can show the same frustums through its Cameras layer toggle, off by default.
- Everything here is simulated in the PoC: no video is captured and no detector runs.
- Designed: live video in the preview, source selection, and the entry to intrinsic and extrinsic
  calibration.

### Sessions

```text
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│ SESSIONS                                                                 [ ● Record ]   │
│ Name               Date                 Duration   Devices   Origin        Size         │
│ Squat set 2        2026-11-16 10:22     01:42      12 + 1    ▨ simulated   3.1 MB   ▶   │
│ Kick test          2026-11-16 09:58     00:37      6         ▨ simulated   0.6 MB   ▶   │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

Playing a session switches the engine to replay, opens Studio, and turns the transport bar into
playback controls. The status bar shows `REPLAY` and the session name until the user exits.

Designed: rename, delete, export, tags, side-by-side comparison.

### Analysis (designed)

Exercise-specific views: stretching (peak range per joint against previous sessions), squats
(depth, knee tracking, left/right symmetry), kicks (chamber and extension angles, peak height,
phase timing), posture and balance (trunk lean, sway). Shared building blocks: joint-angle
charts over time, left/right overlays, repetition markers, movement-phase bands, session
progress. The engine computes every metric; this workspace only charts them.

### Calibration (designed as a workspace)

One page per procedure from [05](05-calibration-and-fusion.md), each with: what it is for, what
to do, a live indicator of whether the precondition holds (stillness, visibility), the capture
step, the result, and what it invalidated.

## 4. The 3D viewport

### Scene

| Element | Design |
|---|---|
| Background | `--surface-0` with a faint vertical gradient. No skybox |
| Floor | Grid on `y = 0`: minor lines every 0.5 m, major every 1 m, fading with distance. Origin marker with +X and +Z ticks labelled in metres |
| Skeleton | Bones as tapered capsules between a segment's origin and its tail; joints as small spheres. Matte material, one hemisphere light and one key light for form |
| Provenance | Measured and derived: opaque, in the provenance colour. Inferred: translucent fill with a dashed centre line. No pose: nothing is drawn and the viewport shows the reason |
| Selection | Accent outline on the selected segment; its row is highlighted in the segment list |
| Angle arcs | For the selected joint (or all, by layer toggle): a sector between the two segment axes with the value as a screen-space label |
| Trackers | A small oriented box on each segment that has a tracker, at its midpoint, in the device's state colour. The label says the position on the bone is illustrative: tracker position is not measured |
| Cameras | A wire frustum per camera whose extrinsics are known, in the camera's state colour. Hidden when extrinsics are missing |
| Trails | A fading line following the distal end of the selected segment over the last two seconds |
| Heads-up | Bottom left: coverage, pose age. Top left: origin badge. Top right: view presets |

### Interaction

| Input | Action |
|---|---|
| Drag | Orbit |
| Right-drag or Shift-drag | Pan |
| Wheel | Zoom |
| Click a bone | Select segment |
| Double-click a bone | Centre the view on it |
| `F`, `L`, `T`, `P` | Front, left side, top, perspective |
| `Home` | Reset view |
| `Tab` within the segment list | Keyboard path to everything the mouse can select |

### Implementation rules

- three.js is used imperatively. A `MotionViewport` class owns the renderer, scene, camera,
  controls, resize observer and animation loop. A thin React component mounts and disposes it.
- The animation loop reads the latest frame from a plain store. **Pose data never passes through
  React state.** React holds only low-rate things: selection, layer toggles, and readouts sampled
  at about 10 Hz.
- Meshes are created once per skeleton revision and updated in place. No allocation per frame.
- Rendering pauses when the tab is hidden. The canvas follows its container size.
- If no frame has arrived for half a second, the skeleton dims and the viewport shows "No pose
  data" with the reason from the engine.
- Frames are drawn as received. Interpolation is not needed at the publish rate and would hide
  timing problems during development.

## 5. Interaction flows

### First run with the simulator

1. The app opens on Overview. The status bar shows the engine connection, `SIMULATED`, and
   "Calibration: action needed".
2. "Needs attention" lists "Mounting calibration missing for 12 trackers" with a Calibrate action.
3. The user opens the reference-pose flow, completes it, and is offered Studio.
4. In Studio the user picks a motion from the transport bar and selects a joint to inspect.

### Reference-pose calibration

```text
Step 1  Explain     "Stand in a T-pose facing forward and hold still."
                    Simulator present → [ Switch simulator to Reference pose ]
Step 2  Countdown   3 · 2 · 1, then a one-second capture with a progress bar
Step 3  Result      Per tracker: ✓ captured, or ✕ with the reason
                    [ Recapture failed ]   [ Done ]
```

Failures are the engine's messages: movement detected, tracker not delivering data, not assigned.

### Reassigning a tracker

Select a new segment on the device card → the UI warns that mounting calibration for this tracker
will need recapturing → confirm → the card shows mounting `missing`, the old segment becomes
inferred in the viewport, and Overview lists the new action.

### Record and replay

Record in the transport bar → elapsed time in the status bar → Stop → the session appears in
Sessions. Play it → Studio in replay with the scrubber → Exit replay returns to live.

### Losing a tracker

The device card changes to disconnected with elapsed time; in the viewport the limb returns to its
parent-relative rest pose in the inferred style; affected joint angles become dashes with a
reason; the status bar goes to degraded and coverage drops; Overview lists it.

## 6. States every view handles

| Condition | Behaviour |
|---|---|
| Connecting | The shell renders immediately. Panels show skeleton placeholders and "Connecting to the engine at 127.0.0.1:7700" |
| Engine disconnected | A persistent banner with the retry countdown. All live values are cleared to dashes — the last values are not left on screen. The viewport dims. Navigation still works |
| Protocol mismatch | A blocking message naming both versions. No data is rendered |
| No devices | Devices explains that sources are defined in the configuration file and names the three example configurations |
| No cameras | Cameras shows an empty state; Studio hides the camera inset |
| No pose | Studio shows the floor and the reasons from the `skeleton_pose` feature |
| Feature unavailable | The control is present but disabled, with the engine's reason. It is not hidden, so users learn what exists |
| Action failed | The engine's error message appears next to the control that caused it, and in a polite live region |
| Recording dropped events | A warning beside the recording indicator with the count |

## 7. Adapting to what is connected

The UI holds no rules about sensors. It reads devices, capabilities and features, and lays out
accordingly.

| Inputs | Studio | Elsewhere |
|---|---|---|
| None | Floor and an empty state | Overview: "No inputs configured" |
| Trackers only | 3D viewport full width; no camera inset | Cameras: empty state |
| Cameras only | The camera view becomes the main view; the 3D viewport collapses to a strip stating why 3D pose is unavailable | Devices lists cameras only |
| Both | 3D viewport with camera inset | — |
| Several cameras | Thumbnail strip in the inset | Camera grid reflows by count |
| More than twelve devices | — | Devices defaults to the table view |
| A different skeleton | Segment list, body map and joint table regenerate from the definition | — |

Cards, columns and readouts are driven by capabilities: a device without `battery` shows a dash
in the battery field; a device kind the UI has never seen is listed with its capabilities and a
generic icon.

## 8. Responsiveness and accessibility

| Width | Layout |
|---|---|
| ≥ 1280 px | Three columns in Studio; navigation rail with labels |
| 960 – 1279 px | The inspector becomes a collapsible right drawer; rail shows icons only |
| 800 – 959 px | Both side panels become overlay sheets over the viewport |
| Minimum | 800 × 600 |

- Everything reachable by mouse is reachable by keyboard, with a visible focus ring in the accent
  colour.
- Shortcuts: `1`–`5` workspaces, `Space` play/pause, `R` record, `[` and `]` speed, `Esc` closes
  a drawer or dialog. Shortcuts are shown in tooltips and do not fire while typing in a field.
- Controls are native elements (`button`, `select`, `input[type=range]`, `dialog`) with labels.
  The layout uses landmarks: `header`, `nav`, `main`, `aside`, `footer`.
- State changes that matter (engine disconnected, device lost, recording started or stopped) are
  announced through a polite live region.
- The segment list and joint table are the non-visual equivalent of the viewport.
- Motion respects `prefers-reduced-motion`.

## 9. Component architecture

```text
src/
├── engine/        EngineConnection (socket, backoff, protocol check)
│                  http (typed ApiResponse client), actions (one function per API route)
│                  stores: connection, system state, skeleton, latest frame, latest landmarks
│                  (2D and triangulated 3D), cameraCalibration (camera poses from profiles)
│                  hooks: useSystemState(selector), useConnection(), useSampledFrame(hz)
├── shell/         AppShell, NavRail, StatusBar, TransportBar, LiveRegion
├── workspaces/    overview/  studio/  devices/  cameras/  sessions/
├── viewport/      MotionViewport, SkeletonRig, LandmarkRig, picking, ViewportCanvas (React wrapper)
│                  layers/ Floor, Trackers, CameraFrustums, Trails, AngleArcs
├── calibration/   ReferencePoseFlow
├── ui/            Panel, StatePill, ProvenanceTag, OriginBadge, Readout, Sparkline, BodyMap,
│                  LandmarkCanvas, EmptyState, Reasons, Field, Select, Button, Dialog, Drawer
├── lib/           quaternion and vector helpers, formatting, hash routing, shortcuts
└── styles/        tokens.css, base.css
```

Rules:

- **Only `engine/` touches the wire.** Components receive typed data from hooks and call
  `actions`.
- Types come from `contracts/ts`. No component declares its own version of a contract type.
- The system-state store is the single source for everything low-rate. Derived values (counts,
  coverage, attention list) are computed by selectors, not stored.
- Components in `ui/` are presentational and take props only.
- CSS Modules per component, tokens for every colour, size and duration.
- No UI framework, state library, router or chart library. The sparkline and landmark view are
  small canvas components.

## 10. Desktop shell (Tauri)

The shell adds a native window and lifecycle around the same web UI.

- On start it runs the engine library in-process with the configuration file given on the command
  line or found in the default location. If the configured address is already serving an engine,
  it attaches to that one instead.
- The UI in the webview talks to the engine over HTTP and WebSocket exactly as the browser build
  does. Tauri's IPC is not used for tracking data.
- The webview's content security policy allows connections only to the engine's address.
- Closing the window shuts the hosted engine down gracefully, finalising any recording.

## 11. PoC subset and priority

All five workspaces ship, at different depths. If time runs short, lower rows are cut first and
the cut is reported.

| Priority | Deliverable |
|---|---|
| 1 | Engine connection, stores, shell, status bar, all connection and error states |
| 2 | Studio: viewport with skeleton, provenance styling, floor, selection, inspector, joint table, simulation controls |
| 3 | Devices: cards, assignment, body map, simulated fault controls, reference-pose flow |
| 4 | Sessions and transport: record, list, replay with scrubber |
| 5 | Cameras: landmark preview and detail panel; camera inset in Studio |
| 6 | Overview |
| 7 | Viewport extras: tracker markers, camera frustums, trails, angle arcs |
| 8 | Tauri shell |

## Workbench shell (2026-11)

The desktop interface keeps the React, Tauri and imperative Three.js architecture. The shared
workbench includes a compact menu, workspace tabs, an expandable icon rail, and a persistent status
and transport area. Menu entries route to existing workspaces or trigger existing actions.

Studio has a compact view/layer toolbar, a searchable scene explorer synchronized with skeleton
selection, and a joint/segment property inspector. Desktop widths are adjustable by pointer or
keyboard, persisted locally under `bansayo.studio.docks.v1`. The Analysis, Capture and Focus
presets reset widths. Below 1280px, existing mobile/tablet drawers replace desktop resizing.

Visual theme (Graphite, Silver, Onyx), density (Compact, Comfortable), and rail expansion
persist under `bansayo.ui.preferences.v1`. Semantic tracking states, provenance marks,
disabled controls, and numerical validity labels must remain legible in every theme.

The renderer is not remounted by adjusting panel widths or changing visual preferences.
Camera sources, training logic, API contracts, and VR presentation are unaffected. The workbench
does not introduce fake tool actions: its controls navigate or operate existing capabilities.
