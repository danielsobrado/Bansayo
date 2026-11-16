import type { Reason } from "@contracts";
import styles from "./Reasons.module.css";

interface ReasonsProps {
  reasons: readonly Reason[];
}

/** The engine's reason messages, verbatim. */
export function Reasons({ reasons }: ReasonsProps) {
  if (reasons.length === 0) return null;
  return (
    <ul className={styles.list}>
      {reasons.map((r, i) => (
        <li key={`${r.code}-${r.subject ?? ""}-${i}`}>{r.message}</li>
      ))}
    </ul>
  );
}
