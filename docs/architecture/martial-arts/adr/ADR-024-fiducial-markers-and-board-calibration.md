# ADR-024 — AprilTag clusters on rigid carriers; AprilTag board calibration; own solver

| | |
|---|---|
| Status | **Pending Validation** — the binding crate and the size/distance rules are unconfirmed; the principles below are accepted |
| Date | 17 November 2026 |
| Extends | ADR-014, ADR-015, ADR-016 in [docs/12](../../../12-decisions.md) |
| Specified in | [09](../09-optical-markers-and-calibration.md), [21](../21-wearable-hardware.md) |

## Revision (17 November 2026): the existing ChArUco script is the calibration path for now

Commits `0397304` and `61950cf` delivered `scripts/calibrate_cameras.py` (ChArUco board, Python
with OpenCV, run by the operator) and `POST /calibration/profiles`, through which it imports
intrinsics, extrinsics and the floor origin. This record's item 5 is therefore **re-scoped**:

- The import endpoint is the stable contract. Calibration tools are replaceable behind it.
- The ChArUco script is kept as the calibration path through roadmap Phase 4. It satisfies the
  "Python only offline" rule: the engine never runs it.
- The in-engine AprilTag board procedure and own solver are built **only with body clusters**
  (roadmap package 6a), when the AprilTag detector exists anyway and one print system for board
  and clusters has value, or earlier if a calibration wizard for non-developers is wanted.
- The calibration check of [09 §7.4](../09-optical-markers-and-calibration.md) and the QA record
  apply to profiles from either tool.

Items 1–4 and 6 (clusters) are unchanged.

## Context

The owner intends to 3D-print wearable holders and attach inexpensive printed tags. Real cameras
also need a calibration procedure, which does not exist (ADR-015 left it open). Both problems need
a fiducial detector. The project has no OpenCV dependency and avoids copyleft code.

## Decision

1. **AprilTag `tag36h11`** for body clusters and the calibration board; `tag25h9` allowed per
   cluster for 720p rigs.
2. Tags are worn as **rigid multi-face clusters on flexible bands**, never as flat tags on fabric.
   A cluster is a registry device assigned to a segment.
3. **Corners are recorded as input observations; poses are derived by the solver** and not recorded
   (ADR-016's principle).
4. A cluster yields a **carrier pose**; a segment rotation follows only through a calibrated
   `marker_mounting` transform and is `derived`. A cluster never overrides a joint landmark.
5. Cameras are calibrated with an **AprilTag grid board** using the same detector; intrinsics,
   extrinsics and floor are solved by a **small own solver** on `nalgebra` (Zhang initialisation,
   damped Gauss–Newton refinement).
6. The detector sits behind a `FiducialDetector` trait: the AprilTag 3 C library through a binding,
   or a pure-Rust port.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| ArUco / ChArUco via OpenCV | Rejected: adds OpenCV for no functional gain over AprilTag ([ADR-020](ADR-020-camera-processing-stack.md)) |
| Single flat tag per segment | Rejected: planar pose ambiguity, lost at oblique angles, bends on a limb |
| Tags on wrists, ankles, hands | Rejected by arithmetic: too small to decode at room scale with 1080p cameras |
| Retro-reflective or active LED markers | Out of scope: different camera hardware |
| Calibration from the athlete's body (wand-less, landmark-based) | Rejected as primary: detector landmarks are too noisy to define scale; kept as a consistency check |
| Checkerboard | Rejected: needs full visibility and a separate detector |
| Recording solved cluster poses | Rejected: could not be re-solved after a calibration or algorithm change |

## Rationale

One detector serves calibration and tracking. Rigid multi-face geometry removes the planar
ambiguity and keeps at least one face towards a camera. Treating the carrier, not the bone, as the
measured object keeps the claim honest.

## Advantages

- Segment twist becomes observable without electronics.
- An absolute heading reference for IMUs on the same segment.
- Calibration needs a sheet of paper and a rigid panel.

## Disadvantages

- Tags do not survive motion blur at striking speed; they help slow and medium work.
- Carriers add bulk; mounting must be recalibrated each time they are put on.
- An own calibration solver must be written and tested.

## Risks

Binding licence or Windows build ([19 §3](../19-risks-and-open-questions.md) O-06); family facts
not re-verified (O-05); soft-tissue motion (R35).

## Decision criteria

Binding accepted when it builds on Windows 11 under an MIT-compatible licence and detects the
board at the camera frame rate within the budget of [15 §2.1](../15-performance-and-latency.md).
Size rules accepted or revised from V-05 and V-06; default tag sizes follow the measured envelope.

## Consequences

New calibration kinds `marker_geometry`, `marker_mounting`; new observation kind `fiducials`; a
print endpoint for boards and clusters; the simulator gains a fiducial projector.

## Reconsider when

- V-06 shows tags unusable even at slow-practice speeds with the owner's cameras.
- A camera class with global shutter and strobed illumination becomes available (retro-reflective
  markers would then outperform printed tags).
