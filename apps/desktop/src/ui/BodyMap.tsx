import type { SkeletonDefinition } from "@contracts";
import type { SegmentReadout } from "../engine/readouts";

export function BodyMap({
  skeleton,
  values,
  highlight,
  onSelect,
}: {
  skeleton: SkeletonDefinition | null;
  values: SegmentReadout[];
  highlight?: string | null;
  onSelect?: (id: string) => void;
}) {
  if (!skeleton)
    return <p className="empty-inline">Waiting for a skeleton definition.</p>;
  const positions = new Map<string, [number, number, number]>();
  for (const segment of skeleton.segments) {
    const parent = segment.parent
      ? (positions.get(segment.parent) ?? [0, 0, 0])
      : [0, 0, 0];
    positions.set(segment.id, [
      parent[0]! + segment.offset_m[0],
      parent[1]! + segment.offset_m[1],
      parent[2]! + segment.offset_m[2],
    ]);
  }
  const scale = 220 / skeleton.height_m;
  return (
    <svg
      className="body-map"
      viewBox="0 0 280 270"
      role="img"
      aria-label="Body coverage at the reference pose"
    >
      <title>Body coverage by segment provenance</title>
      {skeleton.segments.map((segment, index) => {
        const p = positions.get(segment.id)!;
        const provenance = values[index]?.provenance ?? "unavailable";
        return (
          <g
            key={segment.id}
            data-prov={provenance}
            data-highlight={highlight === segment.id}
            role={onSelect ? "button" : undefined}
            tabIndex={onSelect ? 0 : undefined}
            aria-label={`${segment.id}: ${provenance}`}
            onClick={() => onSelect?.(segment.id)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                onSelect?.(segment.id);
              }
            }}
          >
            <title>
              {segment.id}: {provenance}
            </title>
            <line
              x1={140 - p[0] * scale}
              y1={250 - p[1] * scale}
              x2={140 - (p[0] + segment.tail_m[0]) * scale}
              y2={250 - (p[1] + segment.tail_m[1]) * scale}
            />
            <circle cx={140 - p[0] * scale} cy={250 - p[1] * scale} r="3" />
          </g>
        );
      })}
    </svg>
  );
}
