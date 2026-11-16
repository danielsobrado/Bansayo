# 17 — Security, privacy and safety

What the platform protects, what it refuses to claim, and how it avoids putting an athlete at
physical risk. VR-specific space rules are in [11 §7](11-vr-architecture.md) and are not repeated.

Decision record: [ADR-031](adr/ADR-031-local-first-execution.md). Status: **Proposed**, except the
loopback bind, the origin allow-list and body validation of the existing API, which are
**Implemented**.

## 1. Position

Bansayo is a **training aid**. It is not a coach, a physiotherapist, a medical device, a grading
panel or a competition judge. Everything in this document follows from keeping that statement true
in the product's behaviour, not only in its text.

## 2. Security and privacy

### 2.1 Data inventory

| Data | Sensitivity | Where | Default |
|---|---|---|---|
| Camera video | High: identifiable images of a person at home | `sessions/<id>/video.mcap` | **Not recorded.** Needs `consent.video_utc` and an explicit per-session or per-athlete choice |
| Camera preview | High, transient | Memory; `GET /cameras/{id}/preview` | Served only while requested; never written |
| 2D landmarks, IMU samples, tag detections | Medium: movement data from which a person may be recognisable by gait or build | `recording.mcap` | Recorded with `consent.motion_data_utc` |
| Solved motion, observables, events | Medium | `motion.mcap`, SQLite | As above |
| Measurements, scores, progress | Medium | SQLite | As above |
| Athlete profile | Low by design: display name, height, sides, experience, goals | `athletes/<id>/profile.json` | No date of birth, sex, body mass, health data, contact details or photograph ([12 §1](12-data-and-progress.md)) |
| Reference clips | Medium; may leave the machine when shared | `references/<id>/` | Mandatory licence field; shipped clips need a written release ([07 §6.4](07-recognition-and-coaching.md)) |
| Calibration, configuration, content | Low | `calibration/`, `config/`, `content/` | — |

Body movement data is biometric in character. The design treats it as such: collected only with
recorded consent, kept only on the machine, minimised, exportable and erasable per athlete.

### 2.2 Trust boundaries and threats

| Boundary | Threat | Control | Status |
|---|---|---|---|
| Operating-system account | Another local user or process reads the data directory or calls the API | The OS account **is** the security boundary. The data directory is created under the user's application-data path with the platform's per-user permissions. No encryption at rest by the application; full-disk encryption is the user's control and is recommended in the README | By design |
| Local profiles | One athlete opens another's history | No access control between local profiles; stated in the athlete switcher. The product is not suitable, as designed, for a shared machine whose users must not see each other's data | By design; revisit only with a concrete need |
| Loopback API | A web page in the user's browser calls `127.0.0.1:7700` | Origin allow-list and content-type checks on every state-changing route (existing); no CORS wildcard; the preview stream checks the origin too | Implemented / extended |
| Network | Exposure when bound to a LAN address (standalone headset, second machine) | Refused unless `api.lan.enabled`; then a bearer token from `token_file` on every HTTP request and WebSocket upgrade, constant-time comparison, token shown once as a pairing code/QR in the desktop UI; TLS is **not** provided by the engine — the LAN mode is for a trusted home network and says so | Proposed, P2 |
| Ingest socket | A client injects false observations | Only configured `client_ingest` sources; the same validation as any source; in LAN mode the token; ingested devices are labelled by source in every evidence record | Proposed, P2 |
| Content files | A malicious or broken definition | Definitions are data over closed catalogues with no expression language ([13 §1](13-configuration-schemas.md)); size and nesting limits; unknown fields rejected; a bad file is reported and skipped | Proposed |
| Model files | A tampered ONNX model | Models are local user-supplied files loaded by path; the README states their source and expected SHA-256; the engine logs the hash it loaded and includes the model id in `tracking_config_id` | Proposed |
| Dependencies | Supply-chain compromise; copyleft code | Lockfiles pinned (existing); licence review rule of existing ADR-009/ADR-014 extended to the AprilTag binding and `ort` | Existing rule |
| Cloud | — | None exists: no account, telemetry, update check or remote call from engine or clients | By design |

Out of scope: defending against an attacker who already runs code as the user.

