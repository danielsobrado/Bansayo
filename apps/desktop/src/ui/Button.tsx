import type { ButtonHTMLAttributes } from "react";
import styles from "./Button.module.css";
export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "default" | "primary" | "danger";
}
export function Button({ type = "button", variant = "default", className, ...props }: ButtonProps) {
  return <button type={type} className={[styles.button, className].filter(Boolean).join(" ")} data-variant={variant} {...props} />;
}
