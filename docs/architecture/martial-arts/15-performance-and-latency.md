# 15 — Performance and latency

Budgets for every path from a sensor to a pixel, an event or a stored row, and the way each budget
will be measured.

**Every number in this document is an engineering target.** None has been measured. The only
timing facts verified so far are the ones in [docs/13](../../13-implementation-status.md): on
Windows 11 the Tokio timer wakes about every 15.6 ms, which is why ticks come from a plain thread.

## 1. Principles

| # | Principle | Consequence |
|---|---|---|
| L1 | Measurement time and presentation time are different things | Every value carries `t_obs_us`, the capture time of its newest input ([04 §3](04-movement-domain-model.md)). Metrics are computed on capture times; latency never changes a stored number, only when it appears |
| L2 | Nothing is predicted in the engine | No extrapolation in the solver, the observables or the training runner ([08 §7.4](08-tracking-and-sensor-fusion.md)). A late value is late, not guessed |
| L3 | Slow consumers never slow tracking | Every channel is bounded with a stated overflow policy ([14 §3](14-runtime-and-api-contracts.md)) |
| L4 | Feedback during a strike is not attempted | Corrective cues are issued after the repetition ([07 §8.2](07-recognition-and-coaching.md)); the real-time channel carries only safety and tracking notices. The budget that matters for coaching is "movement end → cue", not "sensor → cue" |
| L5 | Budgets are per stage and instrumented | A stage that exceeds its budget is visible by name (§4) before anyone films a screen |

## 2. Latency budgets

All figures are p95 targets on the reference machine (the owner's Windows 11 PC with an RTX 4080;
the machine named in [docs/11](../../11-risks-and-validation.md)). "Frame period" is the camera's.

### 2.1 Camera path, per stage

| Stage | Budget at 60 fps | Notes |
|---|---|---|
| Mid-exposure → frame arrival in the capture thread | ≤ 25 ms | Sensor readout, USB transfer, driver. Unknown per camera; estimated by the `time_sync` procedure or `latency_ms` |
| Decode MJPEG 1280×720 | ≤ 4 ms | The threshold that triggers the `turbojpeg` fallback ([08 §3](08-tracking-and-sensor-fusion.md)) |
| Fiducial detection, full grey frame | ≤ 8 ms | Runs beside inference, not before it |
| Subject box + pose inference, batched over cameras | ≤ 12 ms | Batch window 4 ms; one inference thread |
| Wait for the next solver tick | ≤ 8.3 ms | Tick at 120 Hz on training rigs (`engine.publish_rate_hz`) |
| Time alignment (`triangulation.mode: aligned` only) | + up to 1 frame period of the slowest camera | [08 §4.3](08-tracking-and-sensor-fusion.md) |
| Solver stages 1–10 | ≤ 2 ms | Closed-form stages |
| Training runner: observables → recognition → metrics → exercise → coaching | ≤ 1 ms per tick; ≤ 5 ms on a tick that completes a repetition | Zero-phase re-filter and DTW over one repetition run on that tick; DTW over long clips goes to `spawn_blocking` |
| Serialise + WebSocket to loopback | ≤ 2 ms | Frames are serialised once and shared (existing) |

### 2.2 End to end

| Path | Target p95 | Composition |
|---|---|---|
| Camera exposure → `frame` published (`gated`) | ≤ 55 ms | Rows of §2.1 without alignment |
| Camera exposure → `frame` published (`aligned`) | ≤ 72 ms | + one 60 fps period |
| IMU sample → `frame` published | ≤ 20 ms | Wi-Fi UDP ≤ 10 ms, tick wait ≤ 8.3 ms, solver |
| Camera exposure → desktop pixels | ≤ 90 ms | + one 60 Hz display frame + compositor |
| Camera exposure → VR mirror figure | ≤ 125 ms | + the client's 50 ms interpolation buffer + one headset frame. This is the "roughly a tenth of a second" that rules out a first-person body ([11 §4.2](11-vr-architecture.md)) |
| Headset and hands in view | Runtime's own | Native XR tracking; the application adds nothing |
| Strike instant → `strike` event | ≤ 3 input periods + path latency | The parabola vertex needs one sample after the peak ([04 §7](04-movement-domain-model.md)) |
| Movement end → `rep_completed` with measurements, score and cue | ≤ 250 ms | Dominated by `segmenter.end_hold_ms` (150 ms); a closed-set machine that completes earlier publishes at once |
| Hazard condition → real-time `safety`/`tracking` notice | ≤ 100 ms after the condition holds | Engine side. Headset-side boundary responses do not go through the engine at all ([11 §7.2](11-vr-architecture.md)) |
| Command (HTTP) → new `training_state` on the stream | ≤ 30 ms | One actor round trip |
| `rep_completed` → row durable in SQLite | ≤ 1 s | Writer thread batches inside one transaction per event group |
| Attempt trace or clip request (3 s repetition) | ≤ 200 ms | Indexed MCAP read |
| Re-analysis of a recorded session | ≥ 10 × real time | Recorded inputs are landmarks and samples; no inference is re-run |

