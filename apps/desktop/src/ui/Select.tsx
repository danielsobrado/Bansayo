import { useId } from "react";
import type { SelectHTMLAttributes, ReactNode } from "react";
import styles from "./Select.module.css";
export interface SelectOption { value: string; label: string; disabled?: boolean }
export interface SelectProps extends SelectHTMLAttributes<HTMLSelectElement> {
  label: string;
  options: readonly SelectOption[];
  hint?: ReactNode;
  error?: string;
}
export function Select({ id, label, options, hint, error, className, "aria-describedby": describedBy, ...props }: SelectProps) {
  const generated = useId();
  const controlId = id ?? generated;
  const description = [describedBy, hint ? `${controlId}-hint` : null, error ? `${controlId}-error` : null].filter(Boolean).join(" ") || undefined;
  return <div className={styles.field}>
    <label htmlFor={controlId} className={styles.label}>{label}</label>
    <select {...props} id={controlId} className={[styles.select, className].filter(Boolean).join(" ")} aria-describedby={description} aria-invalid={error ? true : props["aria-invalid"]}>
      {options.map(option => <option key={option.value} value={option.value} disabled={option.disabled}>{option.label}</option>)}
    </select>
    {hint && <span id={`${controlId}-hint`} className={styles.hint}>{hint}</span>}
    {error && <span id={`${controlId}-error`} className={styles.error} role="alert">{error}</span>}
  </div>;
}
