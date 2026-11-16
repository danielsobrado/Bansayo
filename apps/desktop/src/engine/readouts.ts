import type {
  DeviceId,
  JointMeasurement,
  Landmark2D,
  LandmarkSchema,
  PoseLandmarks2D,
  Provenance,
} from "@contracts";
import type { FrameMessage, TriangulatedLandmarks } from "./stores";
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

/** Display cue only: the engine decides which points it accepts. */
export const HIGH_REPROJECTION_PX = 5;

export type LandmarkStatus = "accepted" | "high_error" | "not_reconstructed";

export interface CameraView {
  id: DeviceId;
  role: "used" | "rejected" | "not_used";
  /** `undefined`: no fresh 2D set from this camera; `null`: not detected. */
  observation: Landmark2D | null | undefined;
}

/**
 * Low-rate readout of the latest triangulated set for the landmark list and inspector. Pass
 * `null` / an empty map for stale data: a stale set is "waiting", never shown as live.
 */
export function reconstructionReadout(
  set: TriangulatedLandmarks | null,
  views2d: ReadonlyMap<DeviceId, PoseLandmarks2D>,
  schema: LandmarkSchema | null,
  cameraIds: readonly DeviceId[],
  selected: number | null,
) {
  const live = set !== null && schema !== null && set.schema_id === schema.id;
  const points = (schema?.names ?? []).map(
    (_, i) => (live ? set.points[i] : null) ?? null,
  );
  const statuses: LandmarkStatus[] | null = live
    ? points.map((p) =>
        !p
          ? "not_reconstructed"
          : p.reprojection_error_px > HIGH_REPROJECTION_PX
            ? "high_error"
            : "accepted",
      )
    : null;
  const index = selected ?? -1;
  const name = schema?.names[index];
  const point = points[index] ?? null;
  const inspection =
    name === undefined
      ? null
      : {
          name,
          /** `null` while waiting for a live set. */
          status: statuses?.[index] ?? null,
          point,
          cameras: [
            ...new Set([
              ...cameraIds,
              ...(point?.used_views ?? []),
              ...(point?.rejected_views ?? []),
            ]),
          ].map((id): CameraView => {
            const view = views2d.get(id);
            return {
              id,
              role: point?.used_views.includes(id)
                ? "used"
                : point?.rejected_views.includes(id)
                  ? "rejected"
                  : "not_used",
              observation:
                view && view.schema_id === schema?.id
                  ? (view.points[index] ?? null)
                  : undefined,
            };
          }),
        };
  return {
    live,
    origin: live ? set.origin : null,
    reconstructed: points.filter(Boolean).length,
    total: points.length,
    statuses,
    inspection,
  };
}
