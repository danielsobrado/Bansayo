import { describe, expect, it } from "vitest";
import { DEFAULT_PREFERENCES, parsePreferences } from "./preferences";
import { parseDockLayout } from "./DockLayout";

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

describe("workbench dock preferences", () => {
  it("uses independent defaults for Studio and Cameras", () => {
    expect(parseDockLayout(null, "studio")).toEqual({
      preset: "analysis", left: 210, right: 320, explorer: 42, leftOpen: true, rightOpen: true,
    });
    expect(parseDockLayout(null, "cameras")).toMatchObject({ preset: "capture", right: 340, explorer: 48 });
  });

  it("clamps corrupted widths and preserves a valid preset", () => {
    expect(parseDockLayout(JSON.stringify({
      left: -500, right: 7000, preset: "capture",
    }), "studio")).toEqual({
      preset: "capture", left: 180, right: 440, explorer: 48, leftOpen: true, rightOpen: true,
    });
    expect(parseDockLayout('{"left":440,"right":180,"preset":"analysis"}', "studio"))
      .toMatchObject({ left: 440, right: 180, explorer: 42 });
  });

  it("rejects non-finite and invalid values", () => {
    expect(parseDockLayout(JSON.stringify({
      left: "bad", right: null, preset: "focus", explorer: 2, leftOpen: "true", rightOpen: true,
    }), "cameras")).toEqual({
      preset: "focus", left: 210, right: 320, explorer: 25, leftOpen: false, rightOpen: true,
    });
  });
});
