import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent, PointerEvent } from "react";

export type LayoutPreset = "analysis" | "capture" | "focus";
type Side = "left" | "right";
interface DockWidths {
  left: number;
  right: number;
  preset: LayoutPreset;
}

const STORAGE_KEY = "bansayo.studio.docks.v1";
const LIMITS = { min: 180, max: 440, step: 16 };
const PRESETS: Record<LayoutPreset, DockWidths> = {
  analysis: { left: 240, right: 300, preset: "analysis" },
  capture: { left: 220, right: 360, preset: "capture" },
  focus: { left: 240, right: 300, preset: "focus" },
};

function clamp(value: number): number {
  return Math.min(LIMITS.max, Math.max(LIMITS.min, value));
}

export function parseDockWidths(raw: string | null): DockWidths {
  if (!raw) return { ...PRESETS.analysis };
  try {
    const value: unknown = JSON.parse(raw);
    if (!value || typeof value !== "object") return { ...PRESETS.analysis };
    const obj = value as Record<string, unknown>;
    const preset: LayoutPreset = obj.preset === "capture" || obj.preset === "focus"
      ? obj.preset : "analysis";
    return {
      preset,
      left: typeof obj.left === "number" && Number.isFinite(obj.left)
        ? clamp(obj.left) : PRESETS[preset].left,
      right: typeof obj.right === "number" && Number.isFinite(obj.right)
        ? clamp(obj.right) : PRESETS[preset].right,
    };
  } catch {
    return { ...PRESETS.analysis };
  }
}

function read(): DockWidths {
  try {
    return parseDockWidths(typeof window === "undefined" ? null : localStorage.getItem(STORAGE_KEY));
  } catch {
    return { ...PRESETS.analysis };
  }
}

export function useDockWidths() {
  const [layout, setLayout] = useState<DockWidths>(read);
  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(layout));
    } catch {
      // Resizing works without persistent storage.
    }
  }, [layout]);
  const resize = (side: Side, width: number) =>
    setLayout((current) => ({ ...current, [side]: clamp(width) }));
  const setPreset = (preset: LayoutPreset) => setLayout({ ...PRESETS[preset] });
  return { layout, resize, setPreset };
}

export function DockDivider({
  side, width, onResize,
}: {
  side: Side;
  width: number;
  onResize: (side: Side, width: number) => void;
}) {
  const drag = useRef<{ x: number; width: number } | null>(null);
  const start = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0) return;
    drag.current = { x: event.clientX, width };
    event.currentTarget.setPointerCapture(event.pointerId);
    event.preventDefault();
  };
  const move = (event: PointerEvent<HTMLDivElement>) => {
    if (!drag.current) return;
    const direction = side === "left" ? 1 : -1;
    onResize(side, drag.current.width + direction * (event.clientX - drag.current.x));
  };
  const stop = () => { drag.current = null; };
  const keyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
    const direction = event.key === "ArrowRight" ? 1 : -1;
    onResize(side, width + (side === "left" ? direction : -direction) * LIMITS.step);
    event.preventDefault();
  };
  return (
    <div className="dock-divider" role="separator" tabIndex={0} aria-orientation="vertical"
      aria-label={`Resize ${side} panel`}
      aria-valuemin={LIMITS.min} aria-valuemax={LIMITS.max} aria-valuenow={width}
      onPointerDown={start} onPointerMove={move} onPointerUp={stop}
      onPointerCancel={stop} onLostPointerCapture={stop} onKeyDown={keyDown} />
  );
}
