# Bansayo

Open-source, local-first full-body motion tracking and training platform: wearable IMUs, cameras,
or both, feeding one tracking engine and one canonical body model that a desktop application and a
VR application share.

## Status

The local PoC implements a Rust engine, a React/three.js Studio and a Tauri desktop shell. A
Godot training-room client is included; its import and runtime validation remain unverified
because Godot is not installed and the owner chose to leave that validation unverified.

Inputs are **simulated**: virtual IMU trackers and projected camera landmarks. The simulator
proves the transport, calibration and solver plumbing. It does not establish measurement
accuracy. Heading and body proportions are assumed, and joint definitions remain
`validated: false`. With two or more simulated cameras the engine triangulates the 2D landmarks
into metric 3D landmarks, using camera models the simulator declares (there is no camera
calibration, capture or pose detector); with trackers as well, the root position is then measured
from the triangulated pelvis, otherwise it is inferred.

See the [implementation and validation record](docs/13-implementation-status.md) and
[architecture documentation](docs/README.md).

## Run the engine and Studio

Prerequisites: Rust 1.92+, Node.js 22+ and npm. Native desktop builds also need the platform build
tools (Xcode Command Line Tools on macOS; Visual Studio C++ Build Tools and WebView2 on Windows).

From the repository root:

```sh
cargo run -p bansayo-engine -- --config config/sim-full-body.yaml
```

In another terminal:

```sh
cd apps/desktop
npm ci
npm run dev -- --host 127.0.0.1
```

Open [Studio](http://127.0.0.1:5173/#/studio). Choose **Calibrate mounting → Use reference motion →
Capture mounting**. Once the capture succeeds, select **Squat** or **Front kick** in the transport
bar. Select a joint to inspect its changing angle and trace. Derived segments are solid cyan;
inferred segments are translucent purple with dashed centre lines.

Devices supports assignments and simulated disconnect/quality faults. Sessions lists MCAP input
recordings and provides replay, pause, speed, scrubbing and looping. Cameras shows simulated 2D
landmarks in image coordinates; it does not show camera video. With two or more calibrated
cameras it also shows a 3D view of the camera frustums and the triangulated landmarks, and a
landmark inspector that shows which cameras were used or rejected and why a landmark is missing.
Try it with `config/sim-multi-camera.yaml`.

The [example configurations](config/) include twelve trackers and two cameras, twelve trackers
and four cameras for triangulation (`sim-multi-camera.yaml`), six trackers without cameras, and
one camera without trackers. Both source lists may also be empty. Triangulation is tuned in the
`triangulation:` block (`max_skew_ms`, `max_reprojection_error_px`, `min_views`,
`min_ray_angle_deg`). Validate a configuration without starting the server:

```sh
cargo run -p bansayo-engine -- --config config/sim-full-body.yaml --check
```

Calibration, assignment overrides and sessions live under `engine.data_dir` (default `./data`,
relative to the working directory). Reference-pose capture checks stillness and persists mounting
profiles. Reassigning a tracker makes its previous mounting calibration stale; assigning it back
still requires recapture.

## Desktop and VR clients

```sh
cd apps/desktop
npm run tauri dev
```

The shell attaches to a compatible engine already on port 7700, or hosts the engine library. Its
bundled configuration uses the application's data directory. Set `BANSAYO_CONFIG` to use an
explicit configuration. See [desktop instructions](apps/desktop/README.md) and
[Godot instructions](apps/vr/README.md).

## API

The default endpoint is `http://127.0.0.1:7700/api/v1`. HTTP responses use
`{ "ok": true, "data": ... }` or `{ "ok": false, "error": ... }`.

- Queries: `GET /health`, `/state`, `/skeleton`, `/sessions`.
- Controls: simulation, mounting capture, device assignment/faults, recording and playback.
- Stream: `ws://127.0.0.1:7700/api/v1/stream?topics=frame,state,landmarks_2d,landmarks_3d&max_rate_hz=60`.
  `landmarks_3d` carries the triangulated landmarks, each listing the cameras used and rejected.

The first stream message is `hello`, including protocol version, conventions, skeleton and state.
Frame sequence numbers increase across mode changes. Positions use metres, +Y up and +Z forward;
Hamilton quaternions use `[x,y,z,w]`, and angles use radians. See the
[complete contracts](docs/04-contracts.md).

The API binds to loopback and checks browser origins. State-changing requests require JSON. The
default allow-list includes the Vite development URLs and Tauri origins; configure
`api.allowed_origins` when using another frontend address.

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
node scripts/verify-gate1.mjs
node scripts/verify-gate2.mjs
cd apps/desktop
npm test
npm run build
npm run test:e2e
```

The Node gate checks use temporary data directories and free ports. They start the engine with the
opt-in `--shutdown-on-stdin-close` flag, which shuts it down gracefully when stdin closes: a parent
process cannot deliver Ctrl-C to a child on Windows. They honour `CARGO_TARGET_DIR` when locating
the engine binary. The Playwright journey uses
ports 17700/15173, temporary data and the installed Chrome browser. See the desktop README for
browser configuration and native-shell checks. Contract tests verify checked-in TypeScript and
golden JSON; regenerate intentionally with `BANSAYO_BLESS=1 cargo test -p bansayo-contracts` and
review the resulting diff.

Real device adapters, physical camera capture and pose inference, camera calibration, triangulation
on real cameras, camera/IMU fusion beyond the measured root, real heading alignment, avatars,
exports and the Analysis workspace remain future work. Triangulation is written independently from
standard multi-view geometry; no FreeMoCap code is used (it is AGPL-3.0, this project is MIT), so
there is no FreeMoCap importer. Hardware, OpenXR, long-run performance and measurement accuracy
require separate validation. Every check listed above passes on Windows 11; a packaged Windows
installer has not been built.

Pretrained pose models for the future camera path, if present, live in `models/` (git-ignored;
see `models/README.md`). Nothing in the engine loads them yet.

## Licence

MIT. See [LICENSE](LICENSE).
