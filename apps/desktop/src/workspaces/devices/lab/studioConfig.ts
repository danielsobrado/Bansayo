import type { ChannelId } from "./trace";
export type StudioView = "overview" | "signals" | "calibration" | "timing" | "compare" | "optical";
export type SignalFocus = ChannelId | "all";
export const TIME_WINDOWS = [1, 2, 5] as const;
export const HISTORY_SECONDS = 5;
export const CURSOR_TOLERANCE_US = 150_000;
const STORAGE_KEY = "bansayo.sensor-studio.layout.v1";
export interface StudioLayout { explorer: boolean; inspector: boolean }
export function loadLayout(): StudioLayout {
  const defaults = {explorer:true, inspector:true};
  try {
    const value = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null");
    return value && typeof value.explorer==="boolean" && typeof value.inspector==="boolean" ? {explorer:value.explorer,inspector:value.inspector} : defaults;
  } catch { return defaults; }
}
export function saveLayout(value: StudioLayout) {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(value)); } catch { /* Storage may be disabled. */ }
}
