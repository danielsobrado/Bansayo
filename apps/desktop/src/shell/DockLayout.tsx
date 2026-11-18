import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import type { CSSProperties, PointerEvent, ReactNode, Ref } from "react";
import { useShortcuts } from "../lib/shortcuts";
import { Icon } from "../ui/Icon";

type Workspace = "studio" | "cameras";
type Side = "left" | "right";
type Preset = "analysis" | "capture" | "focus";
type Split = Side | "explorer";
type Layout = Record<Split, number> & {
  preset: Preset;
  leftOpen: boolean;
  rightOpen: boolean;
};
const PRESETS: Record<Preset, Layout> = {
  analysis: { preset: "analysis", left: 210, right: 320, explorer: 42, leftOpen: true, rightOpen: true },
  capture: { preset: "capture", left: 190, right: 340, explorer: 48, leftOpen: true, rightOpen: true },
  focus: { preset: "focus", left: 210, right: 320, explorer: 42, leftOpen: false, rightOpen: false },
};
const PRESET_LABELS = { analysis: "Motion Analysis", capture: "Capture", focus: "Focus" };
const LIMITS = { left: [180, 440], right: [180, 440], explorer: [25, 70] } as const;
const defaultPreset = (workspace: Workspace) => workspace === "studio" ? "analysis" : "capture";
const clamp = (value: number, split: Split) =>
  Math.min(LIMITS[split][1], Math.max(LIMITS[split][0], value));

export function parseDockLayout(raw: string | null, workspace: Workspace): Layout {
  const fallback = PRESETS[defaultPreset(workspace)];
  try {
    const value = JSON.parse(raw ?? "null") as Partial<Layout> | null;
    const preset = value?.preset === "capture" || value?.preset === "focus" ||
      (workspace === "studio" && value?.preset === "analysis") ? value.preset : fallback.preset;
    const base = PRESETS[preset];
    const size = (split: Split) => {
      const n = value?.[split];
      return typeof n === "number" && Number.isFinite(n) ? clamp(n, split) : base[split];
    };
    return {
      preset, left: size("left"), right: size("right"), explorer: size("explorer"),
      leftOpen: typeof value?.leftOpen === "boolean" ? value.leftOpen : base.leftOpen,
      rightOpen: typeof value?.rightOpen === "boolean" ? value.rightOpen : base.rightOpen,
    };
  } catch {
    return { ...fallback };
  }
}

const desktopQuery = "(min-width: 1280px)";
const isDesktop = () => matchMedia(desktopQuery).matches;
function subscribeToWidth(change: () => void) {
  const media = matchMedia(desktopQuery);
  media.addEventListener("change", change);
  return () => media.removeEventListener("change", change);
}

export function DockLayout({
  workspace, toolbar, tools, viewport, explorer, properties,
}: {
  workspace: Workspace;
  toolbar: (showInspector: () => void) => ReactNode;
  tools: ReactNode;
  viewport: ReactNode;
  explorer: ReactNode;
  properties: ReactNode;
}) {
  const storageKey = `bansayo.${workspace}.docks.v1`;
  const [layout, setLayout] = useState(() => {
    try {
      return parseDockLayout(localStorage.getItem(storageKey), workspace);
    } catch {
      return PRESETS[defaultPreset(workspace)];
    }
  });
  useEffect(() => {
    try {
      localStorage.setItem(storageKey, JSON.stringify(layout));
    } catch {
      // Layout controls also work with storage disabled.
    }
  }, [layout, storageKey]);
  const desktop = useSyncExternalStore(subscribeToWidth, isDesktop);
  const [overlay, setOverlay] = useState<Side | null>(null);
  const activeOverlay = desktop ? null : overlay;
  const leftOpen = desktop ? layout.leftOpen : overlay === "left";
  const rightOpen = desktop ? layout.rightOpen : overlay === "right";
  const leftToggle = useRef<HTMLButtonElement>(null);
  const rightToggle = useRef<HTMLButtonElement>(null);
  const overlayClose = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (activeOverlay) overlayClose.current?.focus();
  }, [activeOverlay]);
  const closeOverlay = () => {
    const toggle = overlay === "left" ? leftToggle : rightToggle;
    setOverlay(null);
    toggle.current?.focus();
  };
  useShortcuts(activeOverlay ? { Escape: closeOverlay } : {});
  const toggle = (side: Side) => {
    if (desktop) {
      const key = side === "left" ? "leftOpen" : "rightOpen";
      setLayout((current) => ({ ...current, [key]: !current[key] }));
    } else {
      setOverlay(overlay === side ? null : side);
    }
  };
  const resize = (split: Split, size: number) =>
    setLayout((current) => ({ ...current, [split]: clamp(Math.round(size), split) }));
  const showInspector = () => {
    if (desktop) setLayout((current) => current.rightOpen ? current : { ...current, rightOpen: true });
    else setOverlay("right");
  };
  const leftId = `${workspace}-tools`;
  const rightId = `${workspace}-details`;
  return (
    <>
      <div className="workbench-toolbar" role="group" aria-label="Workspace tools">
        <button ref={leftToggle} type="button" className="icon-control" aria-label="Tools panel"
          title="Tools panel" aria-expanded={leftOpen} aria-controls={leftId} onClick={() => toggle("left")}>
          <Icon name="panel-left" />
        </button>
        {toolbar(showInspector)}
        <div className="dock-controls">
          <select aria-label="Layout preset" value={layout.preset}
            onChange={(event) => {
              setLayout({ ...PRESETS[event.target.value as Preset] });
              setOverlay(null);
            }}>
            {workspace === "studio" && <option value="analysis">{PRESET_LABELS.analysis}</option>}
            <option value="capture">{PRESET_LABELS.capture}</option>
            <option value="focus">{PRESET_LABELS.focus}</option>
          </select>
          <button type="button" className="icon-control" aria-label="Reset dock layout" title="Reset dock layout"
            onClick={() => { setLayout({ ...PRESETS[layout.preset] }); setOverlay(null); }}>
            <Icon name="rotate" />
          </button>
          <button ref={rightToggle} type="button" className="icon-control" aria-label="Explorer and inspector"
            title="Explorer and inspector" aria-expanded={rightOpen} aria-controls={rightId} onClick={() => toggle("right")}>
            <Icon name="panel-right" />
          </button>
        </div>
      </div>
      <div className="workbench-docks" data-desktop={desktop}
        onKeyDown={(event) => {
          if (!activeOverlay || event.key !== "Escape" || event.defaultPrevented) return;
          event.preventDefault();
          closeOverlay();
        }} style={{
        "--tools-width": desktop && leftOpen ? `${layout.left}px` : "0px",
        "--details-width": desktop && rightOpen ? `${layout.right}px` : "0px",
        "--explorer-height": `${layout.explorer}%`,
      } as CSSProperties}>
        <DockPanel side="left" id={leftId} label="Contextual tools" open={leftOpen}>
          <DockHeading title="Tools" onClose={activeOverlay === "left" ? closeOverlay : undefined}
            closeRef={activeOverlay === "left" ? overlayClose : undefined} />
          <div className="dock-scroll">{tools}</div>
        </DockPanel>
        {desktop && leftOpen && <DockSplit split="left" size={layout.left} controls={leftId} onResize={resize} />}
        <div className="dock-viewport" inert={activeOverlay !== null}>{viewport}</div>
        {desktop && rightOpen && <DockSplit split="right" size={layout.right} controls={rightId} onResize={resize} />}
        <DockPanel side="right" id={rightId} label="Explorer and inspector" open={rightOpen}>
          <section className="dock-explorer" id={`${workspace}-explorer`} aria-label="Scene explorer">
            <DockHeading title="Scene Explorer" onClose={activeOverlay === "right" ? closeOverlay : undefined}
              closeRef={activeOverlay === "right" ? overlayClose : undefined} />
            {explorer}
          </section>
          <DockSplit split="explorer" size={layout.explorer} controls={`${workspace}-explorer`} onResize={resize} />
          <PropertyInspector>{properties}</PropertyInspector>
        </DockPanel>
        {activeOverlay && <button type="button" className="dock-backdrop" aria-label="Close overlay panel"
          onClick={closeOverlay} />}
      </div>
    </>
  );
}

