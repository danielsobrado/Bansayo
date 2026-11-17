/** Hash routing: the active workspace lives in `#/<id>`. */
import { useSyncExternalStore } from "react";

/** Workspace id from a hash such as `#/studio`; `fallback` when absent or unknown. */
export function parseHash(
  hash: string,
  known: readonly string[],
  fallback: string,
): { workspace: string; rest: string[] } {
  const [workspace = "", ...rest] = hash.replace(/^#\/?/, "").split("?")[0]!.split("/");
  return known.includes(workspace)
    ? { workspace, rest: rest.filter(Boolean) }
    : { workspace: fallback, rest: [] };
}

const subscribe = (cb: () => void): (() => void) => {
  window.addEventListener("hashchange", cb);
  return () => window.removeEventListener("hashchange", cb);
};

export function useRoute(known: readonly string[], fallback: string): string {
  return useSyncExternalStore(subscribe, () =>
    parseHash(window.location.hash, known, fallback).workspace,
  );
}

/** Workspace and sub-segments of the current hash; re-renders on change. */
export function useHashRoute(
  known: readonly string[],
  fallback: string,
): { workspace: string; rest: string[] } {
  const hash = useSyncExternalStore(subscribe, () => window.location.hash);
  return parseHash(hash, known, fallback);
}

export function navigate(id: string): void {
  window.location.hash = `#/${id}`;
}
