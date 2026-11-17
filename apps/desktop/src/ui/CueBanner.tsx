import type { CueInstance } from "@contracts";
import { textOf } from "../lib/training";
import styles from "./training.module.css";

/** One cue: basis, observed sentence, suggestion. The engine already limits cues. */
export function CueBanner({ cue, texts }: { cue: CueInstance; texts: Record<string, string> | null }) {
  return (
    <div className={styles.cue} role="status" data-testid="cue">
      <span className={styles.cueBasis}>{cue.resolved ? "resolved" : cue.basis}</span>
      <span>{textOf(cue.observed, texts)}</span>
      <span className={styles.cueSuggestion}>{textOf(cue.suggestion, texts)}</span>
    </div>
  );
}
