import styles from "./OriginBadge.module.css";

interface OriginBadgeProps {
  /** `simulated`, `replay`, `fixture`, or any origin a newer engine sends. */
  label: string;
}

/** Hatched, hue-free badge for data that is not live (docs/07 §1). */
export function OriginBadge({ label }: OriginBadgeProps) {
  return <span className={styles.badge}>{label.toUpperCase()}</span>;
}
