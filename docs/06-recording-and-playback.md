# 06 — Recording and playback

## 1. What is recorded

A session records the engine's **inputs**, not its outputs:

- every `SourceEvent`: device announcements, link changes and observations, exactly as accepted
  by the pipeline;
- the **context** needed to interpret them: skeleton definition, assignments, calibration
  profiles and the relevant configuration, as they were when recording started, plus any change
  made during the session.

Playback feeds those events back through the same pipeline. This is what makes a recording an
input adapter rather than a video of the result:

- a session can be re-solved with a better algorithm or a corrected calibration;
- the desktop and VR clients need no playback code, because replayed frames arrive on the same
  stream;
- recorded data is the regression corpus for the solver.

Solved frames are not stored in the PoC. Recording them as an additional channel, for export and
for external viewers, is an additive change.

## 2. Session layout

```text
<data_dir>/sessions/20261116T102205Z-a3f9/
├── recording.mcap
└── session.json
```

`session.json` is the `RecordingSession` object from [04](04-contracts.md) plus `schema_version`.
The session list is produced by reading these files. SQLite is introduced when a query appears
that this cannot answer, such as searching measurements across sessions (ADR-005).

### MCAP structure

| Element | Content |
|---|---|
| Channel `/bansayo/devices` | `device_announced` and `device_link` events, JSON |
| Channel `/bansayo/obs/<device_id>` | `SensorObservation`, JSON, one channel per device |
| Channel `/bansayo/context` | Context changes during the session (assignment or calibration change), JSON |
| Message `log_time` | Session time at which the engine accepted the event |
| Message `publish_time` | The observation's capture time `t_us` |
| Attachment `context.json` | Skeleton definition, assignments, calibration profiles, engine and protocol versions at start |
| Metadata `bansayo.session` | Session id, name, UTC start, origins |

Channels use MCAP's `json` message encoding without a schema record. The file opens in standard
MCAP tools for inspection.

### Writing

- The pipeline runner hands events to the recorder through a bounded channel with `try_send`. If
  the disk cannot keep up, events are dropped, counted in `RecordingState.dropped_events`, and the
  UI shows it. Tracking is never delayed by recording.
- The recorder runs on its own thread and owns the file.
- Stopping, shutdown and engine exit all finalise the MCAP summary. A file left unfinalised by a
  crash is reported as a damaged session rather than hidden.
- If the data directory is not writable, the `recording` feature is `unavailable` with the reason,
  and `POST /recording/start` returns `unavailable`.

## 3. Playback

Starting playback switches the engine from `live` to `replay` mode:

1. Live sources are paused. Their devices leave the device list.
2. A fresh pipeline state is built from the recording's `context.json`. Current assignments and
   calibration are untouched and are restored when playback stops.
3. A `replay` source reads events and delivers them according to a **virtual clock** that the
   playback controls drive.
4. Frames carry the recorded `t_us` and the recorded origins. `SystemState.engine.mode` is
   `replay`, and `playback` describes position, speed and pause state.

| Control | Behaviour |
|---|---|
| Play, pause | Starts and stops the virtual clock |
| Speed | Scales the virtual clock, 0.1× to 4× |
| Seek | Sets the virtual clock; the observation buffer is rebuilt from the events preceding the target within the staleness window |
| Loop | Restarts at the end |
| Stop | Returns to live mode and restores the live pipeline state |
| Frame step | Designed: advance one tick while paused |

### Determinism

Solver ticks in replay are placed on the virtual clock at fixed intervals from the session start,
so they do not depend on scheduling. Two replays of the same session produce the same frame
sequence. A replay is not guaranteed to be bit-identical to what was shown live, because live tick
times depend on when the runtime scheduled them; the difference is bounded by one tick of
sample-and-hold. A test asserts replay-to-replay equality.

### Limits in the PoC

- A session is read fully into memory at playback start, with a size limit; a larger file is
  refused with a clear error. Streaming reads using the MCAP index replace this when sessions
  with camera data make it necessary.
- No video is recorded, because the PoC has no video.
- Recording and replay are mutually exclusive.

## 4. Designed extensions

| Capability | Approach |
|---|---|
| Synchronised camera video | Encoded video in separate files referenced from the MCAP, indexed by `t_us` |
| Session comparison | Two replay pipelines side by side; the wire protocol gains a stream identifier. Not designed in detail |
| Export | Solved frames to CSV (joint angles), BVH or glTF animation (pose), from a replay pass without real-time pacing |
| Session index | SQLite: sessions, tags, per-session measurement summaries |
| Annotations | Markers and labelled intervals stored in `session.json`, later in SQLite |
