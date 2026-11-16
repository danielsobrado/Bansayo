import { useEffect, useRef } from "react";
import styles from "./Sparkline.module.css";

export interface SparklineSeries {
  /** Ring buffer of sample times, microseconds. */
  times: Float64Array;
  /** Ring buffer of values, index-aligned with `times`; NaN draws a gap. */
  values: Float32Array;
  /** Index of the next write in the rings. */
  head: number;
  /** Number of valid samples. */
  count: number;
}

interface SparklineProps {
  series: SparklineSeries | null;
  /** Width of the time window, microseconds, ending at the newest sample. */
  windowUs: number;
  /** Redraw trigger: changes whenever new samples may be present. */
  version: number;
  label: string;
}

/** A rolling trace on a small canvas. Draws in the current text colour. */
export function Sparkline({
  series,
  windowUs,
  version,
  label,
}: SparklineProps) {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;
    const dpr = window.devicePixelRatio || 1;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    if (
      canvas.width !== Math.round(w * dpr) ||
      canvas.height !== Math.round(h * dpr)
    ) {
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);
    if (!series || series.count < 2) return;

    const cap = series.times.length;
    const newest = (series.head - 1 + cap) % cap;
    const tEnd = series.times[newest]!;
    const tStart = tEnd - windowUs;
    let min = Number.POSITIVE_INFINITY;
    let max = Number.NEGATIVE_INFINITY;
    for (let k = 0; k < series.count; k++) {
      const i = (newest - k + cap) % cap;
      if (series.times[i]! < tStart) break;
      const v = series.values[i]!;
      if (Number.isNaN(v)) continue;
      if (v < min) min = v;
      if (v > max) max = v;
    }
    if (!Number.isFinite(min)) return;
    // Keep a flat signal visibly flat instead of stretching noise to full height.
    const MIN_SPAN = 0.05;
    const span = Math.max(max - min, MIN_SPAN);
    const mid = (max + min) / 2;
    const pad = 2;
    const y = (v: number): number =>
      pad + (h - 2 * pad) * (1 - (v - (mid - span / 2)) / span);
    const x = (t: number): number => ((t - tStart) / windowUs) * w;

    ctx.strokeStyle = getComputedStyle(canvas).color;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    let drawing = false;
    for (let k = series.count - 1; k >= 0; k--) {
      const i = (newest - k + cap) % cap;
      const t = series.times[i]!;
      if (t < tStart) continue;
      const v = series.values[i]!;
      if (Number.isNaN(v)) {
        drawing = false;
        continue;
      }
      if (drawing) ctx.lineTo(x(t), y(v));
      else ctx.moveTo(x(t), y(v));
      drawing = true;
    }
    ctx.stroke();
  }, [series, windowUs, version]);

  return (
    <canvas ref={ref} className={styles.canvas} role="img" aria-label={label} />
  );
}
