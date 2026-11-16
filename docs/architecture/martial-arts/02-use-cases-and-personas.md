# 02 — Use cases and personas

This document fixes who the product serves and which end-to-end journeys the architecture must
carry. Every later document traces back to a use case id here.

## 1. Personas

| Id | Persona | Equipment | Needs | Must never happen |
|---|---|---|---|---|
| P1 | **Solo beginner** — trains at home, no instructor present | One webcam | Repetition counts, pacing, simple cues, a visible record of practice | A confident correction based on a view that cannot see the fault |
| P2 | **Committed practitioner** — several years of training, wants measurable progress | Two to four cameras, later printed tags and IMUs | Kick height, timing, left/right comparison, month-over-month trends | Progress that is really a change of camera position |
| P3 | **Instructor** — records demonstrations, prepares drills for students | Same as P2 | Record a reference, define combinations and workouts, review a student's attempts | The system presenting its reference as the only correct technique |
| P4 | **Flexibility-focused athlete** | One or more cameras | Range of motion per side, hold timers, baseline and trend | Encouragement to exceed a comfortable range; a medical-sounding statement |
| P5 | **VR trainee** | PC VR headset plus any of the above | Dojo, mirror, ghost, targets, the same workouts as on desktop | Being led to strike toward a wall or outside the play space |
| P6 | **Developer / validator** | Recorded sessions, a reference measurement system | Replay, re-analysis, error metrics | Results that cannot be reproduced from a recording |

One installation serves several athletes (P1–P5 may share a machine). The active athlete is engine
state ([12 §1](12-data-and-progress.md)).

## 2. Use cases

Priority: **P0** essential foundation, **P1** high value, **P2** later.

| Id | Use case | Personas | Mode | Priority | Minimum capability profile ([08 §9](08-tracking-and-sensor-fusion.md)) |
|---|---|---|---|---|---|
| UC-01 | Count repetitions of a selected kick and time each phase | P1, P2 | Guided technique | P0 | `basic` |
| UC-02 | Receive at most one correction per repetition, tied to an observed measurement | P1, P2 | Guided technique | P0 | `basic` |
| UC-03 | Save a session and compare valid measurements with earlier sessions | P1, P2 | Any | P0 | `basic` |
| UC-04 | Review one attempt: timeline, phase bands, joint-angle curves | P2, P3 | Guided technique | P0 | `basic` |
| UC-05 | Measure controlled kick height against the calibrated floor | P2 | Guided technique, Assessment | P1 | `multi_view` |
| UC-06 | Practise a combination with timing windows and step-by-step results | P1, P2 | Combination | P1 | `basic` |
| UC-07 | Run a timed workout with warm-up, sets, rest and cool-down | P1, P2 | Workout | P1 | `basic` |
| UC-08 | Build and save a workout in the desktop UI | P2, P3 | — | P1 | none |
| UC-09 | Run a benchmark whose result is comparable with the previous run | P2, P4 | Assessment | P1 | Declared per assessment |
| UC-10 | Train freely; recognisable techniques are labelled, the rest is reported as unknown | P2 | Free | P1 | `basic` |
| UC-11 | Hold a stretch with a timer and see range per side against the personal baseline | P4 | Mobility | P1 | `basic` in the plane of the camera; `multi_view` otherwise |
| UC-12 | Record a reference demonstration and use it as a ghost | P3, P2 | Guided technique | P1 | `multi_view` for a 3D ghost |
| UC-13 | Add printed tags to forearms and shins and gain segment-rotation observables | P2 | Any | P2 | `marker_assisted` |
| UC-14 | Add IMUs and keep timing metrics through camera occlusion | P2 | Any | P2 | `hybrid` |
| UC-15 | Run a saved workout in VR with mirror, ghost and targets | P5 | Any | P1 | `multi_view` or `hybrid` |
| UC-16 | Practise a kata or poomsae step by step, then perform it in full with per-step feedback | P2, P3 | Form | P2 | `multi_view` for facing and travel |
| UC-17 | Follow a multi-week program whose next session adapts to the last | P1, P2 | Workout | P2 | `basic` |
| UC-18 | Re-analyse an old recording with a newer algorithm or definition | P6 | — | P1 | Recording with inputs |
| UC-19 | Export or delete all data of one athlete | All | — | P1 | none |
| UC-20 | Calibrate cameras, floor and wearable tags through a wizard | P2 | — | P1 | — |

## 3. Canonical journey (UC-01 to UC-04)

This is the journey of the first vertical slice; its full data flow is in
[18 §4](18-implementation-roadmap.md).

1. The athlete opens **Train**, picks Taekwondo, *Roundhouse kick*, right leg, 3 × 10.
2. The engine reports readiness: tracking profile, which metrics will be available and why others
   are not, calibration state, space check.
3. A countdown starts the session. The athlete takes the start stance; the first repetition is
   armed when the stance is recognised.
4. Each kick is segmented into phases; on completion the repetition counter increments and the
   selected live metrics update.
5. After each repetition at most one cue is shown; after each set a short summary.
6. After the last set the session is saved. The athlete opens any attempt in **Technique** and
   sees phase bands, curves and, where available, the reference.
7. **Progress** shows today's valid measurements next to earlier sessions measured the same way.

## 4. Scope boundaries per persona

| Question | Decision |
|---|---|
| Is the instructor a separate role with permissions? | No. An instructor is an athlete profile that authors content and records references. There is no access control between local profiles |
| Can two athletes train at the same time? | No. One body is tracked; a second person in view is ignored by selection rules in [08 §3](08-tracking-and-sensor-fusion.md) |
| Does the product prescribe training loads? | It executes the programs it is given and applies the declared progression rules; it does not generate programs |
| Does the product assess readiness to train or injury? | No ([17](17-security-privacy-and-safety.md)) |
