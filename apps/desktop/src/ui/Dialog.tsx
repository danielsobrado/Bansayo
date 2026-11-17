import { useEffect, useId, useRef } from "react";
import type { ReactNode } from "react";
import { Button } from "./Button";
import styles from "./Dialog.module.css";
export interface DialogProps {
  open: boolean;
  title: string;
  description?: string;
  children: ReactNode;
  onClose: () => void;
  className?: string;
  actions?: ReactNode;
}
/** Native modal focus containment, Escape handling and focus restoration. */
export function Dialog({ open, title, description, children, onClose, className, actions }: DialogProps) {
  const ref = useRef<HTMLDialogElement>(null);
  const id = useId();
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
    return () => { if (dialog.open) dialog.close(); };
  }, [open]);
  return <dialog ref={ref} className={[styles.dialog, className].filter(Boolean).join(" ")} aria-labelledby={`${id}-title`} aria-describedby={description ? `${id}-description` : undefined}
    onCancel={event => { event.preventDefault(); onClose(); }}
    onClose={() => { if (open && ref.current && !ref.current.open) onClose(); }}
    onKeyDown={event => {
      if (event.key !== "Tab") return;
      const controls = Array.from(event.currentTarget.querySelectorAll<HTMLElement>('button, a[href], input, select, textarea, [tabindex]'))
        .filter(element => element.tabIndex >= 0 && !element.matches(":disabled") && element.getClientRects().length > 0);
      const first = controls[0];
      const last = controls.at(-1);
      if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus(); }
      else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus(); }
    }}>
    <h2 id={`${id}-title`} className={styles.title}>{title}</h2>
    {description && <p id={`${id}-description`} className={styles.description}>{description}</p>}
    <div className={styles.body}>{children}</div>
    <footer className={styles.actions}>{actions}<Button onClick={onClose}>Close</Button></footer>
  </dialog>;
}
