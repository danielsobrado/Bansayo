/** Formatting helpers. The only place where wire units become display units. */

export const EM_DASH = "—";

const RAD_TO_DEG = 180 / Math.PI;

/** Radians to degrees; the single conversion point for angles. */
export function radToDeg(rad: number): number {
  return rad * RAD_TO_DEG;
}

/** An angle in radians shown in degrees with one decimal, or a dash when there is no value. */
export function formatAngle(rad: number | null | undefined): string {
  if (rad === null || rad === undefined || !Number.isFinite(rad)) return EM_DASH;
  const deg = radToDeg(rad);
  // Avoid "-0.0°".
  const fixed = (Math.abs(deg) < 0.05 ? 0 : deg).toFixed(1);
  return `${fixed}°`;
}

/** Confidence `0..1` as `.95`, or a dash. Never converted to an accuracy figure. */
export function formatConfidence(c: number | null | undefined): string {
  if (c === null || c === undefined || !Number.isFinite(c)) return EM_DASH;
  return c >= 1 ? "1.00" : c.toFixed(2).replace(/^0/, "");
}

/** Whole milliseconds, or a dash. */
export function formatMs(ms: number | null | undefined): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms)) return EM_DASH;
  return `${Math.max(0, Math.round(ms))} ms`;
}

/** Microseconds as `m:ss`, or a dash. */
export function formatDurationUs(us: number | null | undefined): string {
  if (us === null || us === undefined || !Number.isFinite(us) || us < 0) return EM_DASH;
  const totalS = Math.floor(us / 1_000_000);
  const m = Math.floor(totalS / 60);
  const s = totalS % 60;
  return `${m}:${String(s).padStart(2, "0")}`;
}

/** Speed multiplier as `1.0×`. */
export function formatSpeed(speed: number): string {
  return `${speed.toFixed(speed < 1 ? 2 : 1)}×`;
}

/** `left_upper_arm` → `left upper arm`. Segment ids are data; this is display only. */
export function humanizeId(id: string): string {
  return id.replaceAll("_", " ");
}

/**
 * Looks up `key` in a table keyed by a contract enum. Unknown values — which newer engines may
 * send (docs/04 §2) — fall back instead of failing.
 */
export function lookup<K extends string, V>(table: Readonly<Record<K, V>>, key: string, fallback: V): V {
  return Object.hasOwn(table, key) ? table[key as K] : fallback;
}
