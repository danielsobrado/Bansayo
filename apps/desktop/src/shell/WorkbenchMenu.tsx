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
  connected: boolean;
  preferences: Preferences;
  onChange: (changes: Partial<Preferences>) => void;
  onCalibrate: () => void;
}

export function WorkbenchMenu({
  items, active, connected, preferences, onChange, onCalibrate,
}: Props) {
  const link = (id: string, label: string) => <a href={`#/${id}`}>{label}</a>;
  return (
    <>
      <div className="workbench-menubar">
        <span className="workbench-wordmark">BANSAYO</span>
        <details className="workbench-menu">
          <summary>File</summary>
          <div className="workbench-dropdown">
            {link("train", "Training")}
            {link("sessions", "Recordings and sessions")}
          </div>
        </details>
        <details className="workbench-menu">
          <summary>View</summary>
          <div className="workbench-dropdown">
            {link("studio", "Motion Studio")}
            {link("cameras", "Camera views")}
            {link("overview", "System overview")}
          </div>
        </details>
        <details className="workbench-menu">
          <summary>Capture</summary>
          <div className="workbench-dropdown">
            {link("cameras", "Cameras")}
            {link("devices", "Sensors and devices")}
            <button type="button" disabled={!connected} onClick={onCalibrate}>Calibrate mounting</button>
          </div>
        </details>
        <details className="workbench-menu">
          <summary>Tools</summary>
          <div className="workbench-dropdown">
            {link("technique", "Technique analysis")}
            {link("progress", "Progress")}
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
            title={`${item.label} (${item.key})`}>
            <Icon name={item.icon} size={14} />
            {item.label}
          </a>
        ))}
      </nav>
    </>
  );
}
