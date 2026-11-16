# ADR-021 — Pose estimation: top-down RTMPose with a feet-capable schema, on ONNX Runtime

| | |
|---|---|
| Status | **Pending Validation** — model availability, licence and inference rate are unconfirmed |
| Date | 17 November 2026 |
| Extends | ADR-009 in [docs/12](../../../12-decisions.md) |
| Specified in | [08 §3–4](../08-tracking-and-sensor-fusion.md), [04 §2.2](../04-movement-domain-model.md) |

## Context

ADR-009 chose RTMPose on ONNX Runtime through `ort`, with a permissively licensed detector, and
left it unbuilt. Local model files exist (`yolox-m-humanart.onnx`, `rtmpose-m.onnx`); nothing loads
them. Kick analysis needs heel and toe points, which the COCO-17 keypoint set lacks. One athlete is
tracked.

## Decision

1. **Top-down**: a person detector (YOLOX) every N frames and on loss; between detections the crop
   follows the previous keypoints.
2. **RTMPose with Halpe-26 output** as the recommended pose model. A COCO-17 model is supported
   and yields no foot observables.
3. **ONNX Runtime via `ort`**, execution providers in order CUDA, DirectML, CPU; one inference
   thread batching crops across cameras.
4. The engine consumes 2D landmarks in the detector's own schema and converts through a **schema
   map** ([ADR-019](ADR-019-canonical-movement-representation.md)). The model identifier is part of
   `tracking_config_id` and of the comparability class.
5. Models are **user-supplied local files** until their terms are reviewed (ADR-009's open point).
6. Monocular 3D lifting is an **extension point** that qualifies for no metric grade.
7. Python is acceptable only offline, for model export.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| MediaPipe Pose / BlazePose (33 points including feet) | Not chosen: its runtime is a separate framework to embed; its "world landmarks" are model-relative, not metric. A schema map for it can be added if an ONNX export is used |
| Ultralytics YOLO-pose | Rejected (existing): AGPL-3.0 |
| Whole-body models (133 points) | Rejected for now: hands and face are out of scope; slower |
| Bottom-up multi-person models | Rejected: one athlete; top-down gives higher resolution on the subject |
| Lightweight single-pose models (MoveNet class) | Not chosen: COCO-17 only |
| A 3D lifter as the primary source | Rejected: inferred depth presented as measurement |
| Python inference sidecar | Rejected: runtime dependency |
| Train a martial-arts-specific pose model | Rejected: no dataset; out of scope |

## Rationale

The model must give feet, run within the frame budget on one GPU beside other work, and carry a
licence that lets the project stay MIT. The architecture limits its own exposure to the choice: a
detector is replaceable by adding one schema-map table, and results from different detectors are
never merged.

## Advantages

- No detector name appears above the adapter.
- Changing the model is safe for stored history (new comparability class).

## Disadvantages

- Detector landmarks are annotation conventions, not joint centres; the offset is method-specific.
- Changing the model fragments series ([19 §1](../19-risks-and-open-questions.md) R29).

## Risks

A Halpe-26 export under acceptable terms may not be obtainable (R23, O-02). ONNX Runtime with CUDA
needs matching driver versions (existing R7). Dataset terms of pretrained weights (existing R8).

## Decision criteria

Accepted as written when: the model's output shape is confirmed as 26 keypoints in the documented
order; its terms are reviewed; inference for four 60 fps cameras fits the budget of
[15 §2.1](../15-performance-and-latency.md). If no Halpe-26 model qualifies, COCO-17 ships as the
default and foot-dependent observables stay unavailable until one does.

## Consequences

Subject selection rules live in the camera adapter. `processing_profiles` in the engine
configuration name model paths and schema. The simulator gains a feet-capable landmark schema so
that foot observables are testable before a real model is settled.

## Reconsider when

- Blur around strikes makes detector landmarks unusable for a P1 metric (V-30): evaluate higher
  frame-rate cameras first, a different model second.
- A reviewed, permissively licensed model with better limb accuracy at speed appears.
