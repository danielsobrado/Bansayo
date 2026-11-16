import { useState } from "react";
import type { DeviceStatus, SystemState } from "@contracts";
import { CameraPreview } from "./CameraPreview";
import { EmptyState } from "../../ui/EmptyState";
import { StatePill } from "../../ui/StatePill";
import { Reasons } from "../../ui/Reasons";

export function Cameras({ state }: { state: SystemState | null }) {
  const cameras =
    state?.devices.filter((d) => d.descriptor.kind === "camera") ?? [];
  const [selected, setSelected] = useState<string | null>(null);
  const camera =
    cameras.find((d) => d.descriptor.id === selected) ?? cameras[0];
  return (
    <div className="workspace-page">
      <header className="workspace-heading">
        <div>
          <span className="eyebrow">Workspace / Cameras</span>
          <h1>
            Cameras <span className="heading-count mono">{cameras.length}</span>
          </h1>
        </div>
        <span className="subtle">Image coordinates · pixels</span>
      </header>
      {camera ? (
        <div className="cameras-layout">
          <section className="camera-main">
            <CameraTabs
              cameras={cameras}
              selected={camera.descriptor.id}
              onSelect={setSelected}
            />
            <CameraPreview device={camera} />
          </section>
          <aside className="camera-detail dock">
            <div className="panel-heading">CAMERA DETAILS</div>
            <div className="page-panel-content">
              <h2 className="mono">{camera.descriptor.id}</h2>
              <p>
                <StatePill state={camera.state} />
              </p>
              <dl className="detail-list">
                <dt>Source</dt>
                <dd>{camera.descriptor.origin}</dd>
                <dt>Resolution</dt>
                <dd className="mono">
                  {camera.descriptor.camera?.resolution_px.join(" × ") ?? "—"}
                </dd>
                <dt>Frame rate</dt>
                <dd className="mono">
                  {camera.stream.measured_rate_hz.toFixed(1)} /{" "}
                  {camera.descriptor.nominal_rate_hz ?? "—"} fps
                </dd>
                <dt>Landmark schema</dt>
                <dd className="mono">
                  {camera.descriptor.camera?.landmark_schema?.id ?? "—"}
                </dd>
                <dt>Dropped frames</dt>
                <dd className="mono">{camera.stream.dropped}</dd>
              </dl>
              <div className="device-calibrations">
                {camera.calibration.map((c) => (
                  <span key={c.kind}>
                    {c.kind.replaceAll("_", " ")} <strong>{c.state}</strong>
                  </span>
                ))}
              </div>
              <Reasons reasons={camera.reasons} />
              <p className="calibration-note">
                Single-camera landmarks are 2D image measurements. They do not
                provide metric 3D position.
              </p>
            </div>
          </aside>
        </div>
      ) : (
        <EmptyState title="No cameras configured">
          <p>
            Add a camera to the simulated source, or run sim-camera-only.yaml.
            No video capture or detector is running in this PoC.
          </p>
        </EmptyState>
      )}
    </div>
  );
}
export function CameraTabs({
  cameras,
  selected,
  onSelect,
}: {
  cameras: DeviceStatus[];
  selected: string;
  onSelect: (id: string) => void;
}) {
  return (
    <div className="camera-tabs" aria-label="Camera selection">
      {cameras.map((camera) => (
        <button
          key={camera.descriptor.id}
          aria-pressed={camera.descriptor.id === selected}
          onClick={() => onSelect(camera.descriptor.id)}
        >
          {camera.descriptor.id}
        </button>
      ))}
    </div>
  );
}
export function CameraInset({
  cameras,
  main = false,
}: {
  cameras: DeviceStatus[];
  main?: boolean;
}) {
  const [selected, setSelected] = useState<string | null>(null);
  const [expanded, setExpanded] = useState(false);
  const camera =
    cameras.find((d) => d.descriptor.id === selected) ?? cameras[0];
  if (!camera) return null;
  return (
    <div
      className={`camera-inset ${main ? "camera-primary" : ""} ${expanded ? "expanded" : ""}`}
    >
      <header>
        <span className="mono">{camera.descriptor.id}</span>
        {!main && (
          <button
            aria-label={
              expanded ? "Shrink camera preview" : "Enlarge camera preview"
            }
            onClick={() => setExpanded(!expanded)}
          >
            {expanded ? "↙" : "↗"}
          </button>
        )}
      </header>
      <CameraPreview device={camera} />
      {cameras.length > 1 && (
        <CameraTabs
          cameras={cameras}
          selected={camera.descriptor.id}
          onSelect={setSelected}
        />
      )}
    </div>
  );
}