function DockPanel({ side, id, label, open, children }: {
  side: Side; id: string; label: string; open: boolean; children: ReactNode;
}) {
  return <aside className={`dock ${side === "left" ? "dock-tools" : "dock-details"}`}
    id={id} aria-label={label} hidden={!open}>{children}</aside>;
}

function DockHeading({ title, onClose, closeRef }: {
  title: string; onClose?: () => void; closeRef?: Ref<HTMLButtonElement>;
}) {
  return (
    <div className="panel-heading dock-heading">
      <h2>{title}</h2>
      {onClose && <button ref={closeRef} type="button" className="icon-control" aria-label={`Close ${title.toLowerCase()}`}
        onClick={onClose}><Icon name="close" size={14} /></button>}
    </div>
  );
}

function PropertyInspector({ children }: { children: ReactNode }) {
  return (
    <section className="dock-properties" aria-label="Properties inspector">
      <DockHeading title="Properties" />
      <div className="dock-scroll">{children}</div>
    </section>
  );
}

function DockSplit({ split, size, controls, onResize }: {
  split: Split;
  size: number;
  controls: string;
  onResize: (split: Split, size: number) => void;
}) {
  const drag = useRef<{ pointer: number; origin: number; size: number; factor: number } | null>(null);
  const stacked = split === "explorer";
  const direction = split === "right" ? -1 : 1;
  const coordinate = (event: PointerEvent) => stacked ? event.clientY : event.clientX;
  const [min, max] = LIMITS[split];
  return <div className={`dock-divider dock-divider-${split}`} role="separator" tabIndex={0}
    aria-orientation={stacked ? "horizontal" : "vertical"} aria-controls={controls}
    aria-label={`Resize ${split === "explorer" ? "scene explorer" : `${split} panel`}`}
    aria-valuemin={min} aria-valuemax={max} aria-valuenow={size}
    aria-valuetext={`${size}${stacked ? "%" : " pixels"}`}
    onPointerDown={(event) => {
      if (event.button !== 0 || drag.current) return;
      const height = event.currentTarget.parentElement?.clientHeight ?? 1;
      drag.current = {
        pointer: event.pointerId, origin: coordinate(event), size,
        factor: stacked ? 100 / Math.max(height, 1) : direction,
      };
      event.currentTarget.focus();
      event.currentTarget.setPointerCapture(event.pointerId);
      event.preventDefault();
    }}
    onPointerMove={(event) => {
      const start = drag.current;
      if (start?.pointer === event.pointerId)
        onResize(split, start.size + (coordinate(event) - start.origin) * start.factor);
    }}
    onLostPointerCapture={() => { drag.current = null; }}
    onKeyDown={(event) => {
      const forward = stacked ? "ArrowDown" : "ArrowRight";
      const back = stacked ? "ArrowUp" : "ArrowLeft";
      const step = stacked ? 5 : 16;
      const next = event.key === forward ? size + direction * step :
        event.key === back ? size - direction * step :
        event.key === "Home" ? min : event.key === "End" ? max : null;
      if (next === null) return;
      event.preventDefault();
      onResize(split, next);
    }} />;
}
