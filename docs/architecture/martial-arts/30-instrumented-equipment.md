# 30 — Physical equipment and instrumented targets

Punching bags, kick pads, pressure and contact sensors. Camera tracking can describe a strike's
trajectory; it cannot establish that contact happened, how hard, or how well. A contact or impact
input would make those claims defensible.

Decision record: [ADR-040](adr/ADR-040-instrumented-equipment-through-sources.md).
Status: **extension contract only**, P2. Nothing is built, and nothing is to be built until a
specific instrumented device exists on the bench.

## 1. Capability card

| Item | Content |
|---|---|
| Use case and value | Contact time and order in combinations on a pad or bag; a relative impact measure for a given device; targets that are physical objects with known positions. Value: the first measurements in the platform that concern the *effect* of a strike |
| Existing support | `contact_sensor` and `range_sensor` device kinds and `contact` / `distance` observations reserved in the source model ([08 §2](08-tracking-and-sensor-fusion.md)); `esp32_udp` auxiliary channel (draft); trackables with tag clusters; engine-defined targets; evidence and grades |
| Needed now | Nothing in code. The contract of §3 is documented so that the first device fits without redesign |
| Dependencies | A physical instrumented pad or bag; firmware; per-device calibration; lab comparison against a reference for anything called force |
| Feasibility and limits | Contact detection is easy. Force is hard: a bag-mounted accelerometer gives the bag's acceleration, not the force on it; pressure pads saturate and depend on contact area. Without calibration against a load reference the honest unit is "relative, this device" |
| Priority | P2 optional extension |
| Smallest milestone | One contact-switch pad through the ESP32 auxiliary channel: `strike.contact` events aligned with camera strike events on the session clock |
| Evidence before development | A device in hand; its latency and bounce characterised with the LED rig |

## 2. What equipment adds

| Quantity | Kinematics alone | With equipment |
|---|---|---|
| Contact occurred | Not known (proximity only) | **Observed** by a switch or pressure threshold |
| Contact instant | Estimated as peak speed | Observed; lets the kinematic strike estimate itself be validated |
| Contact duration | No | Observed (pressure or switch) |
| Impact magnitude | No | Relative per device; in newtons only with a calibrated sensor |
| Contact location on the target | No | With a segmented pad or several sensors |
| Target position | Assumed (virtual) | Measured, when the equipment carries a tag cluster |
| Bag swing, rebound timing | No | From an IMU or tags on the bag |

Camera-derived statements stay what they are ("reached", trajectory, speed before contact);
equipment-derived statements are separate metrics with their own evidence. The two are never
blended into one "power" number.

## 3. Extension contract

All by existing mechanisms; the additions are enum values and one observation variant.

| Element | Contract |
|---|---|
| Device | A registry device of kind `equipment` (new value) with capabilities from `contact`, `impact`, `distance`, `pose` |
| Source | Any adapter: the ESP32 auxiliary channel first; a serial or Bluetooth adapter if a commercial pad needs one |
| Observations | Existing `contact { state }`; new `impact { peak: number; unit: "m_s2" \| "n" \| "relative"; duration_ms: number \| null; zone: string \| null }` |
| Time | Device clock through the offset estimator, as for IMUs; `time_quality` recorded |
| Placement | The equipment is a **trackable**: a tag cluster on it gives `T_world_from_equipment`; without tags, a fixed pose entered or captured once (`equipment_pose` calibration kind) |
| Calibration | `equipment_response` profile: threshold, debounce, and — only when measured against a reference — a scale to newtons with its validity range. Absent that, unit `relative` |
| Targets | A target may be bound to equipment (`target.equipment: <device id>`); `target_reached` then requires contact when the device has a contact capability |
| Metrics | New catalogue entries using existing kinds: `strike.contact_time` (duration), `strike.kinematic_to_contact_lag` (duration between `strike_peak` and contact — also a validation quantity), `strike.impact_peak` (`value_at` over the impact observation), `combo.contact_interval` |
| Comparability | Method class `equipment` plus the device model and response-profile id in the class; never comparable across devices unless both are calibrated to newtons with overlapping validated ranges |
| Recording | Observations are inputs: recorded and replayable like any other |

## 4. Device classes considered

| Class | Gives | Caveats |
|---|---|---|
| Contact switch or conductive pad | Contact, instant, duration | Bounce; needs debounce characterised |
| Pressure-sensitive pad (resistive or capacitive film) | Contact, duration, relative magnitude, sometimes zones | Non-linear, area-dependent, saturates, drifts with temperature and wear |
| Accelerometer on a bag or pad | Impact instant, relative magnitude, bag motion | Measures the target's response, which depends on its mass and mounting |
| Load cell in a mount | Force along one axis | Mechanical design effort; the only route here to newtons |
| Pressure insoles or mat | Foot contact, weight-shift proxies | Would replace the kinematic `foot_contact` estimate; a separate device family with the same contract |
| Commercial "smart" bags and trackers | Whatever their interface exposes | Closed protocols; only through a documented interface |

## 5. Safety

Striking real equipment brings real loads. The platform's volume limits and warm-up rules apply
([17 §3](17-security-privacy-and-safety.md)); wearables are not rated for contact
([21 §8](21-wearable-hardware.md)) and trackers on striking limbs are removed or relocated;
no drill scores maximal impact by default; VR with real equipment is passthrough only, with the
equipment's measured pose drawn.

## 6. Validation

Contact latency and bounce with the LED rig; kinematic-to-contact lag across 50 strikes as a check
of the strike-time estimate ([16 §4](16-accuracy-and-validation.md) V-16 gains a second reference);
magnitude repeatability with a dropped mass; a newton scale only against a load reference, reported
with its range.
