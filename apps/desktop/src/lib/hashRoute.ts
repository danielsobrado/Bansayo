/** Hash routing: the active workspace lives in `#/<id>`. */
import { useSyncExternalStore } from "react";

/** Workspace id from a hash such as `#/studio`; `fallback` when absent or unknown. */
export function parseHash(hash: string, known: readonly string[], fallback: string): string {
  const id = hash.replace(/^#\/?/, "").split(/[/?]/)[0] ?? "";
  return known.includes(id) ? id : fallback;
}

const subscribe = (cb: () => void): (() => void) => {
  window.addEventListener("hashchange", cb);
  return () => window.removeEventListener("hashchange", cb);
};

export function useRoute(known: readonly string[], fallback: string): string {
  return useSyncExternalStore(subscribe, () => parseHash(window.location.hash, known, fallback));
}

export function navigate(id: string): void {
  window.location.hash = `#/${id}`;
}
