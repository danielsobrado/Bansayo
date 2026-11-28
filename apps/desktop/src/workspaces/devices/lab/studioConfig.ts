import type { ChannelId } from "./trace";
export type StudioView = "overview" | "signals" | "calibration" | "timing" | "compare" | "optical" | "events" | "capture" | "reliability" | "performance";
export type SignalFocus = ChannelId | "all";
export const TIME_WINDOWS = [1, 2, 5] as const;
export const HISTORY_SECONDS = 5;
export const CURSOR_TOLERANCE_US = 150_000;
const STORAGE_KEY = "bansayo.sensor-studio.layout.v1";
export const EXPLORER_WIDTH = {min:180,max:320,default:222};
export const INSPECTOR_WIDTH = {min:180,max:340,default:238};
export interface StudioLayout { explorer: boolean; inspector: boolean; explorerWidth:number; inspectorWidth:number }
export const DEFAULT_LAYOUT:StudioLayout = {explorer:true,inspector:true,explorerWidth:EXPLORER_WIDTH.default,inspectorWidth:INSPECTOR_WIDTH.default};
function width(value:unknown,limits:{min:number;max:number;default:number}) {
  return typeof value==="number"&&Number.isFinite(value)?Math.max(limits.min,Math.min(limits.max,Math.round(value))):limits.default;
}
export function loadLayout(): StudioLayout {
  const defaults = {...DEFAULT_LAYOUT};
  try {
    const value = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null");
    return value && typeof value.explorer==="boolean" && typeof value.inspector==="boolean" ? {explorer:value.explorer,inspector:value.inspector,explorerWidth:width(value.explorerWidth,EXPLORER_WIDTH),inspectorWidth:width(value.inspectorWidth,INSPECTOR_WIDTH)} : defaults;
  } catch { return defaults; }
}
export function saveLayout(value: StudioLayout) {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(value)); } catch { /* Storage may be disabled. */ }
}

export const AXIS_CSS = ["var(--studio-axis-x)","var(--studio-axis-y)","var(--studio-axis-z)","var(--studio-axis-w)"] as const;
