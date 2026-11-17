import type { ScoreResult } from "@contracts";
import { formatValue, resolveText } from "../lib/training";
import styles from "./training.module.css";

const TARGET: Record<string, string> = {
  personal: "vs your median",
  definition: "vs definition",
  reference: "vs reference",
};

/** Component scores as bars with their target; `null` components greyed with the reason. Never one number. */
export function ScoreBars({ score, texts }: { score: ScoreResult; texts: Record<string, string> | null }) {
  return (
    <ul className={styles.bars} aria-label="Component scores">
      {score.components.map((c) => {
        const name = resolveText(`component.${c.id}`, {}, texts);
        const label = name === `component.${c.id}` ? c.id : name;
        const target = c.target ? TARGET[c.target.from] ?? c.target.from : null;
        const pct = c.score === null ? null : Math.round(c.score * 100);
        return (
          <li key={c.id} className={styles.bar} data-null={c.score === null}>
            <span>{label}</span>
            <span
              className={styles.track}
              role="meter"
              aria-label={label}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={pct ?? undefined}
              aria-valuetext={pct === null ? "Not assessed" : `${pct} of 100`}
            >
              {pct !== null && <span className={styles.fill} style={{ width: `${pct}%` }} />}
            </span>
            <span className="mono">{pct === null ? "—" : pct}</span>
            {(target || c.score === null) && (
              <span className={styles.barNote}>
                {c.score === null
                  ? `Not assessed: ${c.reasons.map((r) => r.message).join(" ") || "no evidence"}`
                  : `${target}${c.target && c.measurement ? ` ${formatValue(c.target.value, c.measurement.unit).text} ${formatValue(c.target.value, c.measurement.unit).unit}` : ""}`}
              </span>
            )}
          </li>
        );
      })}
      {score.reference_match !== null && (
        <li className={styles.bar}>
          <span>Reference match</span>
          <span className="mono">{["far", "distant", "partial", "close", "very close"][Math.min(4, Math.floor(score.reference_match * 5))]}</span>
          <span className={styles.barNote}>Coverage {Math.round(score.coverage * 100)}%</span>
        </li>
      )}
    </ul>
  );
}
