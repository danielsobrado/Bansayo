import type { Provenance } from "@contracts";
import { lookup } from "../lib/format";
import styles from "./ProvenanceTag.module.css";

const SHORT: Record<Provenance, string> = {
  measured: "mea",
  derived: "der",
  inferred: "inf",
  unavailable: "n/a",
};

interface ProvenanceTagProps {
  /** A `Provenance`; unknown values render neutrally with the raw word. */
  provenance: string;
  short?: boolean;
}

/** Provenance as hue plus border style: solid measured/derived, dashed inferred, dotted unavailable. */
export function ProvenanceTag({
  provenance,
  short = false,
}: ProvenanceTagProps) {
  return (
    <span
      className={styles.tag}
      data-prov={provenance}
      title={short ? provenance : undefined}
    >
      {short ? lookup(SHORT, provenance, provenance) : provenance}
    </span>
  );
}
