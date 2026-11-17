import { useId } from "react";
import type { InputHTMLAttributes, ReactNode } from "react";
import styles from "./Field.module.css";
export interface FieldProps extends InputHTMLAttributes<HTMLInputElement> {
  label: string;
  hint?: ReactNode;
  error?: string;
}
export function Field({ id, label, hint, error, className, "aria-describedby": describedBy, ...props }: FieldProps) {
  const generated = useId();
  const controlId = id ?? generated;
  const description = [describedBy, hint ? `${controlId}-hint` : null, error ? `${controlId}-error` : null].filter(Boolean).join(" ") || undefined;
  return <div className={styles.field}>
    <label htmlFor={controlId} className={styles.label}>{label}</label>
    <input {...props} id={controlId} className={[styles.input, className].filter(Boolean).join(" ")} aria-describedby={description} aria-invalid={error ? true : props["aria-invalid"]} />
    {hint && <span id={`${controlId}-hint`} className={styles.hint}>{hint}</span>}
    {error && <span id={`${controlId}-error`} className={styles.error} role="alert">{error}</span>}
  </div>;
}
