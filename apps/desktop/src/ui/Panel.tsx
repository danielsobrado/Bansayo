import type { ReactNode } from "react";
import styles from "./Panel.module.css";

interface PanelProps {
  title: string;
  /** Id for the heading, so a landmark can be labelled by it. */
  titleId?: string;
  className?: string;
  children: ReactNode;
}

/** A docked panel section with an uppercase label title. */
export function Panel({ title, titleId, className, children }: PanelProps) {
  return (
    <section className={className ? `${styles.panel} ${className}` : styles.panel} aria-labelledby={titleId}>
      <h2 className={styles.title} id={titleId}>
        {title}
      </h2>
      {children}
    </section>
  );
}
