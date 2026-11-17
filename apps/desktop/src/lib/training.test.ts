import { describe, expect, it } from "vitest";
import type { MetricMeasurement } from "@contracts";
import { contentId, describeMeasurement, formatValue, gradeOf, resolveText } from "./training";

const m = (over: Partial<MetricMeasurement>): MetricMeasurement => ({
  metric: "kick.total_time@1",
  value: 0.61,
  norm: null,
  unit: "ms",
  grade: "A",
  evidence: {} as never,
  comparability_class: "c",
  reasons: [],
  ...over,
});

describe("training display", () => {
  it("converts units for display only", () => {
    expect(formatValue(612.4, "ms")).toEqual({ text: "612", unit: "ms" });
    expect(formatValue(1.214, "m")).toEqual({ text: "121.4", unit: "cm" });
    expect(formatValue(Math.PI / 2, "rad")).toEqual({ text: "90.0", unit: "°" });
    expect(formatValue(null, "m").text).toBe("—");
  });

  it("never shows a value for grade X and names every grade", () => {
    const x = describeMeasurement(m({ grade: "X", value: 3 }));
    expect(x.value).toBe("—");
    expect(x.assessed).toBe(false);
    expect(x.grade.label).toBe("Not assessed");
    expect(describeMeasurement(m({ grade: "C" })).grade.label).toBe("Indicative");
    expect(describeMeasurement(m({ grade: "B" })).grade.label).toBe("Estimated");
    expect(gradeOf("Z").label).toBe("Other");
  });

  it("fills parameters and shows unknown keys", () => {
    const texts = { "cue.kick.pivot.observed": "Turned {deviation_deg} less." };
    expect(
      resolveText("cue.kick.pivot.observed", { deviation_deg: { value: Math.PI / 12, format: "deg" } }, texts),
    ).toBe("Turned 15° less.");
    expect(resolveText("missing.key", {}, texts)).toBe("missing.key");
    expect(contentId("kick.total_time@1")).toBe("kick.total_time");
  });
});
