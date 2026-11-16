import type { JointMeasurement, Provenance } from "@contracts";
import type { FrameMessage } from "./stores";
import { STALE_FRAME_MS } from "./stores";

export interface SegmentReadout {
  provenance: Provenance;
  confidence: number | null;
  sources: string[];
}
/** Only readouts cross into React. Spatial pose geometry stays in the frame store. */
export function readouts(frame: FrameMessage | null, ageMs: number) {
  const fresh = frame !== null && ageMs <= STALE_FRAME_MS;
  return {
    seq: fresh ? frame.seq : -1,
    timeUs: fresh ? frame.t_us : null,
    ageMs: fresh ? ageMs : null,
    hasPose: fresh && frame.pose !== null,
    origins: fresh ? frame.origins : [],
    coverage: fresh
      ? `${frame.quality.observed_segments}/${frame.quality.total_segments}`
      : "—",
    state: fresh ? frame.quality.state : null,
    joints: fresh ? frame.joints : ([] as JointMeasurement[]),
    segments: fresh
      ? (frame.pose?.segments.map(({ provenance, confidence, sources }) => ({
          provenance,
          confidence,
          sources,
        })) ?? [])
      : ([] as SegmentReadout[]),
  };
}
