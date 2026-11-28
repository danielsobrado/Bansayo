/** Validate fields the UI reads; retain unknown fields and enum strings for forward compatibility. */
import type { ServerMessage } from "@contracts";

type Obj = Record<string, unknown>;
export const isObj = (v: unknown): v is Obj =>
  typeof v === "object" && v !== null && !Array.isArray(v);
export const isNum = (v: unknown): v is number =>
  typeof v === "number" && Number.isFinite(v);
const isStr = (v: unknown): v is string => typeof v === "string";
export const tuple = (v: unknown, n: number): v is number[] =>
  Array.isArray(v) && v.length === n && v.every(isNum);
const strings = (v: unknown): boolean => Array.isArray(v) && v.every(isStr);
const reasons = (v: unknown): boolean =>
  Array.isArray(v) &&
  v.every((r) => isObj(r) && isStr(r.code) && isStr(r.message));
const calibration = (v: unknown): boolean =>
  Array.isArray(v) &&
  v.every((c) => isObj(c) && isStr(c.kind) && isStr(c.state));

function isSkeleton(v: unknown): boolean {
  if (!isObj(v) || !isNum(v.rev) || !isNum(v.height_m) || !isStr(v.model_id))
    return false;
  if (
    !Array.isArray(v.segments) ||
    !v.segments.every(
      (s) =>
        isObj(s) &&
        isStr(s.id) &&
        (s.parent === null || isStr(s.parent)) &&
        tuple(s.offset_m, 3) &&
        tuple(s.tail_m, 3),
    )
  )
    return false;
  if (new Set(v.segments.map((s: Obj) => s.id)).size !== v.segments.length)
    return false;
  return (
    Array.isArray(v.joints) &&
    v.joints.every(
      (j) => isObj(j) && isStr(j.id) && isStr(j.label) && strings(j.segments),
    )
  );
}
function isCamera(v: unknown): boolean {
  return (
    v === null ||
    (isObj(v) &&
      tuple(v.resolution_px, 2) &&
      (v.landmark_schema === null ||
        (isObj(v.landmark_schema) &&
          strings(v.landmark_schema.names) &&
          Array.isArray(v.landmark_schema.edges) &&
          v.landmark_schema.edges.every((e) => tuple(e, 2)))))
  );
}
function isDevice(d: unknown): boolean {
  return (
    isObj(d) &&
    isObj(d.descriptor) &&
    isStr(d.descriptor.id) &&
    isStr(d.descriptor.kind) &&
    isStr(d.descriptor.origin) &&
    (d.descriptor.nominal_rate_hz === null ||
      isNum(d.descriptor.nominal_rate_hz)) &&
    isCamera(d.descriptor.camera) &&
    isStr(d.state) &&
    calibration(d.calibration) &&
    reasons(d.reasons) &&
    (d.assignment === null || isStr(d.assignment)) &&
    isObj(d.stream) &&
    isNum(d.stream.measured_rate_hz) &&
    isNum(d.stream.dropped) &&
    (d.battery === null || (isObj(d.battery) && isNum(d.battery.level))) &&
    (d.signal === null ||
      (isObj(d.signal) &&
        (d.signal.rssi_dbm === null || isNum(d.signal.rssi_dbm))))
  );
}
function isPlayback(v: unknown): boolean {
  return (
    v === null ||
    (isObj(v) &&
      typeof v.paused === "boolean" &&
      isNum(v.speed) &&
      isNum(v.position_us) &&
      typeof v.looping === "boolean" &&
      isObj(v.session) &&
      isStr(v.session.id) &&
      isStr(v.session.name) &&
      isNum(v.session.duration_us))
  );
}
function isState(v: unknown): boolean {
  return (
    isObj(v) &&
    isObj(v.engine) &&
    isStr(v.engine.state) &&
    Array.isArray(v.devices) &&
    v.devices.every(isDevice) &&
    calibration(v.calibration) &&
    Array.isArray(v.features) &&
    v.features.every(
      (f) => isObj(f) && isStr(f.id) && isStr(f.state) && reasons(f.reasons),
    ) &&
    isPlayback(v.playback) &&
    (v.recording === null ||
      (isObj(v.recording) &&
        isStr(v.recording.session_id) &&
        isNum(v.recording.events) &&
        isNum(v.recording.dropped_events))) &&
    (v.simulation === null ||
      (isObj(v.simulation) &&
        isStr(v.simulation.motion) &&
        isNum(v.simulation.speed) &&
        typeof v.simulation.paused === "boolean" &&
        Array.isArray(v.simulation.available_motions) &&
        v.simulation.available_motions.every(
          (m) => isObj(m) && isStr(m.id) && isStr(m.label),
        )))
  );
}
function isFrame(m: Obj): boolean {
  const pose = m.pose;
  const poseOk =
    pose === null ||
    (isObj(pose) &&
      isNum(pose.skeleton_rev) &&
      tuple(pose.root_position_m, 3) &&
      Array.isArray(pose.segments) &&
      pose.segments.every(
        (s) =>
          isObj(s) &&
          tuple(s.position_m, 3) &&
          tuple(s.rotation, 4) &&
          isStr(s.provenance) &&
          strings(s.sources),
      ));
  return (
    isNum(m.seq) &&
    isNum(m.t_us) &&
    strings(m.origins) &&
    Array.isArray(m.joints) &&
    m.joints.every(
      (j) =>
        isObj(j) &&
        (j.value === null || isNum(j.value)) &&
        isStr(j.provenance) &&
        reasons(j.reasons),
    ) &&
    isObj(m.quality) &&
    isStr(m.quality.state) &&
    isNum(m.quality.observed_segments) &&
    isNum(m.quality.total_segments) &&
    poseOk
  );
}
function isLandmarks(v: unknown): boolean {
  return (
    isObj(v) &&
    isObj(v.frame_meta) &&
    isStr(v.frame_meta.device_id) &&
    tuple(v.frame_meta.image_size_px, 2) &&
    Array.isArray(v.points) &&
    v.points.every(
      (p) =>
        p === null ||
        (isObj(p) && isNum(p.x_px) && isNum(p.y_px) && isNum(p.confidence)),
    )
  );
}
/** Only world-frame triangulated sets are drawn; any other kind or frame is dropped. */
function isTriangulated(v: unknown): boolean {
  return (
    isObj(v) &&
    v.kind === "triangulated" &&
    isNum(v.t_us) &&
    isObj(v.frame) &&
    v.frame.type === "world" &&
    isStr(v.origin) &&
    isStr(v.schema_id) &&
    strings(v.calibration_refs) &&
    Array.isArray(v.points) &&
    v.points.every(
      (p) =>
        p === null ||
        (isObj(p) &&
          tuple(p.m, 3) &&
          isNum(p.views) &&
          isNum(p.reprojection_error_px) &&
          isNum(p.confidence) &&
          strings(p.used_views) &&
          strings(p.rejected_views) &&
          (p.view_diagnostics == null ||
            (Array.isArray(p.view_diagnostics) &&
              p.view_diagnostics.every(v => isObj(v) && isStr(v.device_id) &&
                tuple(v.observed_px, 2) && (v.projected_px === null || tuple(v.projected_px, 2)) &&
                (v.error_px === null || (isNum(v.error_px) && v.error_px >= 0)) &&
                tuple(v.ray_origin_m, 3) && tuple(v.ray_direction, 3) &&
                typeof v.used === "boolean")))),
    )
  );
}
function isTrainingState(v: unknown): boolean {
  return (
    v === null ||
    (isObj(v) &&
      isStr(v.session_id) &&
      isStr(v.athlete_id) &&
      isNum(v.rev) &&
      isNum(v.last_event_seq) &&
      isStr(v.phase) &&
      isObj(v.plan) &&
      isStr(v.plan.title) &&
      Array.isArray(v.readiness) &&
      v.readiness.every(
        (g) => isObj(g) && isStr(g.gate) && isStr(g.state) && reasons(g.reasons),
      ) &&
      Array.isArray(v.metric_availability) &&
      v.metric_availability.every(
        (m) => isObj(m) && isStr(m.metric) && isStr(m.best_grade) && reasons(m.reasons),
      ) &&
      isObj(v.totals) &&
      isNum(v.totals.reps_completed) &&
      isNum(v.totals.reps_other) &&
      (v.current === null || (isObj(v.current) && isObj(v.current.counts))) &&
      isStr(v.capability_profile))
  );
}
function isTrainingEvent(v: unknown): boolean {
  return (
    isObj(v) && isNum(v.event_seq) && isNum(v.t_us) && isStr(v.type) && isObj(v.data)
  );
}
function isObservables(m: Obj): boolean {
  return (
    isNum(m.seq) &&
    isNum(m.t_us) &&
    Array.isArray(m.values) &&
    m.values.every(
      (o) => isObj(o) && isStr(o.id) && (o.value === null || isNum(o.value)) && reasons(o.reasons),
    )
  );
}
export type ParsedMessage =
  | { kind: "message"; message: ServerMessage }
  | { kind: "unknown"; type: string }
  | { kind: "invalid" };

