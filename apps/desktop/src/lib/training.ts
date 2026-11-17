/**
 * Display of training values. Unit conversion and number formatting are the only arithmetic the
 * UI performs on measurements (martial-arts 10 §4); grades, scores and cues come from the engine.
 */
import type { Grade, MetricMeasurement, TextParam, TextRef } from "@contracts";
import { EM_DASH, radToDeg } from "./format";

/** Grade wording (17 §4.2); shape and text, never colour alone. */
export const GRADES: Record<Grade, { label: string; mark: string }> = {
  A: { label: "Observed", mark: "●" },
  B: { label: "Estimated", mark: "○" },
  C: { label: "Indicative", mark: "◌" },
  X: { label: "Not assessed", mark: EM_DASH },
};

export function gradeOf(grade: string): { label: string; mark: string } {
  return Object.hasOwn(GRADES, grade)
    ? GRADES[grade as Grade]
    : { label: "Other", mark: "?" };
}

/** A native-unit value in display units: metres as cm, radians as degrees. */
export function formatValue(
  value: number | null | undefined,
  unit: string,
): { text: string; unit: string } {
  if (value === null || value === undefined || !Number.isFinite(value))
    return { text: EM_DASH, unit: "" };
  switch (unit) {
    case "ms":
      return { text: String(Math.round(value)), unit: "ms" };
    case "s":
      return { text: String(Math.round(value * 1000)), unit: "ms" };
    case "m":
      return { text: (value * 100).toFixed(1), unit: "cm" };
    case "rad":
      return { text: radToDeg(value).toFixed(1), unit: "°" };
    case "m_s":
      return { text: value.toFixed(2), unit: "m/s" };
    case "rad_s":
      return { text: radToDeg(value).toFixed(0), unit: "°/s" };
    case "ratio":
    case "":
      return { text: value.toFixed(2), unit: "" };
    default:
      return { text: value.toFixed(2), unit };
  }
}

/** `kick.total_time@1` → `kick.total_time`. */
export const contentId = (ref: string): string => ref.split("@")[0]!;

/** The measurement shown for a metric: value text, grade and whether a reason must be shown. */
export function describeMeasurement(m: MetricMeasurement): {
  value: string;
  unit: string;
  grade: { label: string; mark: string };
  assessed: boolean;
} {
  const assessed = m.grade !== "X" && m.value !== null;
  const v = assessed ? formatValue(m.value, m.unit) : { text: EM_DASH, unit: "" };
  return { value: v.text, unit: v.unit, grade: gradeOf(m.grade), assessed };
}

function formatParam(p: TextParam | number | string): string {
  if (typeof p !== "object") return String(p);
  switch (p.format) {
    case "deg":
      return `${radToDeg(p.value).toFixed(0)}°`;
    case "cm":
      return `${(p.value * 100).toFixed(0)} cm`;
    case "ms":
      return `${Math.round(p.value)} ms`;
    case "percent":
      return `${(p.value * 100).toFixed(0)}%`;
    case "count":
      return String(Math.round(p.value));
    default:
      return String(p.value);
  }
}

/**
 * Fills `{name}` placeholders from parameters. An unknown key shows the key itself, so a missing
 * text is visible rather than silent.
 */
export function resolveText(
  key: string,
  params: Record<string, TextParam | number | string>,
  texts: Record<string, string> | null,
): string {
  const template = texts?.[key];
  if (template === undefined) return key;
  return template.replace(/\{([a-z_]+)\}/g, (all, name: string) =>
    Object.hasOwn(params, name) ? formatParam(params[name]!) : all,
  );
}

export const textOf = (ref: TextRef, texts: Record<string, string> | null): string =>
  resolveText(ref.key, ref.params, texts);
