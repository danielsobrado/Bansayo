import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { MetricMeasurement, ScoreResult } from "@contracts";
import { MeasurementValue } from "./MeasurementValue";
import { ScoreBars } from "./ScoreBars";
import { ClipPlayer } from "../viewport/ClipPlayer";

const m = (over: Partial<MetricMeasurement>): MetricMeasurement => ({
  metric: "kick.peak_ankle_height@1",
  value: 1.21,
  norm: null,
  unit: "m",
  grade: "A",
  evidence: { provenance: "measured", method: "triangulated", confidence: null, sigma: null, sigma_basis: null, t_obs_us: null, sources: [] },
  comparability_class: "c",
  reasons: [],
  ...over,
});

describe("MeasurementValue", () => {
  it("shows value, unit and grade text for A, B and C", () => {
    const a = renderToStaticMarkup(createElement(MeasurementValue, { m: m({}) }));
    expect(a).toContain("121.0");
    expect(a).toContain("cm");
    expect(a).toContain("Observed");
    expect(renderToStaticMarkup(createElement(MeasurementValue, { m: m({ grade: "B" }) }))).toContain("Estimated");
    expect(renderToStaticMarkup(createElement(MeasurementValue, { m: m({ grade: "C" }) }))).toContain("Indicative");
  });

  it("shows a dash and the reason for X, never a value", () => {
    const x = renderToStaticMarkup(
      createElement(MeasurementValue, {
        m: m({ grade: "X", value: null, reasons: [{ code: "calibration_missing", subject: "floor", message: "Floor calibration is needed." }] }),
      }),
    );
    expect(x).toContain("—");
    expect(x).toContain("Not assessed");
    expect(x).toContain("Floor calibration is needed.");
    expect(x).not.toContain("121");
  });
});

describe("ScoreBars", () => {
  it("greys null components with their reason and labels targets", () => {
    const score: ScoreResult = {
      model: "scoring.kick.basic@1",
      comparability_class: "c",
      reference_match: null,
      coverage: 0.5,
      components: [
        { id: "height", score: null, measurement: null, target: null, deviation: null, reasons: [{ code: "calibration_missing", subject: null, message: "No floor." }] },
        { id: "retraction", score: 0.8, measurement: m({ unit: "ms", value: 300 }), target: { value: 280, from: "personal", ref_id: null }, deviation: 20, reasons: [] },
      ],
    };
    const html = renderToStaticMarkup(createElement(ScoreBars, { score, texts: { "component.height": "Height" } }));
    expect(html).toContain('data-null="true"');
    expect(html).toContain("Not assessed: No floor.");
    expect(html).toContain("Height");
    expect(html).toContain("vs your median 280 ms");
    expect(html).toContain('aria-valuenow="80"');
  });
});

describe("ClipPlayer", () => {
  const sample = (t: number) => ({ t_us: t, root_m: [0, 1, t] as [number, number, number], rotations: [[0, 0, 0, 1] as [number, number, number, number]], positions_m: [[0, 1, t] as [number, number, number]], inferred: [t > 1] });
  const clip = { rate_hz: 60, duration_us: 2, skeleton_rev: 1, samples: [sample(0), sample(1), sample(2)], phases: [], strike_t_us: null, paths: [] };

  it("picks the sample at or before the time and loops", () => {
    const p = new ClipPlayer(clip);
    p.seek(1.5);
    expect(p.pose()!.root_position_m[2]).toBe(1);
    p.seek(2);
    expect(p.pose()!.segments[0]!.provenance).toBe("inferred");
    p.seek(0);
    p.play();
    p.advance(0);
    p.advance(0.0025); // 2.5 µs at 1×: past the end, loops to the start
    expect(p.timeUs).toBe(0);
  });
});