export function parseServerMessage(text: string): ParsedMessage {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    return { kind: "invalid" };
  }
  if (!isObj(raw) || typeof raw.type !== "string") return { kind: "invalid" };
  let valid: boolean;
  switch (raw.type) {
    case "hello":
      valid = isNum(raw.protocol_version);
      break;
    case "skeleton":
      valid = isSkeleton(raw.skeleton);
      break;
    case "state":
      valid = isState(raw.state);
      break;
    case "frame":
      valid = isFrame(raw);
      break;
    case "landmarks_2d":
      valid = isLandmarks(raw.landmarks);
      break;
    case "landmarks_3d":
      valid = isTriangulated(raw.landmarks);
      break;
    case "notice":
      valid = typeof raw.message === "string";
      break;
    case "training_state":
      valid = "state" in raw && isTrainingState(raw.state);
      break;
    case "training_event":
      valid = isStr(raw.session_id) && isTrainingEvent(raw.event);
      break;
    case "observables":
      valid = isObservables(raw);
      break;
    default:
      return { kind: "unknown", type: raw.type };
  }
  return valid
    ? { kind: "message", message: raw as unknown as ServerMessage }
    : { kind: "invalid" };
}
export function isCompleteHello(
  m: Extract<ServerMessage, { type: "hello" }>,
): boolean {
  return isSkeleton(m.skeleton) && isState(m.state);
}
