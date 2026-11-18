# Phase 2 — owner checklist (one webcam, profile `basic`)

The steps of martial-arts roadmap [18 §3 Phase 2](../architecture/martial-arts/18-implementation-roadmap.md)
that need a camera, real people and a phone. Everything the engine can check by itself is done
and recorded in [docs/13](../13-implementation-status.md) "Phase 2". Commands run from the
repository root in PowerShell on the Windows PC; `bansayo-engine` means
`F:\drusniel-cache\cargo-target\release\bansayo-engine.exe` (or `cargo run --release -p bansayo-engine --`).

What you need: the webcam; a phone that films at 240 fps on a tripod; a printed or written consent
note per participant; ≥ 3 participants; a clear floor of about 3 × 3 m; optionally an LED driven
by a microcontroller for L-01.

## A. Set up and check the camera

1. **Build.** `git pull`, then `python scripts/setup.py` (models, ONNX Runtime, desktop, engine).
   Expected: every step `ok`; `models/rtmpose-m.onnx` and `models/yolox-m-humanart.onnx` present.
2. **List the camera.** `bansayo-engine --list-cameras`. Expected: one line `0<TAB><name>`
   followed by its formats, including `MJPEG 1280x720 @ 30, ...`. A camera offering 60 fps or more
   at 1280×720 is better (phase timing becomes "estimated" instead of "indicative"); set
   `rate_hz` to it in step 3.
3. **Write the configuration.** `python scripts/setup.py cameras`. Expected:
   `config/local-cameras.yaml` (git-ignored) with one camera, `rate_hz: 30, exposure_s: 0.004`.
   For timing measurements add `diagnostics: { timing: true }` at the top level.
4. **Place the camera** ([08 §3.1](../architecture/martial-arts/08-tracking-and-sensor-fusion.md)):
   rigid tripod or wall mount, 1.0–1.8 m high, 3–4 m from the kicking spot, the whole body in
   view from head to feet including the raised foot. **Lateral placement**: the athlete stands
   side-on to the camera and kicks towards a target across the image. Note the view in the report;
   the engine records the view it classified (`lateral`, `frontal`, `oblique`) with each reference.
5. **Start and check.** `bansayo-engine --config config/local-cameras.yaml`, then in another
   window `curl.exe -s http://127.0.0.1:7700/api/v1/state`. Expected for the camera device:
   `state: tracking` (with someone in view), `stream.measured_rate_hz` within 5 % of `rate_hz`,
   `camera_settings.exposure_s` ≈ 0.0039 and `camera_settings.problems: []`. A problem names the
   setting the camera refused (`exposure_s` or `gain`); keep it in the report.
