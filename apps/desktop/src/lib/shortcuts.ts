/** Global keyboard shortcuts that do not fire while typing or on a control that owns the key. */
import { useEffect, useRef } from "react";

const TEXT_ENTRY = new Set(["INPUT", "SELECT", "TEXTAREA"]);

/** Whether a key event should be left to the focused element. */
export function isTypingTarget(
  target: EventTarget | null,
  key: string,
): boolean {
  if (typeof HTMLElement === "undefined" || !(target instanceof HTMLElement))
    return false;
  if (target.closest("dialog[open]")) return true;
  if (target.isContentEditable || TEXT_ENTRY.has(target.tagName)) return true;
  // Space and Enter activate a focused button or link; do not also run a shortcut.
  return (
    (key === " " || key === "Enter") &&
    (target.tagName === "BUTTON" || target.tagName === "A")
  );
}

/**
 * Binds `keys` (KeyboardEvent.key values, case-insensitive for letters) while mounted.
 * Modified keys (Ctrl, Meta, Alt) are ignored so browser shortcuts keep working.
 */
export function useShortcuts(keys: Readonly<Record<string, () => void>>): void {
  const ref = useRef(keys);
  ref.current = keys;
  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.ctrlKey || e.metaKey || e.altKey || e.defaultPrevented) return;
      // A disabled capture control can move focus to <body>; the modal still owns Escape.
      if (document.querySelector("dialog[open]")) return;
      if (isTypingTarget(e.target, e.key)) return;
      const k = e.key.length === 1 ? e.key.toLowerCase() : e.key;
      const fn = ref.current[k];
      if (!fn) return;
      e.preventDefault();
      fn();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