### 2.3 VR frame budget

| Item | Target |
|---|---|
| Application frame time | ≤ 11.1 ms at 90 Hz, ≤ 13.9 ms at 72 Hz, measured in the headset |
| Parse and apply one `frame` message in GDScript | ≤ 1 ms. If measured above 2 ms the reconsideration clause of existing ADR-010 applies |
| Degradation order when over budget | Ghost detail, mirror detail, HUD refresh; never the boundary rendering ([11 §7.2](11-vr-architecture.md)) |

## 3. Rates and throughput

| Stream | Rate | Why |
|---|---|---|
| Camera capture | ≥ 60 fps minimum for kick timing; ≥ 100 fps for grade-A speed metrics | A kick's extension lasts on the order of 100–200 ms ([08 §3.1](08-tracking-and-sensor-fusion.md)); at 30 fps that is 3–6 samples |
| IMU output to the engine | 200 Hz | [08 §5.2](08-tracking-and-sensor-fusion.md) |
| Solver tick and analysis | 120 Hz on training rigs; the simulator default is unchanged | Sample-and-hold per (device, kind); observables are `fresh` only when an input advanced |
| `frame` to clients | ≤ client `max_rate_hz` (existing limiter) | |
| `observables` to clients | ≤ 60 Hz, at most 32 ids | Gauges |
| `training_state` | On change + 1 Hz | |
| `motion.mcap` | Every tick, every catalogue observable, zstd | Size is a measured quantity in §4, not estimated here |

Different rates are reconciled by one rule, not by resampling to a common clock: each stage reads
the latest value per (device, kind) at the tick and keeps its `t_obs_us`; derivatives are taken
across distinct observation times ([04 §6.3](04-movement-domain-model.md)).

## 4. How the budgets are measured

### 4.1 Per-stage instrumentation (in the engine, always compiled, off by default)

`diagnostics.timing: true` makes every stage stamp the session clock on the data it handles:

```ts
interface StageTimings {            // carried beside each camera frame and each solver tick
  t_capture_us: TimeUs;             // arrival in the capture thread
  t_decoded_us: TimeUs;
  t_inferred_us: TimeUs;
  t_tick_us: TimeUs;
  t_solved_us: TimeUs;
  t_trained_us: TimeUs;
  t_published_us: TimeUs;
}
```

The engine keeps a fixed-size histogram per stage difference and publishes p50/p95/p99 and the
maximum over the last 10 s on the `diagnostics` stream topic
([14 §4](14-runtime-and-api-contracts.md)) and in the log at shutdown. Channel overflow counters
(existing `dropped` fields) are reported in the same message. A stage over budget for 5 s raises a
`notice` naming the stage.

### 4.2 End-to-end experiments

| Id | Quantity | Method | Result recorded |
|---|---|---|---|
| L-01 | Camera exposure → arrival (`latency_ms`, `time_sync.offset_us`) | An LED driven from a microcontroller that also reports the switching time over serial with a two-way clock exchange; the camera films the LED; the engine finds the first frame in which it is lit. Repeated at each exposure and frame rate | Per camera model: offset mean and jitter (SD) |
| L-02 | Cross-camera skew | The same LED seen by all cameras | Pairwise offsets; compared with the cross-correlation procedure ([16 §3](16-accuracy-and-validation.md), V-07) |
| L-03 | Exposure → desktop pixels | A 240 fps phone films the LED and the monitor showing the Studio viewport, in which a simulated landmark is replaced by an LED-brightness indicator | Frame count × 4.17 ms; resolution ± 4 ms |
| L-04 | Exposure → VR mirror | The same through one headset lens | As L-03 |
| L-05 | IMU sample → `frame` | A tracker tapped on a table in view of a calibrated camera: accelerometer spike time against the video frame of contact, then against the published frame | Offset and jitter per transport |
| L-06 | Movement end → cue | From `motion.mcap`: `t_us` of `activity_end` against the `t_published_us` of the cue event | Histogram per plan kind |
| L-07 | VR frame time and parse cost | Godot profiler in the headset on a full session | p95 frame time; parse time per message |
| L-08 | Sustained load | A 90-minute simulated session with four cameras, twelve trackers, two clients, all recorders on | Stage histograms, drop counters, memory growth, file sizes per minute |
| L-09 | Re-analysis speed | Wall time of re-analysing the L-08 recording | Multiple of real time |

