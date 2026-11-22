# Validation records

One Markdown file per experiment run (martial-arts [16 §10](../architecture/martial-arts/16-accuracy-and-validation.md)):
`YYYY-MM-DD-<id>.md`, for example `2026-12-03-V-19.md`. Copy the matching template from
[`templates/`](templates/), fill every field, and keep the takes it used.

| File | What |
|---|---|
| [PHASE2-OWNER-CHECKLIST.md](PHASE2-OWNER-CHECKLIST.md) | The roadmap Phase 2 steps that need the owner, a camera and real people |
| [2026-11-18-phase2-camera-bringup.md](2026-11-18-phase2-camera-bringup.md) | First physical webcam: formats, rate, manual exposure, stage timings; engine part of L-06 |
| [2026-11-22-phase01-openxr-uplink.md](2026-11-22-phase01-openxr-uplink.md) | Roadmap Phase 01 OpenXR uplink; headset checks BLOCKED (no hardware), sender ↔ engine machine check |
| [templates/V-19.md](templates/V-19.md) | Repetition counting per technique and profile |
| [templates/V-20.md](templates/V-20.md) | Phase boundaries |
| [templates/L-01.md](templates/L-01.md) | Camera exposure → arrival latency |
| [templates/L-03.md](templates/L-03.md) | Exposure → desktop pixels |
| [templates/L-06.md](templates/L-06.md) | Movement end → cue |

Rules that apply to every report:

- A figure is quoted only with this report's id, its operating range and its reference.
- Raw data are the takes' session directories (`recording.mcap`, `motion.mcap`), promoted into
  the real golden set with `bansayo-engine promote`; they stay on the validator's machine
  (`<data_dir>/golden/`) and are never committed, because they contain people's movement.
- Every participant's consent is recorded before the first take (owner checklist, step 4).
