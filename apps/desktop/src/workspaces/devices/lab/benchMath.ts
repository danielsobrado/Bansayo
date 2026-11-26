import type { Channel, Point } from "./trace";

export const GRAVITY = 9.80665;
export const FACE_TOLERANCE = 0.08 * GRAVITY;
export const ROTATION_TOLERANCE_DEG = 10;
export const MAX_PAIR_SKEW_US = 40_000;
export const FACES = ["+X", "-X", "+Y", "-Y", "+Z", "-Z"] as const;

export function quaternionAngle(a: readonly number[], b: readonly number[]): number | null {
  const norm = Math.hypot(...a)*Math.hypot(...b);
  if (!Number.isFinite(norm) || norm < 1e-6 || a.length !== 4 || b.length !== 4) return null;
  const dot = a.reduce((sum, x, i) => sum+x*b[i]!, 0)/norm;
  return 2*Math.acos(Math.min(1,Math.abs(dot)))*180/Math.PI;
}
export function faceMeasurement(channel: Channel, face: number) {
  const end = channel.points.at(-1)?.t_us ?? 0;
  const points = channel.points.filter(p => p.t_us >= end-1_000_000);
  if (points.length < 2 || points.at(-1)!.t_us-points[0]!.t_us < 750_000) return null;
  const mean = [0,1,2].map(i => points.reduce((s,p) => s+p.values[i]!,0)/points.length);
  const target = [0,0,0];
  target[Math.floor(face/2)] = face%2 ? -GRAVITY : GRAVITY;
  const error = Math.hypot(...mean.map((v,i)=>v-target[i]!));
  return { mean, error, pass: error <= FACE_TOLERANCE };
}
export function nearestPairs(a: Point[], b: Point[], skew = MAX_PAIR_SKEW_US) {
  const pairs: [Point,Point][] = [];
  let j = 0;
  for (const p of a) {
    while (j+1<b.length && Math.abs(b[j+1]!.t_us-p.t_us)<Math.abs(b[j]!.t_us-p.t_us)) j++;
    const q = b[j];
    if (q && Math.abs(q.t_us-p.t_us)<=skew) pairs.push([p,q]);
  }
  return pairs;
}
