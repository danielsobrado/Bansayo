/** Chart arithmetic: scales, ticks, gap-preserving paths and decimation. Display only. */

export type Point = readonly [x: number, y: number | null];

export function linear(
  [d0, d1]: readonly [number, number],
  [r0, r1]: readonly [number, number],
): (v: number) => number {
  const span = d1 - d0 || 1;
  return (v) => r0 + ((v - d0) / span) * (r1 - r0);
}

/** Extent of the finite values, padded when flat; `null` when there are none. */
export function extent(values: Iterable<number | null>): [number, number] | null {
  let lo = Infinity;
  let hi = -Infinity;
  for (const v of values)
    if (v !== null && Number.isFinite(v)) {
      lo = Math.min(lo, v);
      hi = Math.max(hi, v);
    }
  if (lo > hi) return null;
  if (lo === hi) return [lo - 1, hi + 1];
  const pad = (hi - lo) * 0.05;
  return [lo - pad, hi + pad];
}

/** About `count` round tick values across the domain. */
export function ticks([lo, hi]: readonly [number, number], count = 4): number[] {
  const raw = (hi - lo) / Math.max(1, count);
  const mag = 10 ** Math.floor(Math.log10(raw || 1));
  const step = [1, 2, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? raw;
  const out: number[] = [];
  for (let v = Math.ceil(lo / step) * step; v <= hi + step * 1e-9; v += step) {
    out.push(v);
    if (v + step <= v) break;
  }
  return out;
}

/** Keeps at most `max` points by stride, never dropping a gap marker. */
export function decimate(points: readonly Point[], max: number): Point[] {
  if (points.length <= max) return [...points];
  const stride = Math.ceil(points.length / max);
  return points.filter((p, i) => i % stride === 0 || p[1] === null || i === points.length - 1);
}

/** An SVG path; unavailable values break the line and are never bridged. */
export function pathOf(
  points: readonly Point[],
  x: (v: number) => number,
  y: (v: number) => number,
): string {
  let d = "";
  let pen = false;
  for (const [px, py] of points) {
    if (py === null || !Number.isFinite(py)) {
      pen = false;
      continue;
    }
    d += `${pen ? "L" : "M"}${x(px).toFixed(1)},${y(py).toFixed(1)}`;
    pen = true;
  }
  return d;
}
