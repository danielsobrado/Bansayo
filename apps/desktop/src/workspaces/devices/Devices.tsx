import { useEffect, useRef, useState } from "react";
import type { DeviceStatus, SkeletonDefinition, SystemState } from "@contracts";
import { actions } from "../../engine/actions";
import { useAction } from "../../engine/hooks";
import { assignmentOptions } from "../../engine/presentation";
import type { readouts } from "../../engine/readouts";
import { BodyMap } from "../../ui/BodyMap";
import { EmptyState } from "../../ui/EmptyState";
import { StatePill } from "../../ui/StatePill";
import { OriginBadge } from "../../ui/OriginBadge";
import { Reasons } from "../../ui/Reasons";
import { formatConfidence, humanizeId } from "../../lib/format";
import { PhonePairing } from "./PhonePairing";

interface Change {
  device: DeviceStatus;
  segment: string | null;
}
export function Devices({
  state,
  skeleton,
  sampled,
  onCalibrate,
}: {
  state: SystemState | null;
  skeleton: SkeletonDefinition | null;
  sampled: ReturnType<typeof readouts>;
  onCalibrate: () => void;
}) {
  const [mode, setMode] = useState<"cards" | "table" | null>(null);
  const [highlight, setHighlight] = useState<string | null>(null);
  const [change, setChange] = useState<Change | null>(null);
  const [assign, status] = useAction(actions.assignDevice);
  const devices = state?.devices ?? [];
  const view = mode ?? (devices.length > 12 ? "table" : "cards");
  async function confirm() {
    if (!change) return;
    const response = await assign(change.device.descriptor.id, {
      segment: change.segment,
    });
    if (response.ok) setChange(null);
  }
  return (
    <div className="workspace-page">
      <header className="workspace-heading">
        <div>
          <span className="eyebrow">Workspace / Devices</span>
          <h1>
            Devices <span className="heading-count mono">{devices.length}</span>
          </h1>
        </div>
        <div className="workspace-tools">
          <div className="view-presets">
            <button
              aria-pressed={view === "cards"}
              onClick={() => setMode("cards")}
            >
              Cards
            </button>
            <button
              aria-pressed={view === "table"}
              onClick={() => setMode("table")}
            >
              Table
            </button>
          </div>
          <button disabled={!state} onClick={onCalibrate}>
            Calibrate mounting
          </button>
        </div>
      </header>
      <div className="devices-layout">
        <section className={`device-collection ${view}`} aria-label="Devices">
          {devices.length === 0 ? (
            <EmptyState title="No devices configured">
              <p>
                Sources are defined in the engine configuration. Try
                sim-full-body.yaml, sim-six-trackers.yaml,
                sim-camera-only.yaml or cameras-two.yaml.
              </p>
            </EmptyState>
          ) : (
            devices.map((device) => (
              <Device
                key={device.descriptor.id}
                device={device}
                state={state!}
                skeleton={skeleton}
                onChange={(segment) => setChange({ device, segment })}
                onFocus={() => setHighlight(device.assignment)}
                onBlur={() => setHighlight(null)}
              />
            ))
          )}
        </section>
        <aside className="device-map dock">
          <div className="panel-heading">BODY MAP</div>
          <BodyMap
            skeleton={skeleton}
            values={sampled.segments}
            highlight={highlight}
          />
          <div className="page-panel-content">
            <h3 className="eyebrow">Unassigned segments</h3>
            <p className="subtle">
              {skeleton?.segments
                .filter((s) => !devices.some((d) => d.assignment === s.id))
                .map((s) => humanizeId(s.id))
                .join(", ") || "—"}
            </p>
          </div>
          <PhonePairing />
        </aside>
      </div>
      {change && (
        <AssignmentDialog
          change={change}
          pending={status.pending}
          error={status.error}
          onCancel={() => setChange(null)}
          onConfirm={() => void confirm()}
        />
      )}
    </div>
  );
}
function Device({
  device,
  state,
  skeleton,
  onChange,
  onFocus,
  onBlur,
}: {
  device: DeviceStatus;
  state: SystemState;
  skeleton: SkeletonDefinition | null;
  onChange: (segment: string | null) => void;
  onFocus: () => void;
  onBlur: () => void;
}) {
  const [fault, status] = useAction(actions.simulateFault);
  const simulated = device.descriptor.origin === "simulated";
  const tracker = device.descriptor.kind !== "camera";
  return (
    <article
      className="device-entry"
      onMouseEnter={onFocus}
      onMouseLeave={onBlur}
      onFocus={onFocus}
      onBlur={onBlur}
      aria-label={`Device ${device.descriptor.id}`}
    >
      <header>
        <h2 className="mono">{device.descriptor.id}</h2>
        <StatePill state={device.state} />
        {device.descriptor.origin !== "live" && (
          <OriginBadge label={device.descriptor.origin} />
        )}
      </header>
      <p className="device-model subtle">
        {device.descriptor.model ?? device.descriptor.kind}
      </p>
      {tracker && (
        <label className="assignment-label">
          Segment
          <select
            aria-label={`Segment for ${device.descriptor.id}`}
            value={device.assignment ?? ""}
            disabled={!skeleton || state.engine.mode === "replay"}
            onChange={(e) => onChange(e.target.value || null)}
          >
            <option value="">Unassigned</option>
            {skeleton &&
              assignmentOptions(skeleton, state, device.assignment).map(
                (option) => (
                  <option value={option.id} key={option.id}>
                    {humanizeId(option.id)}
                    {option.assigned && !option.current ? " (assigned)" : ""}
                  </option>
                ),
              )}
          </select>
        </label>
      )}
      <dl className="device-data">
        <div>
          <dt>Rate / nominal</dt>
          <dd className="mono">
            {device.stream.measured_rate_hz.toFixed(1)} /{" "}
            {device.descriptor.nominal_rate_hz ?? "—"} Hz
          </dd>
        </div>
        <div>
          <dt>Dropped</dt>
          <dd className="mono">{device.stream.dropped}</dd>
        </div>
        <div>
          <dt>Battery</dt>
          <dd className="mono">
            {device.battery ? Math.round(device.battery.level * 100) : "—"}
            {device.battery ? "%" : ""}
          </dd>
        </div>
        <div>
          <dt>Signal</dt>
          <dd className="mono">{device.signal?.rssi_dbm ?? "—"} dBm</dd>
        </div>
        <div>
          <dt>Quality</dt>
          <dd className="mono">{formatConfidence(device.quality)}</dd>
        </div>
        <div>
          <dt>Timing</dt>
          <dd className="mono">
            {humanizeId(device.time_sync?.time_quality ?? device.stream.time_quality)}
            {device.time_sync?.uncertainty_us != null
              ? ` ±${(device.time_sync.uncertainty_us / 1000).toFixed(1)} ms`
              : ""}
          </dd>
        </div>
        <div>
          <dt>Firmware</dt>
          <dd className="mono">{device.descriptor.firmware ?? "—"}</dd>
        </div>
      </dl>
      <div className="device-calibrations">
        {device.calibration.map((c) => (
          <span key={c.kind}>
            {humanizeId(c.kind)}{" "}
            <strong data-calibration={c.state}>{c.state}</strong>
          </span>
        ))}
      </div>
      <Reasons reasons={device.reasons} />
      {simulated && (
        <div className="fault-controls">
          <span className="eyebrow">Simulation controls</span>
          <div>
            <button
              disabled={status.pending || state.engine.mode === "replay"}
              onClick={() =>
                void fault(device.descriptor.id, {
                  action:
                    device.state === "disconnected"
                      ? "reconnect"
                      : "disconnect",
                })
              }
            >
              {device.state === "disconnected" ? "Reconnect" : "Disconnect"}
            </button>
            <button
              disabled={
                status.pending ||
                device.state === "disconnected" ||
                state.engine.mode === "replay"
              }
              onClick={() =>
                void fault(device.descriptor.id, {
                  action: device.state === "degraded" ? "restore" : "degrade",
                  quality: 0.35,
                })
              }
            >
              {device.state === "degraded" ? "Restore quality" : "Degrade"}
            </button>
          </div>
        </div>
      )}
      {status.error && (
        <p className="error" role="alert">
          {status.error}
        </p>
      )}
    </article>
  );
}
function AssignmentDialog({
  change,
  pending,
  error,
  onCancel,
  onConfirm,
}: {
  change: Change;
  pending: boolean;
  error: string | null;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
  }, []);
  return (
    <dialog ref={ref} onCancel={onCancel} aria-labelledby="assignment-title">
      <h2 id="assignment-title">Change tracker assignment?</h2>
      <p>
        <span className="mono">{change.device.descriptor.id}</span>:{" "}
        {change.device.assignment
          ? humanizeId(change.device.assignment)
          : "unassigned"}{" "}
        → {change.segment ? humanizeId(change.segment) : "unassigned"}.
      </p>
      <p className="calibration-note">
        Mounting calibration for this tracker will become stale and needs
        recapturing. Its previous segment will be inferred.
      </p>
      {error && (
        <p className="error" role="alert">
          {error}
        </p>
      )}
      <footer className="dialog-actions">
        <button onClick={onCancel} disabled={pending}>
          Cancel
        </button>
        <button className="primary" disabled={pending} onClick={onConfirm}>
          {pending ? "Changing…" : "Change assignment"}
        </button>
      </footer>
    </dialog>
  );
}
