import { describe, expect, it } from "vitest";
import type { CalibrationProfileSummary } from "@contracts";
import { rotateVec3, type Vec3 } from "../lib/quat";
import {
  calibratedCameras,
  frustumCorners,
  verticalFov,
} from "./cameraCalibration";

const intrinsics = {
  image_size_px: [1280, 720],
  fx_px: 772,
  fy_px: 772,
  cx_px: 640,
  cy_px: 360,
  distortion: [0, 0, 0, 0, 0],
};
// Camera at [0, 1.2, 3.2] looking at [0, 1, 0]: a rotation about +X by π − atan(0.2 / 3.2) turns
// the optical axis (+Z camera) to −Z world, tilted slightly down, and image-down (+Y) to −Y.
const theta = Math.PI - Math.atan2(0.2, 3.2);
const extrinsics = {
  q_world_from_camera: [Math.sin(theta / 2), 0, 0, Math.cos(theta / 2)],
  t_world_from_camera_m: [0, 1.2, 3.2],
};
function profiles(
  intr: unknown = intrinsics,
  extr: unknown = extrinsics,
): CalibrationProfileSummary[] {
  const base = {
    state: "valid",
    device_id: "SIM-CAM1",
    segment_id: null,
    method: "declared_by_simulator",
    created_utc: null,
  } as const;
  return [
    { ...base, id: "i", kind: "camera_intrinsics", data: intr as never },
    { ...base, id: "e", kind: "camera_extrinsics", data: extr as never },
    { ...base, id: "m", kind: "mounting", device_id: "SIM-T01" },
  ];
}

describe("camera calibration to pose and frustum", () => {
  it("points the optical axis at the look-at target and image-down to −Y world", () => {
    const [camera] = calibratedCameras(profiles());
    expect(camera?.id).toBe("SIM-CAM1");
    const forward = rotateVec3(camera!.q, [0, 0, 1], [0, 0, 0]);
    const down = rotateVec3(camera!.q, [0, 1, 0], [0, 0, 0]);
    expect(forward[2]).toBeLessThan(-0.99);
    expect(down[1]).toBeLessThan(-0.99);
    // t + |target − t| · forward lands on the target.
    const d = Math.hypot(0.2, 3.2);
    expect(camera!.t[1] + d * forward[1]).toBeCloseTo(1);
    expect(camera!.t[2] + d * forward[2]).toBeCloseTo(0);
    const corners = frustumCorners(camera!, 1);
    const [tl, tr, , bl] = corners as [Vec3, Vec3, Vec3, Vec3];
    const centre = [0, 1, 2].map(
      (k) => corners.reduce((sum, c) => sum + c[k]!, 0) / 4,
    );
    expect(centre[2]! - camera!.t[2]).toBeCloseTo(forward[2]); // image centre on the optical axis
    expect(bl[1]).toBeLessThan(tl[1]); // image bottom is lower in the world
    expect(tr[0]).toBeGreaterThan(tl[0]); // image right is +X when looking down −Z
  });
  it("sizes the frustum from fy and the image size", () => {
    const [camera] = calibratedCameras(profiles());
    expect(camera!.fovY).toBeCloseTo(2 * Math.atan(360 / 772));
    expect(camera!.aspect).toBeCloseTo(16 / 9);
    expect(
      verticalFov({ ...intrinsics, fy_px: 360 } as never) * (180 / Math.PI),
    ).toBeCloseTo(90);
  });
  it("treats malformed or missing calibration data as no camera", () => {
    expect(calibratedCameras(profiles(null))).toEqual([]);
    expect(calibratedCameras(profiles("intrinsics"))).toEqual([]);
    expect(
      calibratedCameras(profiles({ ...intrinsics, fy_px: 0 })),
    ).toEqual([]);
    expect(
      calibratedCameras(profiles({ ...intrinsics, image_size_px: [1280] })),
    ).toEqual([]);
    expect(
      calibratedCameras(
        profiles(intrinsics, { ...extrinsics, q_world_from_camera: [0, 0, 0, 0] }),
      ),
    ).toEqual([]);
    expect(
      calibratedCameras(
        profiles(intrinsics, { ...extrinsics, t_world_from_camera_m: [0, "1", 3] }),
      ),
    ).toEqual([]);
    expect(
      calibratedCameras(
        profiles().map((p) => ({ ...p, state: "missing" as const })),
      ),
    ).toEqual([]);
    expect(calibratedCameras(profiles().slice(1))).toEqual([]);
  });
});
