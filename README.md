# Bansayo

Open-source, local-first full-body motion tracking and training platform: wearable IMUs, cameras,
or both, feeding one tracking engine and one canonical body model that a desktop application and a
VR application share.

## Optional AprilTag 3 detection

AprilTag 3 detection is available as an **off-by-default** camera extension using a separately
installed native C library. It provides a per-camera HTTP inspection endpoint and an opt-in
marker overlay in the Cameras workspace; it does not yet estimate body poses from printed tags.
See [installation, configuration and validation](docs/apriltag.md).

## Status

The local PoC implements a Rust engine, a React/three.js Studio, a Tauri desktop shell and a
Godot training room (validated in desktop mode with Godot 4.5.1; not yet in a headset).

Two input paths share one pipeline. The **simulator** drives virtual IMU trackers and projected
camera landmarks. The **camera path** captures USB cameras (Windows Media Foundation) or image
folders, detects one person per view with YOLOX and RTMPose (COCO-17) on ONNX Runtime, imports
ChArUco camera calibrations, interpolates free-running cameras to one instant, triangulates, and
fits the skeleton to the 3D landmarks when no trackers are configured: joint angles then come
from cameras alone. The camera path is built and tested without hardware (image folders,
synthetic calibration scenes, simulated geometry); **no physical camera or headset has been
connected yet**, so nothing here establishes measurement accuracy. Joint definitions remain
`validated: false`.

See the [implementation and validation record](docs/13-implementation-status.md) and
[architecture documentation](docs/README.md).

## Setup

Install the system tools once: [Rust](https://rustup.rs) (the pinned toolchain in
`rust-toolchain.toml` is selected automatically), [Node.js 22+](https://nodejs.org) with npm,
Python 3.10+, and optionally [Godot 4.5](https://godotengine.org) for the VR room. Native desktop
builds also need the platform build tools (Xcode Command Line Tools on macOS; Visual Studio C++
Build Tools and WebView2 on Windows).

Then let the setup script download, verify and configure everything else:

```sh
python scripts/setup.py          # all steps; safe to re-run, skips what is in place
python scripts/setup.py check    # report what is installed, change nothing
```

| Step | What it does |
|---|---|
| `prerequisites` | Reports cargo, node, npm, Python and Godot versions |
| `models` | Downloads the YOLOX-m and RTMPose-m ONNX models into `models/` |
| `onnxruntime` | Installs ONNX Runtime 1.24.4 into `models/onnxruntime/` (DirectML on Windows, CPU on Linux x64 / macOS arm64) |
| `python` | `pip install --user opencv-python numpy` for camera calibration, if missing |
| `desktop` | `npm ci` in `apps/desktop` |
| `engine` | `cargo build --release -p bansayo-engine` |
| `godot` | Imports `apps/vr` and runs its smoke test, if Godot is on PATH |
| `verify` | Validates `config/cameras-two.yaml` against the installed models and runs the calibration selftest |

Run single steps by name (`python scripts/setup.py models onnxruntime`); `--force` replaces
files already present. Every download is checked against a pinned SHA-256 before it is
extracted. Models and runtime are local, git-ignored files; their sources and licence notes are
in [models/README.md](models/README.md). Nothing is installed system-wide except the optional
user-level pip packages.

## Run the engine and Studio

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
recordings and provides replay, pause, speed, scrubbing and looping. Cameras shows 2D landmarks
in image coordinates, over the live image for physical cameras. With two or more calibrated
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

## Attach physical cameras

1. Run `python scripts/setup.py` (see [Setup](#setup)).
2. Plug in two to four USB cameras with overlapping views of the training area (front and side
   first).
3. Run `python scripts/setup.py cameras`: it lists the attached cameras and writes
   `config/local-cameras.yaml` (git-ignored) from [config/cameras-two.yaml](config/cameras-two.yaml)
   with one entry per camera. Adjust `resolution_px` if needed: a camera that negotiates another
   resolution reports the value to set. `--check` validates the file and the model paths.
4. Calibrate with a printed ChArUco board (Python 3 with `opencv-python` and `numpy`), with the
   engine stopped while capturing:
   ```sh
   python scripts/calibrate_cameras.py board --out board.png      # print large, measure a square
   python scripts/calibrate_cameras.py capture --camera CAM-FRONT=0 --camera CAM-SIDE=1 --out calib
   cargo run --release -p bansayo-engine -- --config config/local-cameras.yaml
   python scripts/calibrate_cameras.py solve --dir calib --square-m 0.20 --engine http://127.0.0.1:7700
   ```
   Capture 15–30 sets with the board tilted across each view, then a few with it flat on the floor
   where every camera sees it: that view defines the world origin and floor. `solve` rejects
   blurred or inconsistent data by reprojection error and imports the profiles, which persist.
   `python scripts/calibrate_cameras.py selftest` checks the solver on a synthetic scene.
5. Stand in view: Cameras shows each feed with its landmarks and the 3D reconstruction; Studio
   shows the camera-only skeleton and knee/hip angles; the Godot room shows the figure, angles,
   counts and record/replay controls.

Use a release build for live cameras (inference is several times slower in debug). Without
hardware, `frames_dir:` in place of `device:` plays a folder of images as a camera, and
`config/sim-cameras-skeleton.yaml` shows the camera-only skeleton from four simulated cameras.

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
- Controls: simulation, mounting capture, device assignment/faults, recording and playback,
  camera calibration import (`POST /calibration/profiles`).
- Camera previews: `GET /cameras/{id}/preview.jpg` (newest frame, downscaled, physical cameras).
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

Physical IMU adapters, camera/IMU fusion beyond the measured root, real IMU heading alignment,
avatars, exports, technique scoring and the Analysis workspace remain future work. Triangulation is written independently from
standard multi-view geometry; no FreeMoCap code is used (it is AGPL-3.0, this project is MIT), so
there is no FreeMoCap importer. Hardware, OpenXR, long-run performance and measurement accuracy
require separate validation. Every check listed above passes on Windows 11; a packaged Windows
installer has not been built.

## Licence

MIT. See [LICENSE](LICENSE).
