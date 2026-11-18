/** Design tokens resolved to three.js colours, so the scene uses the same palette as the CSS. */
import { Color, SRGBColorSpace } from "three";
import { oklchToSrgb, parseOklch } from "../lib/color";

export interface Palette {
  surface0: Color;
  line: Color;
  ink2: Color;
  ink3: Color;
  accent: Color;
  measured: Color;
  derived: Color;
  inferred: Color;
  degraded: Color;
}

/** Reads an OKLCH or hex token from the active theme. */
export function tokenColor(
  name: string,
  from: Element = document.documentElement,
): Color {
  const value = getComputedStyle(from).getPropertyValue(name).trim();
  if (/^#[\da-f]{6}$/i.test(value)) return new Color(value);
  const parsed = parseOklch(value);
  if (!parsed)
    throw new Error(
      `Design token ${name} is not an OKLCH or hex colour: "${value}"`,
    );
  const [r, g, b] = oklchToSrgb(...parsed);
  return new Color().setRGB(r, g, b, SRGBColorSpace);
}

export function readPalette(): Palette {
  return {
    surface0: tokenColor("--surface-0"),
    line: tokenColor("--line"),
    ink2: tokenColor("--ink-2"),
    ink3: tokenColor("--ink-3"),
    accent: tokenColor("--accent"),
    measured: tokenColor("--prov-measured"),
    derived: tokenColor("--prov-derived"),
    inferred: tokenColor("--prov-inferred"),
    degraded: tokenColor("--state-degraded"),
  };
}

/** Reads a token as a CSS string, for canvas text. */
export function tokenCss(name: string): string {
  return getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim();
}
