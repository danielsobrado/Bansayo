/** Shared lookups for the training screens: content texts, names, and whether the rig is simulated. */
import type { ContentIndex, SystemState } from "@contracts";
import { contentCache, useQuery } from "../../engine/queries";
import { paths } from "../../engine/trainingActions";

export function useTexts(): Record<string, string> | null {
  return useQuery<Record<string, string>>(paths.i18n("en"), contentCache).data;
}

export function useContentIndex(): ContentIndex | null {
  return useQuery<ContentIndex>(paths.contentIndex, contentCache).data;
}

/** Display name of a content id (or `id@version`); the id when unknown. */
export function nameOf(index: ContentIndex | null, ref: string | null | undefined): string {
  if (!ref) return "—";
  const id = ref.split("@")[0]!;
  return index?.entries.find((e) => e.id === id)?.names.display ?? id;
}

/**
 * Simulated sessions are excluded from progress unless asked for (12 §5.4 C5). On a simulator
 * rig the UI asks for them, and shows the origin badge.
 */
export function progressOrigin(state: SystemState | null): "simulated" | null {
  return state?.devices.some((d) => d.descriptor.origin === "simulated") ? "simulated" : null;
}

/** Session ids are validated before they reach a route (10 §3). */
export const SESSION_ID = /^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$/;
export const CONTENT_ID = /^[a-z0-9_]+(\.[a-z0-9_]+)+$/;

export function formatUtc(utc: string): string {
  return utc.replace("T", " ").replace(/(\.\d+)?Z$/, "Z");
}