6. **Check the exposure.** Open `http://127.0.0.1:7700/api/v1/cameras/CAM1/preview.jpg` in a
   browser. If the image is too dark, raise gain first, then exposure:
   `curl.exe -X PUT -H "Content-Type: application/json" -d '{\"gain\": 64}' http://127.0.0.1:7700/api/v1/cameras/CAM1/settings`
   (gain is in the camera's own units; the answer shows what the camera took). Keep exposure at or
   below 0.004 s for kicks. The camera stays at manual exposure after the engine stops; unplug and
   replug it (or use its own app) to return it to automatic for other uses.
7. **Unplug test.** Start a session (section B, steps 9–10 with yourself), unplug the camera after
   the first counted kick. Expected: "Tracking lost" after about 1 s, the session `paused` about
   3 s later; replug: the engine reopens the camera within about 2 s and the session continues by
   itself. (Machine-tested with a removed image folder; this checks the real driver.)

## B. Record the takes (V-19, V-20)

8. **Consent.** Before the first take, each participant reads and signs your consent note
   (what is recorded: engine landmark recordings and a phone video; where it is kept: this PC only;
   that they can ask for deletion). In the desktop, create one athlete per participant with a
   pseudonymous name (P1, P2, P3) and tick motion-data consent; recordings are only made with it.
   Keep the signed notes outside the repository.
9. **Phone.** Tripod next to the webcam, 240 fps, the athlete's whole body and feet in frame.
   Start the phone recording before the session starts.
10. **Session.** In the desktop: Train → athlete → roundhouse kick → side → 3 sets × 10, rest 30 s.
    Acknowledge the safety notes, start. Before the first kick of each set, and after the last,
    **clap once with both hands above the head, facing the webcam**, holding still for a second
    before the first clap. Kick at the athlete's normal pace.
11. Repeat for the other side, and for every participant: **≥ 30 kicks per side per person,
    ≥ 3 people**. Write down each session id (Train summary, or
    `curl.exe -s http://127.0.0.1:7700/api/v1/training/sessions`).
12. **Promote every take** into the real golden set (never into the repository):
    `bansayo-engine promote --data ./data --session <id> --golden ./data/golden --rig one-webcam-30`.
    Expected: `Promoted <id> to ./data/golden; annotate it, then run regress --bless`.

## C. Annotate

13. **Clap times.** `bansayo-engine claps ./data/sessions/<id>`. Expected: one session time (µs)
    per clap, in order. If none is printed, the hands were not seen together above the shoulders;
    use the LED (L-01) or annotate the clap frame in a slow-motion view of the engine's camera
    preview instead and note it.
14. **Mark the video.** In a frame-accurate player, note the phone frame of the first clap, and per
    kick the frames where each phase starts and ends: chamber (knee starts rising), pivot (support
    foot turns, if visible), extension (lower leg starts to extend), retraction (leg starts to
    return), recovery (kicking foot lands or the leg is back in stance), plus the strike instant.
    Write `marks.csv` as documented at the top of `scripts/annotate_reps.py`. A second annotator
    marks at least one take independently (inter-annotator difference for V-20).
15. **Convert.**
    `python scripts/annotate_reps.py marks.csv --session <id> --fps 240 --sync-frame <clap frame> --sync-us <clap time from step 13> --annotator "<name>" --side right --out ./data/golden/sessions/<id>/annotations.json`.
    Expected: `<n> repetitions written`.

## D. Evaluate and report

16. In `./data/golden/manifest.yaml`, every promoted session has `annotations: required` and
    `labels: { precision_min: 0.95, recall_min: 0.95 }`. Add under each `expect:`
    `phase_boundary_ms: { mean_abs_max: 70 }` (two input periods at 30 fps).
17. `bansayo-engine regress ./data/golden/manifest.yaml`. Each session prints a `labels` and a
    `phase_boundary_ms` line with precision, recall, counts, count error per 20 and phase-start
    error (mean, SD, max). Failing lines are results, not errors: report them.
18. Fill [templates/V-19.md](templates/V-19.md) and [templates/V-20.md](templates/V-20.md) as
    `docs/validation/YYYY-MM-DD-V-19.md` and `…-V-20.md`. Run
    `bansayo-engine regress ./data/golden/manifest.yaml --bless` once the figures are recorded, so
    later algorithm changes are checked against these takes.

## E. Latency (L-01, L-03, L-06)

19. **L-01** with the LED rig as in [templates/L-01.md](templates/L-01.md); without one, the
    alternative described there gives an upper bound only, and says so.
20. **L-03**: phone at 240 fps films a hand clap in front of the webcam and the monitor showing
    the desktop live view, ≥ 20 times, with `diagnostics: { timing: true }`; fill
    [templates/L-03.md](templates/L-03.md).
21. **L-06**: from the V-19 takes' events, engine part (`rep_completed.t_us − repetition.t_end_us`);
    client part as in L-03 with the cue banner in view; fill [templates/L-06.md](templates/L-06.md).
22. **Stage budgets on real kicks**: with `diagnostics: { timing: true }` during one session, copy
    the shutdown log lines `stage timing over the last 10 s` into the V-19 report; with a subject
    followed, inference should be far below the 22 ms p95 measured with nobody in view.

## F. Hand back

23. Commit the reports (`docs/validation/*.md`, never takes or videos), update the Phase 2 rows of
    docs/13 that say "pending owner" with the report ids, and push.
