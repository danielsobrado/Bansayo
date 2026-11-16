# Kinexa

Open-source, local-first full-body motion tracking and training platform: wearable IMUs, cameras,
or both, feeding one tracking engine and one canonical body model that a desktop application and a
VR application share.

## Status

The local PoC implements a Rust engine, a React/three.js Studio and a Tauri desktop shell. A
Godot training-room client is included; its import and runtime validation remain unverified
because Godot is not installed and the owner chose to leave that validation unverified.

Inputs are **simulated**: virtual IMU trackers and projected camera landmarks. The simulator
proves the transport, calibration and solver plumbing. It does not establish measurement
accuracy. Heading and body proportions are assumed, root position is inferred, and joint
definitions remain `validated: false`.

See the [implementation and validation record](docs/13-implementation-status.md) and
[architecture documentation](docs/README.md).

## Run the engine and Studio

Prerequisites: Rust 1.92+, Node.js 22+ and npm. Native desktop builds also need the platform build
tools (Xcode Command Line Tools on macOS; Visual Studio C++ Build Tools and WebView2 on Windows).

From the repository root:

```sh
cargo run -p kinexa-engine -- --config config/sim-full-body.yaml
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
landmarks in image coordinates; it does not show camera video or a camera-derived world pose.

The [example configurations](config/) include twelve trackers and two cameras, six trackers
without cameras, and one camera without trackers. Both source lists may also be empty. Validate
a configuration without starting the server:

```sh
cargo run -p kinexa-engine -- --config config/sim-full-body.yaml --check
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
bundled configuration uses the application's data directory. Set `KINEXA_CONFIG` to use an
explicit configuration. See [desktop instructions](apps/desktop/README.md) and
[Godot instructions](apps/vr/README.md).

## API

The default endpoint is `http://127.0.0.1:7700/api/v1`. HTTP responses use
`{ "ok": true, "data": ... }` or `{ "ok": false, "error": ... }`.

- Queries: `GET /health`, `/state`, `/skeleton`, `/sessions`.
- Controls: simulation, mounting capture, device assignment/faults, recording and playback.
- Stream: `ws://127.0.0.1:7700/api/v1/stream?topics=frame,state,landmarks_2d&max_rate_hz=60`.

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

The Node gate checks use temporary data directories and free ports. The Playwright journey uses
ports 17700/15173, temporary data and the installed Chrome browser. See the desktop README for
browser configuration and native-shell checks. Contract tests verify checked-in TypeScript and
golden JSON; regenerate intentionally with `KINEXA_BLESS=1 cargo test -p kinexa-contracts` and
review the resulting diff.

Real device adapters, physical camera capture/inference, optical triangulation/fusion, real heading
alignment, avatars, exports and the Analysis workspace remain future work. Windows, hardware,
OpenXR, long-run performance and measurement accuracy require separate validation.

## Licence

MIT. See [LICENSE](LICENSE).
