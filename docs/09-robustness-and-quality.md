# 09 — Robustness and quality

State definitions are in [03 §4](03-domain-model.md). This document says how the system behaves
when things go wrong, and the engineering rules that keep it that way.

## 1. Principles

1. **Failures are local.** A source runs in its own task with its own queue. Its failure changes
   its devices' state and nothing else.
2. **Loss of input becomes a labelled gap.** Never a held value presented as current, never an
   interpolation presented as measured.
3. **Every non-nominal state has a reason** that a user can read and, where possible, an action.
4. **The engine keeps serving.** With every device gone the API still answers and reports why
   there is nothing to show.
5. **Nothing waits on a consumer.** A slow client, a slow disk or a closed socket cannot delay
   tracking.

## 2. Failure and recovery model

| Failure | Detection | Immediate behaviour | Recovery |
|---|---|---|---|
| Tracker lost | No observation for `quality.disconnect_after_ms`; or link event from the adapter | Device → `disconnected`. Its segment goes stale after `engine.stale_after_ms`, then becomes `inferred` in parent-relative rest pose. Dependent joint angles → `null` with reason | Automatic when data resumes: `acquiring`, then `tracking` once rate and clock offset settle. Calibration is retained |
| Tracker degraded | Measured rate below `degraded_rate_fraction` of nominal, sequence gaps, or reported quality below `degraded_quality` | Device → `degraded`. Segment stays `derived` with reduced confidence | Automatic |
| Camera disconnected | Capture error or no frames | Camera → `disconnected`. Its landmark stream stops. Features that needed it re-evaluate | Adapter retries with backoff; automatic on return |
| Invalid or stale calibration | Load failure, schema mismatch, invalidation rule | Profile → `invalid` or `stale`. Dependent segments are not derived | User recaptures; the UI offers the action |
| Missing ONNX model (designed) | File check at source start | Camera source → `unavailable` with the path it looked for. Other sources unaffected | User supplies the model; source restarts on config reload |
| GPU unavailable (designed) | Execution provider fails to initialise | Falls back to the next configured provider if one is listed, with a `degraded` reason naming the fallback; otherwise `unavailable` | — |
| SlimeVR Server not running (designed) | Connection refused | Source → `disconnected`, reconnect with backoff | Automatic |
| Headset disconnected | XR session state in the Godot client | Client switches to its desktop rig and keeps showing the body stream | Automatic on session resume |
| Timestamps out of sync | Clock offset estimate unstable, or device time jumps (reboot) | Offset estimator resets; device → `acquiring` until it settles. Observations with implausible times are rejected and counted | Automatic |
| Low-confidence tracking | Confidence below threshold | Values keep their provenance, with reduced confidence; system state → `degraded` | — |
| Recording path unavailable | Write error or unwritable directory | `recording` feature → `unavailable`; an active recording stops and finalises what it can; a `notice` is sent | User fixes the path |
| Disk slower than the data rate | Recorder queue full | Events dropped and counted in `dropped_events` | — |
| Invalid configuration | Validation at startup | Engine does not start; all errors printed with paths | User edits the file |
| API port in use | Bind error | Standalone engine exits with a clear message. The desktop shell attaches to the engine already running on that address | — |
| A source task fails | Task returns an error or panics | Its devices → `unavailable` with the error message; a supervisor restarts the source with capped backoff | Automatic; after repeated failures it stays `unavailable` |
| Engine unreachable from a client | Socket close or connect failure | Client shows "Engine disconnected", clears live values, retries with backoff | Automatic; client resynchronises from `hello` |
| Protocol mismatch | `protocol_version` in `hello` | Client shows an incompatibility message and renders nothing | User updates the older side |
| Malformed observation | Validation: non-finite numbers, non-unit quaternion, unknown device | Rejected, counted, logged at a limited rate | — |

## 3. Input validation

Validation happens once, at the boundary where data enters the engine:

- **Sources** validate every observation before it reaches the pipeline: finite numbers, unit
  quaternions within tolerance, plausible timestamps, known device, declared capability.
- **The API** validates path parameters against identifier patterns, bodies against their types,
  values against ranges, and the request origin against the allow-list.
- **Configuration and persisted files** are validated on load with their `schema_version`.
- **Clients** validate `protocol_version` and tolerate unknown fields and enum values.

Internal stages trust their inputs. Validation is not repeated between modules.

## 4. Error handling rules

- Each module has a typed error enum. Errors carry the context needed to act on them (which
  device, which file, which field).
- Recoverable runtime paths do not `unwrap` or `expect`. Where an invariant genuinely cannot fail,
  the `expect` message states the invariant.
- An error is handled or propagated, never discarded. Dropping data on a full queue is a counted,
  visible event, not a silent one.
- User-facing messages are plain language. Detail goes to the log.
- No placeholder returns a fabricated success. An unimplemented capability is reported as
  `unavailable` with reason `not_implemented`.

## 5. Logging

- `tracing` with structured fields: `source`, `device`, `session`, `profile`.
- Levels: `error` for failures needing attention, `warn` for degraded operation, `info` for
  lifecycle (start, stop, device connect/disconnect, calibration, recording), `debug` for
  diagnostics. Nothing per-observation or per-frame above `trace`.
- Repeating conditions (a rejected stream, a full queue) are rate-limited.
- Output format is `pretty` or `json`, set in configuration.

## 6. Performance rules

- Separate rates per stage (see [02 §2](02-architecture.md)); no stage is clocked by another.
- All queues are bounded. Each has a stated overflow policy: drop newest and count (ingest,
  record), latest wins (frames), drop oldest (landmarks).
- Frames and landmark sets are shared with `Arc` between consumers and serialised once per
  message, not once per client.
- No serialisation between internal stages.
- Inference and file I/O never run on the async runtime's worker threads.
- Camera frames are not copied between capture and inference beyond what the capture API
  requires; stale frames are dropped before inference.
- In the UI, the render loop reads the latest frame from a plain store and never goes through
  React state; per-frame allocations in the render loop are avoided.
- Optimisation follows measurement. There are no latency, accuracy or throughput figures in this
  project until they have been measured on the target hardware, and no number is quoted from an
  estimate.

## 7. Constants

Thresholds that describe behaviour (staleness, disconnect timeout, degraded limits, publish rate)
are configuration. Numerical constants that describe the model (proportion ratios, capture window,
stillness threshold, tolerances) are named constants grouped by module. There are no bare numbers
in operational code.
