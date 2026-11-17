import { useId } from "react";
import styles from "./Readout.module.css";
export interface ReadoutProps {
  label: string;
  value: number | string | null;
  unit?: string;
  precision?: number;
  large?: boolean;
  className?: string;
}
export function Readout({ label, value, unit, precision = 1, large = false, className }: ReadoutProps) {
  const id = useId();
  const available = value !== null && (typeof value !== "number" || Number.isFinite(value));
  const display = !available ? "—" : typeof value === "number" ? value.toFixed(Math.min(20, Math.max(0, Math.trunc(precision) || 0))) : value;
  return <div className={[styles.readout, className].filter(Boolean).join(" ")} data-large={large}>
    <span id={id} className={styles.label}>{label}</span>
    <output aria-labelledby={id} className={styles.value}>{display}{available && unit && <span className={styles.unit}> {unit}</span>}</output>
  </div>;
}
