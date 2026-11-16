import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";
import { formatAngle, formatConfidence, formatDurationUs } from "./format";
import { contrastRatio } from "./color";
import { rotateVec3 } from "./quat";
import { parseHash } from "./hashRoute";

describe("display conventions", () => {
  it("converts radians to one-decimal degrees and never invents unavailable values", () => {
    expect(formatAngle(Math.PI / 2)).toBe("90.0°");
    expect(formatAngle(null)).toBe("—");
    expect(formatAngle(Number.NaN)).toBe("—");
    expect(formatAngle(-0.00001)).toBe("0.0°");
    expect(formatConfidence(0.95)).toBe(".95");
    expect(formatDurationUs(125_000_000)).toBe("2:05");
  });
  it("uses Hamilton xyzw quaternions and keeps the wire axes", () => {
    const out: [number, number, number] = [0, 0, 0];
    rotateVec3([0, Math.SQRT1_2, 0, Math.SQRT1_2], [0, 0, 1], out);
    expect(out[0]).toBeCloseTo(1);
    expect(out[1]).toBeCloseTo(0);
    expect(out[2]).toBeCloseTo(0);
  });
  it("falls back from unknown hash workspaces", () => {
    expect(parseHash("#/studio", ["studio"], "studio")).toBe("studio");
    expect(parseHash("#/unknown", ["studio"], "studio")).toBe("studio");
  });
  it("keeps text and semantic readout tokens at AA contrast on panels", () => {
    const css = readFileSync(
      new URL("../styles/tokens.css", import.meta.url),
      "utf8",
    );
    const token = (name: string) =>
      new RegExp(`${name}: (oklch\\([^)]+\\))`).exec(css)![1]!;
    for (const name of [
      "--ink-1",
      "--ink-2",
      "--ink-3",
      "--state-tracking",
      "--state-degraded",
      "--state-acquiring",
      "--state-disconnected",
      "--state-unavailable",
      "--prov-derived",
      "--prov-inferred",
      "--prov-unavailable",
    ]) {
      expect(
        contrastRatio(token(name), token("--surface-1")),
        name,
      ).toBeGreaterThanOrEqual(4.5);
    }
  });
});

describe("modal keyboard ownership", () => {
  it("leaves Escape and navigation keys to the active native dialog", async () => {
    const { vi } = await import("vitest");
    const { isTypingTarget } = await import("./shortcuts");
    class DialogButton {
      tagName = "BUTTON";
      isContentEditable = false;
      closest(selector: string) {
        return selector === "dialog[open]" ? {} : null;
      }
    }
    vi.stubGlobal("HTMLElement", DialogButton);
    try {
      expect(
        isTypingTarget(new DialogButton() as unknown as EventTarget, "Escape"),
      ).toBe(true);
      expect(
        isTypingTarget(new DialogButton() as unknown as EventTarget, "3"),
      ).toBe(true);
    } finally {
      vi.unstubAllGlobals();
    }
  });
});
