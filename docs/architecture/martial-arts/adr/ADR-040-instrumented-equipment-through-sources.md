# ADR-040 — Instrumented equipment enters through the existing source and observation contract; nothing is built ahead of a device

| | |
|---|---|
| Status | **Proposed** — an extension contract; no implementation is planned until a device exists |
| Date | 17 November 2026 |
| Extends | [ADR-022](ADR-022-sensor-ingestion.md) |
| Specified in | [30](../30-instrumented-equipment.md), [27](../27-reaction-and-decision-drills.md) |

## Context

Impact force, contact and contact quality are not observable from kinematics. Pads, bags and
pressure or contact sensors could supply them. The brief asks to prepare for this without building
speculative interfaces.

## Decision

1. Equipment is an ordinary **registry device** (`equipment`) fed by an ordinary **source
   adapter**; its readings are ordinary **observations** on the session clock, recorded and
   replayable.
2. Reserved additions, to be made **when the first device is integrated and not before**: device
   kind `equipment`; observation `impact { peak, unit, duration_ms, zone }` beside the existing
   `contact`; calibration kinds `equipment_pose` and `equipment_response`; method class
   `equipment`.
3. Magnitudes are **`relative` to the device** unless a response profile calibrated against a load
   reference gives newtons with a stated range.
4. Equipment-derived metrics are **separate metrics**; they are never merged with kinematic ones
   into a composite.
5. A target may be bound to equipment; contact then decides `target_reached`.
6. No abstraction layer, trait or service is added now. The same rule covers reaction-drill
   stimuli: their event pair is documented and unbuilt.

## Alternatives evaluated

| Alternative | Verdict |
|---|---|
| Estimate force from kinematics (effective mass × velocity change) | Rejected: effective mass and contact are unobserved; the result would be an invented number with a unit |
| A generic "equipment plugin" interface now | Rejected: no device to design against; would be wrong in detail |
| A dedicated equipment service or protocol | Rejected: the source model already covers it |
| Treat a bag accelerometer reading as force | Rejected: it measures the target's response, dependent on its mass and mounting |
| Support commercial smart-bag protocols first | Rejected: closed protocols; only through documented interfaces |

## Rationale

The source model was designed so that a new sensor is one adapter. Proving that claim with a
written contract costs nothing; building for an unseen device would cost design mistakes.

## Advantages

- A future device needs an adapter, two enum values and metric definitions.
- Contact gives an independent check of the kinematic strike-time estimate.

## Disadvantages

- Nothing can be claimed about equipment until one is integrated.
- Per-device calibration burden.

## Risks

The reserved observation shape may not fit the first real device; it is a starting point and is
revised then.

## Consequences

The feasibility matrix lists force and contact as "needs instrumentation" with this record as the
path.

## Reconsider when

A specific instrumented pad or bag is on the bench: turn this record into an Accepted one with the
actual observation shape and calibration procedure.
