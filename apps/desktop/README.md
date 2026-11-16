# Kinexa desktop

Vite, React, TypeScript and imperative three.js, hosted by a Tauri 2 shell or a browser.
The UI consumes the generated protocol-v1 contracts in `../../contracts/ts`.

From the repository root:

```sh
cargo run -p kinexa-engine -- --config config/sim-full-body.yaml
```

In a second terminal:

```sh
cd apps/desktop
npm ci
npm run dev -- --host 127.0.0.1
```

Open `http://127.0.0.1:5173`. The engine defaults to `http://127.0.0.1:7700`.
For browser development, set `VITE_ENGINE_URL` before starting Vite or building the frontend and
allow the frontend origin in the engine configuration. Fonts are bundled locally.

1. Overview reports the engine’s input, calibration and feature states.
2. Open **Calibrate mounting**, choose **Use reference motion**, then **Capture mounting**.
3. In Studio select a motion, a segment or a joint. The viewport uses the engine’s metre/Y-up,
   Hamilton-xyzw coordinates directly. Derived and inferred segments have distinct styles.
4. Devices supports assignment changes with a calibration warning, and labelled simulated faults.
5. Cameras displays projected 2D landmarks without pretending there is video or metric 3D data.
6. Record a session, stop it, then open Sessions and replay it. Replay has play/pause, speed,
   scrubbing and exit controls. Choose looping in Sessions before starting replay.

Navigation uses hash URLs. Shortcuts: `1`–`5` workspaces, `Space` play/pause, `R` record/stop,
`[`/`]` speed, `F`/`L`/`T`/`P` viewport presets, `Home` perspective, `Escape` closes sheets/dialogs.
Shortcuts leave text entry and native control activation alone. Side panels become drawers at
narrower sizes; the minimum supported window is 800×600.

## Native shell

With Rust and platform build tools installed:

```sh
npm run tauri -- dev
npm run tauri -- build --debug --bundles app  # macOS local .app
npm run tauri -- build                      # target-platform distributables
```

The shell attaches to a compatible local protocol-v1 engine on port 7700, or hosts the engine
library using the bundled full-body configuration. It validates the app data directory before
starting. Hosted data lives in the platform app data directory for `org.kinexa.desktop`.
`KINEXA_CONFIG=/absolute/path/to/config.yaml` selects a configuration and preserves its data
path; the native UI currently requires its `api.bind` to remain `127.0.0.1:7700`.
Quitting the shell gracefully stops only an engine that the shell itself started. An attached
standalone engine remains running. The webview exposes no filesystem or shell commands.

## Checks

```sh
npm run typecheck
npm test
npm run build
npm run test:e2e
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

The browser E2E journey starts an isolated engine on 17700, a frontend on 15173 and a temporary
data directory, then removes its data. It uses an existing Chrome installation; set
`KINEXA_BROWSER_EXECUTABLE` to an existing Chromium executable instead if needed. It never
downloads a browser. It checks reference capture, live joint motion, a disconnected shin making
the dependent knee unavailable, recovery, recording/replay, all workspaces at 1440/1024/800 px,
page errors, and saves Studio screenshots under `test-results`.

TypeScript/helper tests, real-engine browser E2E, frontend production build, native Rust compile,
probe test and clippy have passed on macOS arm64. Native WebView interaction is a separate
runtime check; see the repository validation report for the latest evidence. Windows packaging,
real trackers/cameras, headset/OpenXR operation and clinical angle validation remain unverified.
