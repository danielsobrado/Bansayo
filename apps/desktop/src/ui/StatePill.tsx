import type { FeatureState, TrackingState } from "@contracts";
import { lookup } from "../lib/format";
import styles from "./StatePill.module.css";

const GLYPH: Record<TrackingState | FeatureState, string> = {
  available: "●",
  tracking: "●",
  degraded: "◐",
  acquiring: "○",
  disconnected: "□",
  unavailable: "✕",
};

const WORD: Record<TrackingState | FeatureState, string> = {
  available: "Available",
  tracking: "Tracking",
  degraded: "Degraded",
  acquiring: "Acquiring",
  disconnected: "Disconnected",
  unavailable: "Unavailable",
};

interface StatePillProps {
  /** A `TrackingState` or `FeatureState`; unknown values render as a neutral "?" with the raw word. */
  state: string;
  /** Text after the word, e.g. coverage. */
  detail?: string;
}

/** State as shape, hue and word (docs/07 §1). */
export function StatePill({ state, detail }: StatePillProps) {
  return (
    <span className={styles.pill} data-state={state}>
      <span className={styles.glyph} aria-hidden="true">
        {lookup(GLYPH, state, "?")}
      </span>
      <span>{lookup(WORD, state, state)}</span>
      {detail ? <span className={styles.detail}>{detail}</span> : null}
    </span>
  );
}