### 2.3 Consent, export, deletion

| Operation | Rule | Reference |
|---|---|---|
| Consent to store movement data | Explicit action per athlete; without it a session can run and nothing about it is stored | [12 §1](12-data-and-progress.md) |
| Consent to record video | Separate, explicit, withdrawable; the live view shows a recording indicator whenever any camera frame is being written | [12 §1](12-data-and-progress.md) |
| A second person in view | Only the selected athlete is tracked ([08 §3](08-tracking-and-sensor-fusion.md)); video, if on, records whoever is in frame, and the consent text says so | — |
| Minors | The product collects no age. The README states that a parent or guardian should set up and supervise use by a child | — |
| Retention | Per artefact type, with defaults that delete video first | [12 §8](12-data-and-progress.md) |
| Export | Everything about one athlete in documented formats | [12 §8](12-data-and-progress.md) |
| Erasure | Deleting an athlete removes rows, session directories, references and athlete-scoped calibration, then compacts the database; `secure_delete` is on | [12 §8](12-data-and-progress.md) |
| Logs | No landmarks, names or free text from profiles in logs; ids only | — |
| Sharing a session for debugging | An explicit export; never automatic | — |

## 3. Physical safety

### 3.1 Hazards and controls

| Hazard | Control in the design | Where |
|---|---|---|
| Kicking or striking a wall, furniture, a person or a pet | `safety.min_clear_radius_m` per technique; `space_check` gate (confirmation on desktop, blocking bounds check in VR); targets and ghosts only inside the zone; no interaction placed where reaching it leaves the zone | [03 §1](03-training-modes.md), [11 §7](11-vr-architecture.md) |
| Full-power strikes into empty air (hyperextension of knee or elbow) | Every striking drill states that there is no physical target and that techniques are to be controlled; no metric rewards maximum speed by default — speed is measured, not a scoring component of the shipped models; the slow stage precedes the normal stage in guided practice | [05 §5](05-technique-library.md), [07 §7](07-recognition-and-coaching.md) |
| Over-stretching | No prompt beyond the stored comfortable baseline plus the definition's `progression_step`; `max_hold_s`; mandatory `min_rest_s`; hold time counts only while stable; no visual that invites pushing further; the baseline is what the athlete reached under control, never a population norm | [03 §8](03-training-modes.md), [06 §9](06-exercise-and-workout-engine.md) |
| Training cold | `safety.warmup_required` forces a warm-up block in any workout that contains the technique | [06 §5.1](06-exercise-and-workout-engine.md) |
| Fatigue and excessive volume | Per-technique repetition and set caps; a per-session count of high-intensity strikes with notices; a session-duration notice; rest that cannot be skipped after holds. The limits are conservative defaults chosen by the project, **not** validated training-load prescriptions | [06 §9](06-exercise-and-workout-engine.md) |
| Inappropriate intensity for the athlete | Levels start at 1; progression rules move one step at a time and only on completion and the athlete's own effort rating; the engine proposes, the athlete starts | [06 §6](06-exercise-and-workout-engine.md) |
| Wrong corrective feedback | Evidence gate, persistence rule, one cue at a time, basis tag on every cue, no absolute statements, validation of cue precision before the detailed level is default | [07 §8](07-recognition-and-coaching.md), [16 §5](16-accuracy-and-validation.md) V-24 |
| Feedback from incomplete observation | Grades; suppression; `null` score components; "not assessed" is never shown as failure | [05 §8](05-technique-library.md) |
| Lost tracking mid-movement | Auto-pause after 3 s without required observables; no value held across a gap; in VR the headset-side monitor pauses on its own tracking loss without waiting for the engine | [06 §2](06-exercise-and-workout-engine.md), [11 §7.2](11-vr-architecture.md) |
| Wearables | Carrier rules: no rigid part over a joint, rounded edges, nothing on striking or landing surfaces, low mass; trackers strapped, not held | [09 §5.1](09-optical-markers-and-calibration.md) |
| Cables, controllers | Cable warning for turning techniques on a tethered headset; controllers put down or strapped before striking drills | [11 §6](11-vr-architecture.md), [11 §7.3](11-vr-architecture.md) |
| Distraction by the interface | During `in_rep` no control is active and no text changes except the phase indicator; audio is preferred over reading for counts | [10](10-desktop-architecture.md), [11 §5](11-vr-architecture.md) |

