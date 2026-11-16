import { describe, expect, it } from "vitest";
import type { ServerMessage } from "@contracts";
import { readouts } from "./readouts";

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
