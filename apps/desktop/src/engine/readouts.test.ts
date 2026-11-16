import { describe, expect, it } from "vitest";
import type { PoseLandmarks2D, ServerMessage } from "@contracts";
import { readouts, reconstructionReadout } from "./readouts";
import type { TriangulatedLandmarks } from "./stores";

type Frame = Extract<ServerMessage, { type: "frame" }>;
const frame: Frame = {
  type: "frame",
  seq: 3,
  t_us: 1000,
  origins: ["simulated"],
  pose: {
    skeleton_rev: 1,
    root_position_m: [0, 1, 0],
    root_position_provenance: "inferred",
    segments: [
      {
        position_m: [0, 1, 0],
        rotation: [0, 0, 0, 1],
        provenance: "derived",
        confidence: 1,
        sources: ["SIM-T01"],
      },
    ],
  },
  joints: [{ value: 1, provenance: "derived", confidence: 0.9, reasons: [] }],
  quality: {
    state: "tracking",
    observed_segments: 1,
    total_segments: 1,
    reasons: [],
  },
};
describe("sampled readouts", () => {
  it("extracts scalar readouts without putting pose geometry into React state", () => {
    const value = readouts(frame, 10);
    expect(value.joints[0]?.value).toBe(1);
    expect(value.segments[0]).toEqual({
      provenance: "derived",
      confidence: 1,
      sources: ["SIM-T01"],
    });
    expect(JSON.stringify(value)).not.toMatch(
      /position_m|rotation|skeleton_rev/,
    );
    expect(value.coverage).toBe("1/1");
    expect(value.timeUs).toBe(1000);
  });
  it("clears every live readout when frames become stale", () => {
    const value = readouts(frame, 501);
    expect(value.joints).toEqual([]);
    expect(value.segments).toEqual([]);
    expect(value.ageMs).toBeNull();
    expect(value.coverage).toBe("—");
  });
  it("has an honest empty state before the first frame", () => {
    expect(readouts(null, Infinity).origins).toEqual([]);
    expect(readouts(null, Infinity).hasPose).toBe(false);
  });
  it("preserves joint reasons for camera-only frames without a pose", () => {
    expect(readouts({ ...frame, pose: null }, 0).hasPose).toBe(false);
    expect(readouts({ ...frame, pose: null }, 0).joints).toEqual(frame.joints);
  });
});

describe("reconstruction inspector", () => {
  const schema = {
    id: "bansayo.body.v1.joints",
    names: ["pelvis", "left_knee", "left_ankle"],
    edges: [
      [0, 1],
      [1, 2],
    ] as [number, number][],
  };
  const set: TriangulatedLandmarks = {
    kind: "triangulated",
    t_us: 1000,
    frame: { type: "world" },
    origin: "simulated",
    schema_id: schema.id,
    calibration_refs: [],
    points: [
      {
        m: [0, 0.93, 0],
        views: 2,
        reprojection_error_px: 1.4,
        confidence: 0.9,
        used_views: ["CAM1", "CAM2"],
        rejected_views: [],
      },
      {
        m: [0.1, 0.5, 0.05],
        views: 2,
        reprojection_error_px: 7.5,
        confidence: 0.5,
        used_views: ["CAM1", "CAM2"],
        rejected_views: ["CAM3"],
      },
      null,
    ],
  };
  const view = (id: string, points: PoseLandmarks2D["points"]) =>
    ({ frame_meta: { device_id: id }, schema_id: schema.id, points }) as PoseLandmarks2D;
  const views2d = new Map([
    ["CAM1", view("CAM1", [{ x_px: 640, y_px: 400, confidence: 0.98 }, null, null])],
    ["CAM2", view("CAM2", [null, null, { x_px: 10, y_px: 20, confidence: 0.4 }])],
  ]);
  const cams = ["CAM1", "CAM2", "CAM3"];

  it("reports an accepted point with its used views and each camera's 2D observation", () => {
    const r = reconstructionReadout(set, views2d, schema, cams, 0);
    expect(r.live).toBe(true);
    expect(r.reconstructed).toBe(2);
    expect(r.statuses).toEqual(["accepted", "high_error", "not_reconstructed"]);
    expect(r.inspection?.status).toBe("accepted");
    expect(r.inspection?.point?.m).toEqual([0, 0.93, 0]);
    expect(r.inspection?.cameras).toEqual([
      { id: "CAM1", role: "used", observation: { x_px: 640, y_px: 400, confidence: 0.98 } },
      { id: "CAM2", role: "used", observation: null },
      { id: "CAM3", role: "not_used", observation: undefined },
    ]);
  });
  it("flags a high reprojection error and the rejected camera", () => {
    const r = reconstructionReadout(set, views2d, schema, cams, 1);
    expect(r.inspection?.status).toBe("high_error");
    expect(r.inspection?.cameras.map((c) => c.role)).toEqual([
      "used",
      "used",
      "rejected",
    ]);
  });
  it("shows a null point as not reconstructed with whatever the cameras still see", () => {
    const r = reconstructionReadout(set, views2d, schema, cams, 2);
    expect(r.inspection?.status).toBe("not_reconstructed");
    expect(r.inspection?.point).toBeNull();
    expect(r.inspection?.cameras[1]).toEqual({
      id: "CAM2",
      role: "not_used",
      observation: { x_px: 10, y_px: 20, confidence: 0.4 },
    });
  });
  it("waits instead of showing a stale or mismatched set as live", () => {
    for (const r of [
      reconstructionReadout(null, new Map(), schema, cams, 0),
      reconstructionReadout({ ...set, schema_id: "other" }, views2d, schema, cams, 0),
    ]) {
      expect(r.live).toBe(false);
      expect(r.statuses).toBeNull();
      expect(r.inspection?.status).toBeNull();
      expect(r.inspection?.point).toBeNull();
    }
    expect(reconstructionReadout(set, views2d, schema, cams, null).inspection).toBeNull();
  });
});
