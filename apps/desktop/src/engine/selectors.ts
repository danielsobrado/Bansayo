/**
 * Derived values. Pure functions of what the engine reported; nothing here decides availability —
 * it only summarises states the engine already published.
 */
import type {
  CalibrationState,
  DeviceStatus,
  FeatureId,
  Origin,
  Reason,
  SkeletonDefinition,
  SystemState,
} from "@contracts";

export type CalibrationReadiness = "ready" | "action_needed" | "assumptions";

const NEEDS_ACTION: ReadonlySet<string> = new Set<CalibrationState>([
  "missing",
  "stale",
  "invalid",
]);

/**
 * Status-bar summary of the calibration states the engine reports: any missing, stale or invalid
 * profile means action is needed; otherwise any assumed one means running on assumptions.
 */
export function calibrationReadiness(
  state: SystemState | null,
): CalibrationReadiness | null {
  if (!state) return null;
  const states: string[] = state.calibration.map((c) => c.state);
  for (const d of state.devices)
    for (const c of d.calibration) states.push(c.state);
  if (states.some((s) => NEEDS_ACTION.has(s))) return "action_needed";
  if (states.includes("assumed")) return "assumptions";
  return "ready";
}

/** Distinct origins that are not `live`, in order of first appearance. */
export function nonLiveOrigins(
  origins: readonly Origin[] | null | undefined,
): string[] {
  if (!origins) return [];
  return [...new Set(origins.filter((o) => o !== "live"))];
}

export function featureReasons(
  state: SystemState | null,
  id: FeatureId,
): Reason[] {
  return state?.features.find((f) => f.id === id)?.reasons ?? [];
}

export function deviceById(
  state: SystemState | null,
  id: string,
): DeviceStatus | null {
  return state?.devices.find((d) => d.descriptor.id === id) ?? null;
}

export interface SegmentRow {
  /** Index into `SkeletonDefinition.segments` (and `SkeletonPose.segments`). */
  index: number;
  id: string;
  depth: number;
}

/** Segments in hierarchy order (depth-first, children in definition order), with their depth. */
export function segmentRows(skeleton: SkeletonDefinition | null): SegmentRow[] {
  if (!skeleton) return [];
  const children = new Map<string | null, number[]>();
  const ids = new Set(skeleton.segments.map((s) => s.id));
  skeleton.segments.forEach((s, i) => {
    // A parent the definition does not contain is treated as a root rather than dropping the segment.
    const key = s.parent !== null && ids.has(s.parent) ? s.parent : null;
    children.set(key, [...(children.get(key) ?? []), i]);
  });
  const rows: SegmentRow[] = [];
  const visit = (parent: string | null, depth: number): void => {
    for (const i of children.get(parent) ?? []) {
      const id = skeleton.segments[i]!.id;
      rows.push({ index: i, id, depth });
      visit(id, depth + 1);
    }
  };
  visit(null, 0);
  return rows;
}