L-01, L-02 and L-05 need a microcontroller with an LED; no other special equipment is assumed.
Results go into `docs/validation/` as described in [16 §10](16-accuracy-and-validation.md).

## 5. Fast movement: what the architecture does about it

| Problem | Handling | Where |
|---|---|---|
| A strike spans few camera frames | Rate gates on speed metrics (X below 50 Hz, at most B below 100 Hz); event times interpolated between samples; IMU angular velocity preferred for timing when present | [05 §10](05-technique-library.md), [04 §7](04-movement-domain-model.md) |
| Motion blur at the striking limb | Short manual exposure set by the engine; confidence-gated landmarks; tags expected to drop out and arbitration to fall back | [08 §3.2](08-tracking-and-sensor-fusion.md), [09 §4.3](09-optical-markers-and-calibration.md) |
| Unsynchronised cameras | Skew gate today; interpolation to a common instant, pending validation | [08 §4.3](08-tracking-and-sensor-fusion.md) |
| Timestamp skew between device clocks | Offset estimator per device clock; `time_quality` on every observation; timing sigma grows by 10 ms for `receive_time` | [08 §1.1](08-tracking-and-sensor-fusion.md) T-05 |
| Temporary occlusion | Anchored FK through IMU- or tag-rotated segments; `max_gap_ms` per technique; otherwise `rejected_quality` | [08 §7.3](08-tracking-and-sensor-fusion.md) |
| Filter lag on live values | Live low-pass for display; completed repetitions re-filtered zero-phase before metrics | [04 §6.3](04-movement-domain-model.md) |
| Rendering faster than data | Clients interpolate between received frames with a fixed delay; they do not extrapolate | [11 §8](11-vr-architecture.md) |
| Combination timing | Step intervals are differences of interpolated strike times on the session clock, independent of when events were delivered | [06 §4](06-exercise-and-workout-engine.md) |

## 6. Resource budgets (targets)

| Resource | Target | Action when exceeded |
|---|---|---|
| Engine CPU, four 720p60 cameras + twelve trackers | ≤ 4 cores at p95 | Lower `max_inference_hz`, then resolution; reported as `degraded` with the cause |
| GPU inference | ≤ 50 % of one RTX 4080 at 4 × 60 fps | Leaves headroom for the VR client on the same GPU; if the VR frame budget is missed the engine's `max_inference_hz` is the first control |
| Engine memory | Flat over L-08 (no growth beyond start-up + bounded buffers) | A defect |
| Solver history | Last four landmark sets per camera; one repetition's observable window (≤ 4 s × 120 Hz) per active recogniser | Fixed |
| Disk, `motion.mcap` | Measured in L-08 and shown per athlete by `GET /storage/usage` | Retention defaults ([12 §8](12-data-and-progress.md)) are revisited with the measured figure |
| Disk, `video.mcap` | Camera bitrate × duration; off by default | 14-day default retention |

## 7. Prediction versus measurement

| Layer | May interpolate | May extrapolate | Feeds a score |
|---|---|---|---|
| Solver, observables, metrics | Camera views to a common instant (flagged, widens sigma); event times between two samples | Never | Yes |
| Client display (desktop, VR) | Between received frames | No (not implemented; if added, client-side, visibly flagged) | Never |
| XR runtime head and hand prediction | — | Yes, the runtime's own, for the user's view only | Never. Poses ingested from XR devices are sent with the runtime's predicted **display** time as device time and are treated like any other observation at that time ([11 §6](11-vr-architecture.md)) |

## 8. Status and decision references

Nothing here is implemented except bounded channels, the ticker thread and the stream rate limiter.
The capture, decode and inference budgets decide two pending records:
[ADR-020](adr/ADR-020-camera-processing-stack.md) and
[ADR-021](adr/ADR-021-pose-estimation-integration.md). The alignment cost is weighed in
[ADR-023](adr/ADR-023-time-aligned-triangulation.md). The VR parse budget is the reconsideration
trigger of [ADR-033](adr/ADR-033-vr-runtime.md).
