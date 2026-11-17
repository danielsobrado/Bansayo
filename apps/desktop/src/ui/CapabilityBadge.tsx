import type { CapabilityReport } from "@contracts";
import styles from "./training.module.css";

/** Capability profile and the number of unavailable observables (10 §5.1). */
export function CapabilityBadge({ report }: { report: CapabilityReport | null | undefined }) {
  if (!report) return null;
  const unavailable = report.observables.filter((o) => o.state === "unavailable").length;
  return (
    <span
      className={styles.badge}
      title={report.observables
        .filter((o) => o.state === "unavailable")
        .map((o) => `${o.id}: ${o.reasons.map((r) => r.message).join(" ")}`)
        .join("\n")}
    >
      Tracking: <strong>{report.profile}</strong>
      {unavailable > 0 && <span>· {unavailable} unavailable</span>}
    </span>
  );
}
