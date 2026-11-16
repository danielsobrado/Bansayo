import type { Reason, SkeletonDefinition, SystemState } from "@contracts";

export function assignmentOptions(
  skeleton: SkeletonDefinition,
  state: SystemState,
  current: string | null,
) {
  const used = new Set(
    state.devices.flatMap((d) => (d.assignment === null ? [] : [d.assignment])),
  );
  return skeleton.segments
    .map((segment) => ({
      id: segment.id,
      assigned: used.has(segment.id),
      current: segment.id === current,
    }))
    .sort((a, b) => Number(a.assigned) - Number(b.assigned));
}
export function attentionReasons(state: SystemState | null): Reason[] {
  if (!state) return [];
  const seen = new Set<string>();
  return [
    ...state.devices.flatMap((d) => d.reasons),
    ...state.features.flatMap((f) => f.reasons),
  ].filter((reason) => {
    const key = `${reason.code}/${reason.subject ?? ""}/${reason.message}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}
