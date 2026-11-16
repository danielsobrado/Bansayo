/** OKLCH parsing, conversion to sRGB, and WCAG contrast. Used to feed design tokens to three.js. */

export type Rgb = [number, number, number];

const OKLCH_RE = /^oklch\(\s*([\d.]+)%\s+([\d.]+)\s+([\d.]+)\s*\)$/;

/** Parses `oklch(L% C H)` into [L 0..1, C, H degrees]; null for anything else. */
export function parseOklch(text: string): [number, number, number] | null {
  const m = OKLCH_RE.exec(text.trim());
  if (!m) return null;
  return [Number(m[1]) / 100, Number(m[2]), Number(m[3])];
}

/** OKLCH to linear sRGB (unclamped), per Björn Ottosson's reference matrices. */
export function oklchToLinearSrgb(l: number, c: number, hDeg: number): Rgb {
  const h = (hDeg * Math.PI) / 180;
  const a = c * Math.cos(h);
  const b = c * Math.sin(h);
  const l_ = (l + 0.3963377774 * a + 0.2158037573 * b) ** 3;
  const m_ = (l - 0.1055613458 * a - 0.0638541728 * b) ** 3;
  const s_ = (l - 0.0894841775 * a - 1.291485548 * b) ** 3;
  return [
    4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
    -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
    -0.0041960863 * l_ - 0.7034186147 * m_ + 1.707614701 * s_,
  ];
}

const clamp01 = (x: number): number => Math.min(1, Math.max(0, x));

function gammaEncode(x: number): number {
  return x <= 0.0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - 0.055;
}

/** Gamma-encoded sRGB in 0..1, clamped to the gamut. */
export function oklchToSrgb(l: number, c: number, h: number): Rgb {
  const [r, g, b] = oklchToLinearSrgb(l, c, h);
  return [gammaEncode(clamp01(r)), gammaEncode(clamp01(g)), gammaEncode(clamp01(b))];
}

/** WCAG 2 relative luminance of an `oklch(...)` colour. */
export function relativeLuminance(oklch: string): number {
  const parsed = parseOklch(oklch);
  if (!parsed) throw new Error(`Not an oklch colour: ${oklch}`);
  const [r, g, b] = oklchToLinearSrgb(...parsed).map(clamp01) as Rgb;
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

/** WCAG 2 contrast ratio between two `oklch(...)` colours. */
export function contrastRatio(fg: string, bg: string): number {
  const a = relativeLuminance(fg);
  const b = relativeLuminance(bg);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}
