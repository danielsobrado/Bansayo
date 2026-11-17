import type { PhaseInterval } from "@contracts";
import styles from "./training.module.css";
import { SERIES_COLORS } from "./chart/Chart";

interface Props {
  phases: readonly PhaseInterval[];
  strikeUs: number | null;
  /** Repetition window, session clock. */
  startUs: number;
  endUs: number;
  cursorUs?: number | null;
  /** Scrub: called with a session-clock time. */
  onScrub?: (tUs: number) => void;
}

/** Phase intervals as bands, the strike marker and a scrub cursor (10 §5.1). */
export function PhaseTimeline({ phases, strikeUs, startUs, endUs, cursorUs = null, onScrub }: Props) {
  const span = Math.max(1, endUs - startUs);
  const x = (t: number) => ((t - startUs) / span) * 100;
  return (
    <svg
      className={styles.timeline}
      viewBox="0 0 100 40"
      preserveAspectRatio="none"
      role="img"
      aria-label={`Phases: ${phases.map((p) => `${p.phase} ${Math.round((p.t_end_us - p.t_start_us) / 1000)} ms`).join(", ")}`}
      data-testid="phase-timeline"
      onPointerDown={(e) => {
        if (!onScrub) return;
        const r = e.currentTarget.getBoundingClientRect();
        onScrub(startUs + ((e.clientX - r.left) / r.width) * span);
      }}
    >
      {phases.map((p, i) => (
        <g key={`${p.phase}${p.t_start_us}`}>
          <rect className={styles.phase} x={x(p.t_start_us)} y={4} width={Math.max(0.2, x(p.t_end_us) - x(p.t_start_us))} height={32} fill={SERIES_COLORS[i % SERIES_COLORS.length]} fillOpacity={0.45} vectorEffect="non-scaling-stroke">
            <title>{p.phase}</title>
          </rect>
        </g>
      ))}
      {strikeUs !== null && <line className={styles.strike} x1={x(strikeUs)} x2={x(strikeUs)} y1={0} y2={40} vectorEffect="non-scaling-stroke" />}
      {cursorUs !== null && <line className={styles.cursor} x1={x(cursorUs)} x2={x(cursorUs)} y1={0} y2={40} vectorEffect="non-scaling-stroke" />}
    </svg>
  );
}
