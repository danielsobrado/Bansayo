import { useRef, useState } from "react";
import type { SegmentPose, SkeletonDefinition } from "@contracts";
import { useSampledFrame } from "../../engine/hooks";
import { humanizeId } from "../../lib/format";
import { Sparkline, type SparklineSeries } from "../../ui/Sparkline";

export type EvidenceSource = "camera" | "imu" | "hybrid" | "inferred" | "none";

/** Where a segment's rotation comes from, from the method the solver reports (ADR-025). */
export function segmentSource(s: SegmentPose | undefined): EvidenceSource {
  if (!s) return "none";
  if (s.provenance === "inferred" || s.provenance === "unavailable") return "inferred";
  switch (s.method) {
    case "imu_optical_fused":
      return "hybrid";
    case "imu_mounted":
    case "imu_gyro":
      return "imu";
    case "rest_fill":
    case "floor_contact":
    case null:
    case undefined:
      return "inferred";
    default:
      return "camera";
  }
}

const LABEL: Record<EvidenceSource, string> = {
  camera: "Cameras only",
  imu: "Tracker only",
  hybrid: "Tracker, camera-corrected",
  inferred: "Inferred",
  none: "No pose",
};
const WINDOW_US = 10_000_000;
const CAPACITY = 128;

function ring(): SparklineSeries {
  return {
    times: new Float64Array(CAPACITY),
    values: new Float32Array(CAPACITY),
    head: 0,
    count: 0,
  };
}

/**
 * Per segment: which evidence drives it and how old that evidence is. The age traces share one
 * 10 s time axis, so camera-driven segments (stepped, older) and tracker-driven ones (fresh) can be
 * compared at a glance.
 */
export function SegmentEvidence({ skeleton }: { skeleton: SkeletonDefinition | null }) {
  const traces = useRef(new Map<string, SparklineSeries>());
  const [selected, setSelected] = useState<string | null>(null);
  const sample = useSampledFrame(10, (frame) => {
    const segments = frame?.pose?.segments ?? [];
    for (const [i, s] of segments.entries()) {
      const id = skeleton?.segments[i]?.id;
      if (!id || !frame) continue;
      const age = s.t_obs_us == null ? NaN : (frame.t_us - s.t_obs_us) / 1000;
      const r = traces.current.get(id) ?? ring();
      traces.current.set(id, r);
      r.times[r.head] = frame.t_us;
      r.values[r.head] = age;
      r.head = (r.head + 1) % CAPACITY;
      r.count = Math.min(r.count + 1, CAPACITY);
    }
    return { t: frame?.t_us ?? 0, segments, now: frame?.t_us ?? 0 };
  });
  if (!skeleton) return null;
  const pick = skeleton.segments.findIndex((s) => s.id === selected);
  const picked = pick >= 0 ? sample.segments[pick] : undefined;
  return (
    <div className="page-panel-content">
      <h3 className="eyebrow">Segment evidence</h3>
      <table className="sessions-table" aria-label="Evidence per segment">
        <thead>
          <tr>
            <th>Segment</th>
            <th>Source</th>
            <th>Age</th>
            <th>Age, last 10 s</th>
          </tr>
        </thead>
        <tbody>
          {skeleton.segments.map((def, i) => {
            const s = sample.segments[i];
            const source = segmentSource(s);
            const age = s?.t_obs_us == null ? null : (sample.now - s.t_obs_us) / 1000;
            return (
              <tr
                key={def.id}
                aria-selected={selected === def.id}
                onClick={() => setSelected(selected === def.id ? null : def.id)}
                style={{ cursor: "pointer" }}
              >
                <td>{humanizeId(def.id)}</td>
                <td data-source={source}>{LABEL[source]}</td>
                <td className="mono">{age == null ? "—" : `${age.toFixed(0)} ms`}</td>
                <td style={{ width: "40%" }}>
                  <Sparkline
                    series={traces.current.get(def.id) ?? null}
                    windowUs={WINDOW_US}
                    version={sample.t}
                    label={`Evidence age of ${humanizeId(def.id)}`}
                  />
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
      {selected && (
        <dl className="detail-list">
          <dt>Segment</dt>
          <dd>{humanizeId(selected)}</dd>
          <dt>Method</dt>
          <dd className="mono">{picked?.method ?? "—"}</dd>
          <dt>Provenance</dt>
          <dd>{picked ? humanizeId(picked.provenance) : "—"}</dd>
          <dt>Inputs</dt>
          <dd className="mono">{picked?.sources.join(", ") || "—"}</dd>
          <dt>Confidence</dt>
          <dd className="mono">{picked?.confidence?.toFixed(2) ?? "—"}</dd>
        </dl>
      )}
    </div>
  );
}
