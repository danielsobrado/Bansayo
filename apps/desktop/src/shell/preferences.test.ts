import { describe, expect, it } from "vitest";
import { DEFAULT_PREFERENCES, parsePreferences } from "./preferences";
import { parseDockWidths } from "../workspaces/studio/useDockWidths";

describe("workstation preferences", () => {
  it("returns safe defaults for missing or invalid data", () => {
    expect(parsePreferences(null)).toEqual(DEFAULT_PREFERENCES);
    expect(parsePreferences("{bad")).toEqual(DEFAULT_PREFERENCES);
    expect(parsePreferences(JSON.stringify({ theme: "invalid", density: "unknown", rail: true })))
      .toEqual(DEFAULT_PREFERENCES);
  });

  it("restores independently selected theme, density and navigation", () => {
    expect(parsePreferences(JSON.stringify({
      theme: "silver", density: "comfortable", rail: "expanded",
    }))).toEqual({
      theme: "silver", density: "comfortable", rail: "expanded",
    });
  });
});

describe("studio dock preferences", () => {
  it("starts with analysis proportions", () => {
    expect(parseDockWidths(null)).toEqual({ left: 240, right: 300, preset: "analysis" });
  });

  it("clamps corrupted widths and preserves a valid preset", () => {
    expect(parseDockWidths(JSON.stringify({
      left: -500, right: 7000, preset: "capture",
    }))).toEqual({ left: 180, right: 440, preset: "capture" });
  });

  it("rejects non-finite and invalid values", () => {
    expect(parseDockWidths(JSON.stringify({
      left: "bad", right: null, preset: "focus",
    }))).toEqual({ left: 240, right: 300, preset: "focus" });
  });
});
