# Bansayo for Android

The phone as a **camera** for the engine, a **live viewer** (pose, counts, phases, cues), a **coach**
console (athletes, sessions) and **settings** (pairing, engine devices, USB camera exposure). Decision record:
[ADR-042](../../docs/architecture/martial-arts/adr/ADR-042-android-client.md). Pose estimation runs
on the PC; the phone sends JPEG frames. Phone and PC must be on the **same Wi-Fi**; the connection
is not encrypted, so use a trusted home network.

Kotlin, platform views, CameraX 1.6.1, OkHttp 4.12.0; `minSdk` 26 (Android 8.0), `targetSdk` 36.

## Build

Needs the Android SDK (platform 36, build tools) and a JDK 17 or newer.

```bash
cd apps/android
./gradlew assembleDebug
```

The APK is `app/build/outputs/apk/debug/app-debug.apk`. Create `local.properties` with
`sdk.dir=...` if the SDK is not found (it is git-ignored).

## Install on a phone

1. On the phone: Settings → About phone → tap *Build number* seven times; then Developer options →
   *USB debugging* on.
2. Connect it by USB and run `adb install -r app/build/outputs/apk/debug/app-debug.apk`
   (`adb` is in the SDK's `platform-tools`). Without USB: copy the APK to the phone and open it,
   allowing installation from that source.

## Set up the engine for the phone

In the engine configuration (for example `config/local-cameras.yaml`):

```yaml
schema_version: 2
api:
  bind: 0.0.0.0:7700          # serve the home network
  lan: { enabled: true }       # needed for any non-loopback bind; requests need the token
sources:
  - id: cams
    kind: camera
    models_dir: ./models
    cameras:
      - { id: CAM1, device: 0, resolution_px: [1280, 720], rate_hz: 30, exposure_s: 0.004 }
      - { id: PHONE1, network: true, resolution_px: [1280, 720], rate_hz: 30 }   # the phone
```

Start the engine. Windows asks whether `bansayo-engine.exe` may use the network: allow
**private networks** only. The token is created in `<data_dir>/lan-token` on first start.

## Pair and use

1. Desktop → **Devices** → *Phone pairing* → **Show pairing code**.
2. Scan the QR code with the phone's camera app; it opens Bansayo's Settings already paired. Or
   open Settings (gear icon) and type the address, port and token shown there. Every screen links
   to Settings when the engine is not paired or not reachable.
3. **Camera**: hold the phone in landscape on a tripod, choose the slot (PHONE1) and *Start
   sending*. *Apply exposure* sets 1/250–1/1000 s with the chosen ISO (for kicks); the engine shows
   PHONE1 as `tracking` at about the slot's `rate_hz`.
4. **Live view**: pick a camera to see its preview with the engine's landmarks, and the session's
   state, counts and cues.
5. **Coach**: create athletes (tick consent to save sessions), make one active, create a session,
   acknowledge the safety notes, then Start / Pause / Resume / End. USB camera exposure and the
   engine's device list are in **Settings**.
6. **Train**: see the active athlete and session, open the live viewer, and browse saved
   desktop and VR sessions. A saved clinic shows the same graded set comparison as desktop Train.

## What was tested (18 November 2026)

On the Android emulator (API 36) against an engine in LAN mode on the same PC, reached through
the PC's LAN address with the token: pairing link → connect; camera streaming → PHONE1 `tracking`
at 29 Hz with pose inference; viewer → camera preview and session state; admin → athlete created
with consent and made active, session created, safety acknowledged, ended. **Not yet tested on a
physical phone**: real Wi-Fi bandwidth and latency, manual exposure on a real sensor, QR scanning
from the desktop screen.
