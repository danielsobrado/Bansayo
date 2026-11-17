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
godot --headless --path apps/vr --script tests/contract_smoke.gd
```

**Validated on Windows 11 with Godot 4.5.1 (17 November 2026):** headless import, the contract
smoke script, and desktop-mode rendering against a live simulated engine (`sim-full-body.yaml`,
front kick, 12/19 observed segments) captured with `--write-movie`. The OpenXR runtime
(Virtual Desktop) was found but no headset was connected, so in-headset rendering, scale and floor
height are still unverified.

Next: confirm in a headset room scale/floor, add calibrated XR alignment,
then avatar retargeting and native tracker roles. Current bones and joint measurements are
simulated and not clinically validated.
