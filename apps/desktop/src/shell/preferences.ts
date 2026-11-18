import { useEffect, useState } from "react";

export type Theme = "graphite" | "silver" | "onyx";
export type Density = "compact" | "comfortable";
export type Rail = "compact" | "expanded";
export interface Preferences {
  theme: Theme;
  density: Density;
  rail: Rail;
}

export const DEFAULT_PREFERENCES: Readonly<Preferences> = {
  theme: "graphite",
  density: "compact",
  rail: "compact",
};
const STORAGE_KEY = "bansayo.ui.preferences.v1";

export function parsePreferences(raw: string | null): Preferences {
  if (!raw) return { ...DEFAULT_PREFERENCES };
  try {
    const value: unknown = JSON.parse(raw);
    if (!value || typeof value !== "object") return { ...DEFAULT_PREFERENCES };
    const obj = value as Record<string, unknown>;
    return {
      theme: obj.theme === "silver" || obj.theme === "onyx" ? obj.theme : "graphite",
      density: obj.density === "comfortable" ? "comfortable" : "compact",
      rail: obj.rail === "expanded" ? "expanded" : "compact",
    };
  } catch {
    return { ...DEFAULT_PREFERENCES };
  }
}

function readPreferences(): Preferences {
  try {
    return parsePreferences(typeof window === "undefined" ? null : window.localStorage.getItem(STORAGE_KEY));
  } catch {
    return { ...DEFAULT_PREFERENCES };
  }
}

export function usePreferences() {
  const [preferences, setPreferences] = useState<Preferences>(readPreferences);
  useEffect(() => {
    document.documentElement.dataset.theme = preferences.theme;
    document.documentElement.dataset.density = preferences.density;
    try {
      window.localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences));
    } catch {
      // The UI remains usable without persistent storage.
    }
  }, [preferences]);
  const update = (changes: Partial<Preferences>) =>
    setPreferences((current) => ({ ...current, ...changes }));
  return { preferences, update };
}
