import { describe, expect, it } from "vitest";
import type { SegmentPose } from "@contracts";
import { segmentSource } from "./SegmentEvidence";

const pose = (p: Partial<SegmentPose>): SegmentPose => ({
  rotation: [0, 0, 0, 1],
  position_m: [0, 0, 0],
  provenance: "derived",
  confidence: null,
  sources: [],
  ...p,
});

describe("segmentSource", () => {
  it("classifies by the solver's method, inferred first", () => {
    expect(segmentSource(undefined)).toBe("none");
    expect(segmentSource(pose({ method: "imu_mounted" }))).toBe("imu");
    expect(segmentSource(pose({ method: "imu_optical_fused" }))).toBe("hybrid");
    expect(segmentSource(pose({ method: "skeleton_fit_axis" }))).toBe("camera");
    expect(segmentSource(pose({ method: "rest_fill" }))).toBe("inferred");
    expect(segmentSource(pose({ provenance: "inferred", method: "imu_mounted" }))).toBe("inferred");
    // An older engine without the field never claims a source.
    expect(segmentSource(pose({}))).toBe("inferred");
  });
});
