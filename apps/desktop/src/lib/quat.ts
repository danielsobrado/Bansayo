/** Quaternion helpers in the wire convention: Hamilton, `[x, y, z, w]`. No axis conversion. */

export type Vec3 = [number, number, number];
export type Quat = [number, number, number, number];

/** Rotates `v` by unit quaternion `q` (v' = q·v·q⁻¹) into `out`, which is returned. */
export function rotateVec3(
  q: Readonly<Quat>,
  v: Readonly<Vec3>,
  out: Vec3,
): Vec3 {
  const [qx, qy, qz, qw] = q;
  const [vx, vy, vz] = v;
  // t = 2 · (q.xyz × v)
  const tx = 2 * (qy * vz - qz * vy);
  const ty = 2 * (qz * vx - qx * vz);
  const tz = 2 * (qx * vy - qy * vx);
  // v' = v + w·t + q.xyz × t
  out[0] = vx + qw * tx + (qy * tz - qz * ty);
  out[1] = vy + qw * ty + (qz * tx - qx * tz);
  out[2] = vz + qw * tz + (qx * ty - qy * tx);
  return out;
}

/** World position of the point `local` (in the segment frame) for a segment at `origin` with `rotation`. */
export function segmentPoint(
  origin: Readonly<Vec3>,
  rotation: Readonly<Quat>,
  local: Readonly<Vec3>,
  out: Vec3,
): Vec3 {
  rotateVec3(rotation, local, out);
  out[0] += origin[0];
  out[1] += origin[1];
  out[2] += origin[2];
  return out;
}
