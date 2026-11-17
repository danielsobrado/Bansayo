# 31 — Offline operation, deployment and device lifecycle

Supported ways to install and run the platform, what may run on a different machine, how devices
are found, checked and kept compatible, and how the installation updates and recovers.

Decision record: [ADR-031](adr/ADR-031-local-first-execution.md). Status: the loopback engine,
native shell, `scripts/setup.py` and `--list-cameras` are **Implemented**; the rest is
**Proposed**.

## 1. Capability card

| Item | Content |
|---|---|
| Use case and value | Training works with the network cable unplugged; a new camera or tracker is recognised or refused with a reason; an update never corrupts history; a failed GPU provider degrades instead of stopping |
| Existing support | One-process engine on loopback; Tauri shell that hosts or attaches; source supervision with reconnect; device states and reasons; `setup.py` (models with SHA-256, ONNX Runtime, dependencies, camera configuration, status check); execution-provider fallback; persisted calibration and assignments |
| Needed now | The compatibility fields of §4 on `DeviceDescriptor`; schema-version discipline for every persisted file (already the rule); the retention job ([12 §8](12-data-and-progress.md)) |
| Dependencies | Physical devices; a packaged installer (not yet built per docs/13) |
| Feasibility and limits | High. Limits: no automatic updates and no remote diagnostics by design; standalone headsets need the LAN mode |
| Priority | P1 |
| Smallest milestone | A packaged Windows installer that runs a simulated session on a machine with no network and no development tools |
| Evidence before development | Physical-camera bring-up; the installer build listed as not done in docs/13 |

## 2. Deployment models

| Model | Where things run | Network | Status |
|---|---|---|---|
| **A — Desktop** | Engine (inside the native shell or standalone), desktop UI, cameras and trackers on one Windows PC | None required. Trackers on Wi-Fi need the local network only | Supported; primary |
| **B — Desktop + PC VR** | As A, plus the Godot client on the same PC driving a tethered or streamed headset through the PC's OpenXR runtime | Loopback between client and engine; the headset link is the runtime's concern | Supported target for VR |
| **C — Desktop + standalone headset** | Engine and cameras on the PC; the Godot client on the headset | LAN mode with token ([17 §2.2](17-security-privacy-and-safety.md)) | Proposed, P2 |
| **D — Browser UI** | Engine on the PC; UI in a browser on the same PC | Loopback | Supported for development |
| Engine on a different machine from the cameras | — | — | **Not supported.** Capture, inference and solving share one process and one clock by design |
| Analysis or storage on another machine | — | — | Not supported; exchange is by export bundles ([26](26-instructor-and-remote-coaching.md)) |
| Cloud anything | — | — | Not supported ([ADR-031](adr/ADR-031-local-first-execution.md)) |

What may run elsewhere: **only clients** (model C), and offline tools working on exported data.

## 3. Offline operation

| Aspect | Rule |
|---|---|
| Runtime network calls | None by engine or clients |
| Installation | `setup.py` downloads models and runtime once, verifying SHA-256; afterwards nothing is fetched. A fully offline install is possible by copying the model and runtime files |
| Content | Shipped with the application; user content local |
| Time | The session clock is monotonic and local; wall-clock time is used only for labels. A wrong system clock mislabels sessions and cannot break measurements |
| Licences and activation | None |

## 4. Device discovery, compatibility and health

| Device | Discovery | Compatibility check | Health reported |
|---|---|---|---|
| Cameras | OS enumeration (`--list-cameras`, exists); configured by name and resolution | Declared resolution and rate must be delivered (exists); manual-exposure capability probed; mismatch → `unavailable` or `degraded` with the reason | Delivered rate, dropped frames, decode time, exposure in effect, reprojection drift |
| IMU trackers via SlimeVR | The SlimeVR server's feed | Server version pinned and reported; unknown version → `degraded` with reason `unverified_server_version` | Battery, signal, rate, loss — as far as the feed exposes them |
| ESP32 trackers | UDP announce on the local network (draft telemetry) | `DeviceDescriptor` gains `firmware: { name, version, protocol }`; the engine holds a table of supported protocol versions; outside it → `unavailable` with `unsupported_firmware` and the version range expected | Battery, temperature, loss fraction, clock-offset stability, saturation counts ([08 §5.4](08-tracking-and-sensor-fusion.md)) |
| Tag clusters | Configuration (`trackables`); seen ids reported, unregistered ids listed | Carrier model exists and face count matches | Decode rate, last seen, single-view share |
| Headset and controllers | The OpenXR runtime | Capability checks for each optional extension ([25 §6](25-mixed-reality-and-avatars.md)) | Tracking validity, boundary state |
| Instrumented equipment | As its adapter provides | As trackers | Battery, bounce/threshold diagnostics |
| GPU | Execution-provider probe at start | Ordered list; fallback provider → camera `degraded` with the provider named (exists in design) | Inference time per frame, queue depth |

Every refusal names what was found and what is required; nothing fails silently
(existing rule, [docs/09](../../09-robustness-and-quality.md)).

Firmware updates are **not** performed by the engine. The Devices workspace shows the installed
and the expected protocol version and links to the flashing instructions; flashing stays a manual,
wired step.

## 5. Software lifecycle

| Topic | Rule |
|---|---|
| Updates | Manual: the user installs a new version. No update check. Release notes state changes to observables, metrics, shipped definitions and default tracking settings — the identifiers of [32 §5](32-measurement-provenance.md) I2 |
| Data migrations | SQLite migrations by `user_version`; JSON and YAML by `schema_version` / schema major. A migration failure leaves files untouched and stops with the migration named ([12 §9](12-data-and-progress.md)) |
| Downgrade | Not supported after a migration; the installer backs up `db/` before migrating and says where |
| Content updates | Shipped definitions change only by version increment; stored snapshots keep old sessions explainable |
| Model updates | A new pose model is a new detector id: a new comparability class, announced in the release notes |
| Golden regression | Run before each release ([28](28-dataset-and-regression.md)) |
| Configuration | One engine file per rig; `setup.py cameras` writes a git-ignored local file (exists) |

## 6. Recovery

| Event | Behaviour |
|---|---|
| Engine crash or power loss mid-session | Session row becomes `interrupted` at next start; MCAP files are closed as far as written and reported as damaged if unreadable; `pending.jsonl` is replayed into the store |
| Disk full | Recording stops with a notice; the session continues unrecorded; analytics writes are spilled then dumped ([14 §3](14-runtime-and-api-contracts.md)) |
| Camera unplugged | Reopen attempts every 2 s (exists); profile and grades follow the capability report |
| Tracker drops | Device state machine; calibration retained ([08 §10](08-tracking-and-sensor-fusion.md)) |
| Corrupt profile or content file | Reported and skipped; the rest loads |
| Lost data directory | Nothing to recover from: there is no cloud. Backup is copying the data directory while the engine is stopped; the README says so |
| Moving to a new PC | Copy the data directory and install the same or a newer version; rig calibration must be redone if the rig moved |

## 7. Storage efficiency

Retention defaults and per-athlete overrides, artefact pinning and usage reporting are in
[12 §8](12-data-and-progress.md). Additional lifecycle rule: before deleting `recording.mcap` of a
session that is the only evidence for a comparability class's continuity, the retention job keeps
it until continuity re-analysis has run or the user dismisses it.

## 8. Validation

Installer test on a clean, offline Windows machine; firewall-logged run showing no egress
([17 §6](17-security-privacy-and-safety.md)); kill-the-process and pull-the-plug tests for each
row of §6; migration round trips; a tracker with an unsupported protocol version is refused with
the stated reason.
