import type { ReactNode } from "react";
import styles from "../training.module.css";
import { decimate, extent, linear, pathOf, ticks, type Point } from "./scale";

export const SERIES_COLORS = [
  "var(--accent)",
  "var(--state-tracking)",
  "var(--state-degraded)",
  "var(--prov-inferred)",
  "var(--state-acquiring)",
  "var(--ink-2)",
];

export interface Series {
  id: string;
  label: string;
  points: readonly Point[];
  /** Secondary series are drawn dashed and thinner. */
  secondary?: boolean;
  color?: string;
}

export interface Marker {
  x: number;
  label: string;
  kind: "strike" | "break";
}

interface Props {
  label: string;
  series: readonly Series[];
  height?: number;
  /** Fixed x domain; otherwise the data extent. */
  xDomain?: readonly [number, number];
  xFormat?: (v: number) => string;
  yFormat?: (v: number) => string;
  markers?: readonly Marker[];
  cursor?: number | null;
  /** Drawn under the lines in data coordinates, e.g. phase bands. */
  under?: (x: (v: number) => number, top: number, bottom: number) => ReactNode;
}

const W = 640;
const PAD = { l: 44, r: 8, t: 8, b: 20 };

/** SVG axes, lines with gaps, markers and a cursor (10 §5.1). Decimates to the drawing width. */
export function Chart({ label, series, height = 180, xDomain, xFormat = String, yFormat = (v) => v.toPrecision(3), markers = [], cursor = null, under }: Props) {
  const xs = xDomain ?? extent(series.flatMap((s) => s.points.map((p) => p[0])));
  const ys = extent(series.flatMap((s) => s.points.map((p) => p[1])));
  if (!xs || !ys) return <p className={styles.reasons}>No values to chart.</p>;
  const x = linear(xs, [PAD.l, W - PAD.r]);
  const y = linear(ys, [height - PAD.b, PAD.t]);
  return (
    <figure style={{ margin: 0 }}>
      <svg className={styles.chart} viewBox={`0 0 ${W} ${height}`} role="img" aria-label={label}>
        {under?.(x, PAD.t, height - PAD.b)}
        <line className={styles.axis} x1={PAD.l} x2={W - PAD.r} y1={height - PAD.b} y2={height - PAD.b} />
        <line className={styles.axis} x1={PAD.l} x2={PAD.l} y1={PAD.t} y2={height - PAD.b} />
        {ticks(ys).map((t) => (
          <text key={`y${t}`} className={styles.tick} x={PAD.l - 4} y={y(t) + 3} textAnchor="end">{yFormat(t)}</text>
        ))}
        {ticks(xs, 5).map((t) => (
          <text key={`x${t}`} className={styles.tick} x={x(t)} y={height - 6} textAnchor="middle">{xFormat(t)}</text>
        ))}
        {series.map((s, i) => (
          <path
            key={s.id}
            className={styles.line}
            d={pathOf(decimate(s.points, W), x, y)}
            stroke={s.color ?? SERIES_COLORS[i % SERIES_COLORS.length]}
            strokeDasharray={s.secondary ? "4 3" : undefined}
            strokeWidth={s.secondary ? 1 : 1.5}
          />
        ))}
        {series.map((s, i) =>
          s.points.length < 40
            ? s.points.map(([px, py]) =>
                py === null ? null : (
                  <circle key={`${s.id}-${px}`} cx={x(px)} cy={y(py)} r={3} fill={s.secondary ? "none" : (s.color ?? SERIES_COLORS[i % SERIES_COLORS.length])} stroke={s.color ?? SERIES_COLORS[i % SERIES_COLORS.length]} />
                ),
              )
            : null,
        )}
        {markers.map((m) => (
          <line key={`${m.kind}${m.x}`} className={m.kind === "strike" ? styles.strike : styles.break} x1={x(m.x)} x2={x(m.x)} y1={PAD.t} y2={height - PAD.b}>
            <title>{m.label}</title>
          </line>
        ))}
        {cursor !== null && <line className={styles.cursor} x1={x(cursor)} x2={x(cursor)} y1={PAD.t} y2={height - PAD.b} />}
      </svg>
      <ul className={styles.legend}>
        {series.map((s, i) => (
          <li key={s.id}>
            <span className={styles.swatch} style={{ background: s.color ?? SERIES_COLORS[i % SERIES_COLORS.length] }} />
            {s.label}
            {s.secondary ? " (secondary)" : ""}
          </li>
        ))}
      </ul>
    </figure>
  );
}