### 3.2 Safety fields of a definition

Every technique, hold and form carries a mandatory `safety` block
(`notes[]`, `min_clear_radius_m`, `max_reps_per_set`, `max_sets_per_session`, `warmup_required`;
holds add `max_hold_s`, `min_rest_s`, `progression_step`). The athlete acknowledges the notes once
per content version (`safety_ack` gate). User-authored definitions must fill the block; the
validator refuses a definition without it. A user definition created with `extends` inherits the
base definition's block and may only tighten it.

### 3.3 What the software cannot know

It cannot see the room beyond the cameras' view, cannot detect pain, dizziness or injury, cannot
tell an active stretch from a forced one ([03 §8](03-training-modes.md)), and cannot tell whether a
correction suits a particular body. The safety model therefore relies on conservative defaults and
on the athlete's judgement, and the product says so at first run and in every safety note.

## 4. Claims and limitations

### 4.1 Statements the product never makes

| Never claimed | Why |
|---|---|
| Impact force, power, energy, contact quality | No force or contact observation exists ([05 §11](05-technique-library.md)) |
| Muscle activation, effort, fatigue state | Not observable from kinematics |
| Centre of mass, centre of pressure, ground reaction | Only a labelled proxy is computed |
| Injury risk, readiness to train, diagnosis, rehabilitation progress | Medical claims; out of scope |
| "Correct" or "perfect" technique | Scores measure closeness to a selected target ([07 §7.1](07-recognition-and-coaching.md)) |
| Grades, belts, certification, competition scores | Not a judging system; federation criteria are not modelled |
| An accuracy figure without a validation report | [16 §10](16-accuracy-and-validation.md) |

### 4.2 Wording rules (enforced by review of the i18n files)

- "Reached", never "hit" or "impact", for targets.
- "Estimated" for propagated uncertainty; "indicative" for grade C.
- "Differs from the selected reference", never "wrong".
- "Not assessed", never a zero or a failure, for missing evidence.
- "Balance proxy", never "centre of mass".
- No comment on body shape, weight, pain or health in any cue.

### 4.3 First-run and recurring notices

| Notice | When |
|---|---|
| Training-aid statement, the limits of §3.3, advice to consult a qualified instructor for technique and a medical professional before starting vigorous exercise or when in pain | First run per athlete; available under Help |
| Technique safety notes | First use of each content version (`safety_ack`) |
| Clear-space confirmation | Every session (desktop); enforced bounds (VR) |
| Tracking-quality statement: what will and will not be measured | Every session, in readiness |
| Volume and duration notices | When limits are reached |

## 5. Failure handling

| Failure | Behaviour |
|---|---|
| Consent missing | Session runs unrecorded with a visible "not saved" state, or is refused with reason when the plan requires history (assessments, relative goals) |
| Token missing or wrong in LAN mode | HTTP 401 with a new, additive `ApiErrorCode` value `unauthorized` (the existing codes have no fitting member); the engine never falls back to open access |
| Non-loopback bind without `api.lan.enabled` | Configuration error at start ([13 §11](13-configuration-schemas.md)) |
| Deletion interrupted | Deletion is ordered rows-first inside a transaction, then files; on restart the engine removes session directories that have no row |
| Safety block missing in user content | Validation error; the definition is not offered |

## 6. Validation

| Check | Method |
|---|---|
| No network egress | Run the engine and both clients behind a firewall rule that logs outbound connections; expect none |
| Origin and token enforcement | Table tests on every state-changing route and on both sockets |
| Erasure | Property test: after deleting an athlete, no row, file or directory contains the id ([12 §9](12-data-and-progress.md)) |
| Limits | Table tests of every limit in [06 §9](06-exercise-and-workout-engine.md); replay tests asserting that prompts never exceed baseline + step |
| Wording | A lint over `content/i18n/*.yaml` for the banned words of §4.2 |
| VR space rules | The headset checks of [11 §11](11-vr-architecture.md) |
| Review | A qualified instructor per discipline reviews the shipped safety notes and limits before a definition is marked `reviewed` |
