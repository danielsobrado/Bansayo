/**
 * Camera poses from the calibration profiles in `SystemState`. Profile `data` is untyped on the
 * wire, so it is validated here: anything malformed means "no calibration" for that camera.
 */
import type {
  CalibrationProfileSummary,
  CameraIntrinsicsData,
  DeviceId,
} from "@contracts";
import { segmentPoint, type Quat, type Vec3 } from "../lib/quat";
import { isNum, isObj, tuple } from "./messages";

export interface CameraPose {
  id: DeviceId;
  /** `q_world_from_camera`, Hamilton `[x, y, z, w]`. Camera frame: +X right, +Y down, +Z optical axis. */
  q: Quat;
  /** Optical centre in world, metres. */
  t: Vec3;
  /** Vertical field of view, radians. */
  fovY: number;
  /** Image width / height. */
  aspect: number;
}

function intrinsics(data: unknown): CameraIntrinsicsData | null {
  if (!isObj(data)) return null;
  const size = data.image_size_px;
  return tuple(size, 2) &&
    size[0]! > 0 &&
    size[1]! > 0 &&
    isNum(data.fx_px) &&
    data.fx_px > 0 &&
    isNum(data.fy_px) &&
    data.fy_px > 0 &&
    isNum(data.cx_px) &&
    isNum(data.cy_px) &&
    tuple(data.distortion, 5)
    ? (data as CameraIntrinsicsData)
    : null;
}

function extrinsics(data: unknown): { q: Quat; t: Vec3 } | null {
  if (
    !isObj(data) ||
    !tuple(data.q_world_from_camera, 4) ||
    !tuple(data.t_world_from_camera_m, 3)
  )
    return null;
  const q = data.q_world_from_camera;
  const norm = Math.hypot(...q);
  if (norm < 1e-6) return null;
  return {
    q: q.map((c) => c / norm) as Quat,
    t: [...data.t_world_from_camera_m] as Vec3,
  };
}

/** Vertical field of view from the focal length and image height, radians. */
export function verticalFov(i: CameraIntrinsicsData): number {
  return 2 * Math.atan(i.image_size_px[1] / 2 / i.fy_px);
}

const UNUSABLE: ReadonlySet<string> = new Set(["missing", "invalid"]);

/** Cameras with both usable intrinsics and extrinsics, in profile order. */
export function calibratedCameras(
  calibration: readonly CalibrationProfileSummary[],
): CameraPose[] {
  const found = new Map<
    DeviceId,
    { i?: CameraIntrinsicsData | null; e?: { q: Quat; t: Vec3 } | null }
  >();
  for (const c of calibration) {
    if (typeof c.device_id !== "string" || UNUSABLE.has(c.state)) continue;
    const entry = found.get(c.device_id) ?? {};
    if (c.kind === "camera_intrinsics") entry.i ??= intrinsics(c.data);
    else if (c.kind === "camera_extrinsics") entry.e ??= extrinsics(c.data);
    else continue;
    found.set(c.device_id, entry);
  }
  return [...found].flatMap(([id, { i, e }]) =>
    i && e
      ? [
          {
            id,
            ...e,
            fovY: verticalFov(i),
            aspect: i.image_size_px[0] / i.image_size_px[1],
          },
        ]
      : [],
  );
}

/** World positions of the image corners `depth` metres along the optical axis: TL, TR, BR, BL. */
export function frustumCorners(pose: CameraPose, depth: number): Vec3[] {
  const h = depth * Math.tan(pose.fovY / 2);
  const w = h * pose.aspect;
  return [
    [-w, -h],
    [w, -h],
    [w, h],
    [-w, h],
  ].map(([x, y]) => segmentPoint(pose.t, pose.q, [x!, y!, depth], [0, 0, 0]));
}
