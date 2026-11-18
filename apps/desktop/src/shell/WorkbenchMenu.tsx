import { Icon, type IconName } from "../ui/Icon";
import type { Density, Preferences, Theme } from "./preferences";

interface WorkspaceLink {
  id: string;
  label: string;
  key: string;
  icon: IconName;
}
interface Props {
  items: readonly WorkspaceLink[];
  active: string;
  canCalibrate: boolean;
  preferences: Preferences;
  onChange: (changes: Partial<Preferences>) => void;
  onCalibrate: () => void;
}

export function WorkbenchMenu({
  items, active, canCalibrate, preferences, onChange, onCalibrate,
}: Props) {
  const bar = useRef<HTMLDivElement>(null);
  const openMenu = () => bar.current?.querySelector<HTMLDetailsElement>("details[open]");
  useEffect(() => {
    const dismiss = (event: PointerEvent) => {
      if (event.target instanceof Node && !openMenu()?.contains(event.target))
        openMenu()?.removeAttribute("open");
    };
    document.addEventListener("pointerdown", dismiss);
    return () => document.removeEventListener("pointerdown", dismiss);
  }, []);
  const link = (id: string, label: string) => <a href={`#/${id}`}>{label}</a>;
  return (
    <>
      <div className="workbench-menubar" ref={bar}
        onKeyDown={(event) => {
          const menu = openMenu();
          if (event.key !== "Escape" || !menu) return;
          event.preventDefault();
          menu.open = false;
          menu.querySelector("summary")?.focus();
        }}
        onClick={(event) => {
          if (event.target instanceof Element && event.target.closest(".workbench-dropdown > :is(a, button)"))
            openMenu()?.removeAttribute("open");
        }}>
        <span className="workbench-wordmark">BANSAYO</span>
        <details className="workbench-menu" name="application-menu">
          <summary>File</summary>
          <div className="workbench-dropdown">
            {link("sessions", "Recordings and sessions")}
          </div>
        </details>
        <details className="workbench-menu" name="application-menu">
          <summary>View</summary>
          <div className="workbench-dropdown">
            {link("studio", "Motion Studio")}
            {link("cameras", "Camera views")}
            {link("overview", "System overview")}
          </div>
        </details>
        <details className="workbench-menu" name="application-menu">
          <summary>Capture</summary>
          <div className="workbench-dropdown">
            {link("cameras", "Cameras")}
            {link("devices", "Sensors and devices")}
            <button type="button" disabled={!canCalibrate} onClick={onCalibrate}>Calibrate mounting</button>
          </div>
        </details>
        <details className="workbench-menu" name="application-menu">
          <summary>Training</summary>
          <div className="workbench-dropdown">
            {link("train", "Train")}
            {link("technique", "Technique")}
            {link("progress", "Progress")}
          </div>
        </details>
        <details className="workbench-menu" name="application-menu">
          <summary>Tools</summary>
          <div className="workbench-dropdown">
            {link("devices", "Device assignment")}
            <button type="button" onClick={() => onChange({ theme: "graphite", density: "compact", rail: "compact" })}>
              Reset interface settings
            </button>
          </div>
        </details>
        <details className="workbench-menu" name="application-menu">
          <summary>Help</summary>
          <div className="workbench-dropdown workbench-shortcuts">
            <span>Play or pause <kbd>Space</kbd></span>
            <span>Record or stop <kbd>R</kbd></span>
            <span>Reset Studio view <kbd>Home</kbd></span>
          </div>
        </details>
        <span className="workbench-menu-spacer" />
        <label className="workbench-setting">
          Theme
          <select aria-label="Interface theme" value={preferences.theme}
            onChange={(event) => onChange({ theme: event.target.value as Theme })}>
            <option value="graphite">Graphite</option>
            <option value="silver">Silver</option>
            <option value="onyx">Onyx</option>
          </select>
        </label>
        <label className="workbench-setting">
          Density
          <select aria-label="Interface density" value={preferences.density}
            onChange={(event) => onChange({ density: event.target.value as Density })}>
            <option value="compact">Compact</option>
            <option value="comfortable">Comfortable</option>
          </select>
        </label>
      </div>
      <nav className="workbench-tabs" aria-label="Workspace tabs">
        {items.map((item) => (
          <a key={item.id} href={`#/${item.id}`}
            aria-current={active === item.id ? "page" : undefined}
            aria-keyshortcuts={item.key}
            title={`${item.label} (${item.key})`}>
            <Icon name={item.icon} size={14} />
            {item.label}
          </a>
        ))}
      </nav>
    </>
  );
}
import { useEffect, useRef } from "react";
