import type { MetricMeasurement } from "@contracts";
import { describeMeasurement, formatValue } from "../lib/training";
import styles from "./training.module.css";

/**
 * The only way a measurement is shown (10 §5.1): value in display units with its grade; an `X`
 * shows a dash and the engine's reasons, never a zero.
 */
export function MeasurementValue({ m, showReasons = true }: { m: MetricMeasurement; showReasons?: boolean }) {
  const d = describeMeasurement(m);
  const sigma = d.assessed && m.evidence?.sigma != null ? formatValue(m.evidence.sigma, m.unit) : null;
  return (
    <span className={styles.measurement} data-grade={m.grade}>
      <span className={styles.value}>{d.value}</span>
      {d.unit && <span className={styles.unit}>{d.unit}</span>}
      {sigma && <span className={styles.sigma} title="Estimated uncertainty">± {sigma.text}</span>}
      <span className={styles.grade} data-grade={m.grade} title={d.grade.label}>
        <span aria-hidden="true">{d.grade.mark}</span>
        {d.grade.label}
      </span>
      {showReasons && !d.assessed && m.reasons.length > 0 && (
        <span className={styles.reasons}>{m.reasons.map((r) => r.message).join(" ")}</span>
      )}
    </span>
  );
}
