import { useMemo, useState } from "react";
import type { DeviceStatus, SkeletonDefinition, SystemState } from "@contracts";
import { calibratedCameras } from "../../engine/cameraCalibration";
import { readouts } from "../../engine/readouts";
import { jointHistory, TRACE_WINDOW_US } from "../../engine/jointHistory";
import { featureReasons, segmentRows } from "../../engine/selectors";
import { formatAngle, formatConfidence, formatMs, humanizeId } from "../../lib/format";
import type { SceneSelection } from "../../lib/sceneSelection";
import { useShortcuts } from "../../lib/shortcuts";
import { DockLayout } from "../../shell/DockLayout";
import { SceneExplorer, type SceneGroup } from "../../shell/SceneExplorer";
import { Icon } from "../../ui/Icon";
import { OriginBadge } from "../../ui/OriginBadge";
import { ProvenanceTag } from "../../ui/ProvenanceTag";
import { Reasons } from "../../ui/Reasons";
import { Sparkline } from "../../ui/Sparkline";
import { StatePill } from "../../ui/StatePill";
import { ViewportCanvas } from "../../viewport/ViewportCanvas";
import type { ViewPreset } from "../../viewport/MotionViewport";
import { CameraInset } from "../cameras/Cameras";
import { CameraProperties } from "../cameras/CameraProperties";

type Sampled = ReturnType<typeof readouts>;
interface Props {
  skeleton: SkeletonDefinition | null;
  sampled: Sampled;
  state: SystemState | null;
  onCalibrate: () => void;
  connected: boolean;
}
const VIEWS = [
  ["3D", "P"], ["Front", "F"], ["Side", "L"], ["Top", "T"],
] as const satisfies readonly (readonly [ViewPreset, string])[];

export function Studio({ skeleton, sampled, state, onCalibrate, connected }: Props) {
  const [selection, setSelection] = useState<SceneSelection | null>(null);
  const [activeCamera, setActiveCamera] = useState<string | null>(null);
  const [view, setView] = useState<{ preset: ViewPreset; revision: number }>({ preset: "3D", revision: 0 });
  const [skeletonVisible, setSkeletonVisible] = useState(true);
  const [floorVisible, setFloorVisible] = useState(true);
  const [camerasVisible, setCamerasVisible] = useState(false);
  const calibration = state?.calibration;
  const room = state?.active_room ?? null;
  const poses = useMemo(() => calibratedCameras(calibration ?? [], room), [calibration, room]);
  const devices = state?.devices ?? [];
  const cameras = devices.filter((d) => d.descriptor.kind === "camera");
  const sensors = devices.filter((d) => d.descriptor.kind !== "camera");
  const rows = useMemo(() => segmentRows(skeleton), [skeleton]);
  const cameraOnly = cameras.length > 0 &&
    state?.features.some((feature) => feature.id === "skeleton_pose" && feature.state === "unavailable");
  const selectedDevice = selection && "id" in selection
    ? devices.find((device) => device.descriptor.id === selection.id)
    : undefined;
  const selected = selection ?? (rows[0] && { kind: "segment", index: rows[0].index }) ?? null;
  const select = (target: SceneSelection) => {
    setSelection(target);
    if (target.kind === "camera") {
      setActiveCamera(target.id);
      setCamerasVisible(true);
    }
  };
  const setPreset = (preset: ViewPreset) => setView((current) => ({ preset, revision: current.revision + 1 }));
  useShortcuts({ f: () => setPreset("Front"), l: () => setPreset("Side"), t: () => setPreset("Top"),
    p: () => setPreset("3D"), Home: () => setPreset("3D") });

  const groups: SceneGroup[] = [
    { label: state?.athlete?.display_name ?? "Athlete", icon: "layers", expanded: true, nodes: rows.map((row) => ({
      selection: { kind: "segment", index: row.index }, label: humanizeId(row.id), depth: row.depth,
      detail: sampled.segments[row.index]?.provenance,
    })) },
    { label: "Joint angles", icon: "ruler", nodes: (skeleton?.joints ?? []).map((joint, index) => ({
      selection: { kind: "joint", index }, label: joint.label, detail: formatAngle(sampled.joints[index]?.value),
    })) },
    { label: "Cameras", icon: "camera", expanded: true, nodes: cameras.map(({ descriptor, state }) => ({
      selection: { kind: "camera", id: descriptor.id }, label: descriptor.id, detail: humanizeId(state),
    })) },
    { label: "Sensors", icon: "usb", nodes: sensors.map(({ descriptor, state }) => ({
      selection: { kind: "sensor", id: descriptor.id }, label: descriptor.id, detail: humanizeId(state),
    })) },
  ];

  return (
    <div className="workbench-workspace">
      <header className="workspace-heading workbench-heading">
        <h1>Motion Studio</h1>
        <span className="mono subtle">{skeleton?.model_id ?? "Awaiting skeleton"}</span>
      </header>
      <DockLayout workspace="studio"
        toolbar={(showInspector) => <>
          <div className="tool-group" role="group" aria-label="View orientation">
            {VIEWS.map(([preset, key]) => (
              <button type="button" key={preset} aria-pressed={view.preset === preset}
                aria-keyshortcuts={key} title={`${preset} view (${key})`} onClick={() => setPreset(preset)}>
                {preset === "3D" && <Icon name="rotate" size={14} />}{preset}
              </button>
            ))}
          </div>
          <div className="tool-group" role="group" aria-label="Inspection and calibration">
            <button type="button" className="icon-control" aria-label="Inspect joint angles"
              title={skeleton?.joints.length ? "Inspect joint angles" : "No joint measurements"}
              disabled={!skeleton?.joints.length} onClick={() => {
                if (selection?.kind !== "joint") select({ kind: "joint", index: 0 });
                showInspector();
              }}><Icon name="ruler" /></button>
            <button type="button" className="icon-control" aria-label="Open mounting calibration"
              title="Calibrate mounting" disabled={!connected || !!state?.playback} onClick={onCalibrate}><Icon name="crosshair" /></button>
          </div>
          <div className="tool-group" role="group" aria-label="Viewport layers">
            <button type="button" className="icon-control" aria-label="Skeleton" title="Skeleton layer"
              aria-pressed={skeletonVisible} onClick={() => setSkeletonVisible(!skeletonVisible)}><Icon name="layers" /></button>
            <button type="button" className="icon-control" aria-label="Floor grid" title="Floor grid"
              aria-pressed={floorVisible} onClick={() => setFloorVisible(!floorVisible)}><Icon name="grid" /></button>
            <button type="button" className="icon-control" aria-label="Camera frustums" title={poses.length ? "Camera frustums" : "No calibrated cameras"}
              aria-pressed={camerasVisible && poses.length > 0} disabled={!poses.length}
              onClick={() => setCamerasVisible(!camerasVisible)}><Icon name="camera" /></button>
          </div>
        </>}
        tools={<>
          <div className="dock-tool-section">
            <h3>Layers</h3>
            <label><input type="checkbox" checked={skeletonVisible} onChange={(e) => setSkeletonVisible(e.target.checked)} /> Skeleton</label>
            <label><input type="checkbox" checked={floorVisible} onChange={(e) => setFloorVisible(e.target.checked)} /> Floor grid</label>
            <label title={poses.length ? undefined : "No calibrated cameras"}>
              <input type="checkbox" checked={camerasVisible && poses.length > 0} disabled={!poses.length}
                onChange={(e) => setCamerasVisible(e.target.checked)} /> Cameras <span className="subtle mono">{poses.length}</span>
            </label>
          </div>
          <div className="dock-tool-section">
            <h3>Actions</h3>
            <button type="button" className="tool-action" onClick={onCalibrate} disabled={!connected || !!state?.playback}>
              <Icon name="crosshair" size={14} /> Calibrate mounting
            </button>
            <a className="tool-action" href="#/technique"><Icon name="target" size={14} /> Compare technique</a>
            <a className="tool-action" href="#/cameras"><Icon name="video" size={14} /> Camera capture</a>
          </div>
        </>}
        viewport={<section className={`viewport-panel ${cameraOnly ? "camera-only-viewport" : ""}`} aria-label="Motion viewport">
          <ViewportCanvas skeleton={skeleton} selection={selected} onSelect={select} view={view.preset}
            viewRevision={view.revision} skeletonVisible={skeletonVisible} floorVisible={floorVisible}
            cameras={camerasVisible ? poses : undefined} />
          {cameras.length > 0 && <CameraInset cameras={cameras} main={cameraOnly} selected={activeCamera}
            onSelect={(id) => select({ kind: "camera", id })} />}
          <div className="viewport-top viewport-hud">
            <div>
              <span className="viewport-mode mono">{view.preset === "3D" ? "Perspective" : view.preset}</span>
              {state?.engine.mode === "replay" && <OriginBadge label="replay" />}
              {[...new Set(sampled.origins.filter((origin) => origin !== "live"))]
                .map((origin) => <OriginBadge key={origin} label={origin} />)}
            </div>
          </div>
          {!sampled.hasPose && (
            <div className="no-pose">
              <Icon name="focus" />
              <h2>No pose data</h2>
              {connected ? <Reasons reasons={featureReasons(state, "skeleton_pose")} />
                : <p>Waiting for a live engine connection.</p>}
            </div>
          )}
          <div className="viewport-bottom">
            <div className="viewport-readouts">
              <span>Coverage <strong className="mono">{sampled.coverage}</strong></span>
              <span>Pose age <strong className="mono">{formatMs(sampled.ageMs)}</strong></span>
            </div>
            <span className="axis-key mono">Y-up · metres</span>
          </div>
          <div className="viewport-help">Drag to orbit <span>·</span> Scroll to zoom</div>
        </section>}
        explorer={<SceneExplorer groups={groups} selection={selected} onSelect={select} />}
        properties={selectedDevice?.descriptor.kind === "camera" ? <CameraProperties camera={selectedDevice} />
          : selectedDevice ? <SensorProperties sensor={selectedDevice} />
          : <MotionProperties key={selected?.kind + (selected && "index" in selected ? `:${selected.index}` : "")}
            selection={selected} skeleton={skeleton} sampled={sampled} devices={devices} />}
      />
    </div>
  );
}

function MotionProperties({ selection, skeleton, sampled, devices }: {
  selection: SceneSelection | null;
  skeleton: SkeletonDefinition | null;
  sampled: Sampled;
  devices: DeviceStatus[];
}) {
  const jointIndex = selection?.kind === "joint" ? selection.index : -1;
  const joint = skeleton?.joints[jointIndex];
  const jointValue = sampled.joints[jointIndex];
  const segmentIndex = selection?.kind === "segment" ? selection.index : -1;
  const segment = skeleton?.segments[segmentIndex];
  const segmentValue = sampled.segments[segmentIndex];
  if (joint && skeleton) {
    const history = jointHistory.valuesOf(jointIndex);
    const sources = new Set(joint.segments.flatMap((id) =>
      sampled.segments[skeleton.segments.findIndex((item) => item.id === id)]?.sources ?? []));
    return (
      <div className="inspector-content">
        <span className="eyebrow">Joint measurement</span>
        <h3>{joint.label}</h3>
        <div className="large-readout mono" data-testid="selected-angle">{formatAngle(jointValue?.value)}</div>
        <div className="readout-meta">
          <ProvenanceTag provenance={jointValue?.provenance ?? "unavailable"} />
          <span className="mono">conf {formatConfidence(jointValue?.confidence)}</span>
        </div>
        {!joint.validated && <span className="validation-note">Not validated</span>}
        <dl className="detail-list"><dt>Input sources</dt><dd className="mono">{[...sources].join(", ") || "—"}</dd></dl>
        <Sparkline series={history && sampled.seq >= 0 ? {
          times: jointHistory.times, values: history, head: jointHistory.head, count: jointHistory.count,
        } : null} windowUs={TRACE_WINDOW_US} version={sampled.seq} label={`${joint.label}, last ten seconds`} />
        <div className="trace-caption"><span>Rolling trace</span><span className="mono">10 s</span></div>
        <Reasons reasons={jointValue?.reasons ?? []} />
      </div>
    );
  }
  if (!segment) return <p className="empty-inline">Select an object in the scene explorer or viewport.</p>;
  const device = devices.find((d) => d.descriptor.id === segmentValue?.sources[0]);
  return (
    <div className="inspector-content">
      <span className="eyebrow">Body segment</span>
      <h3>{humanizeId(segment.id)}</h3>
      <div className="segment-provenance"><ProvenanceTag provenance={segmentValue?.provenance ?? "unavailable"} /></div>
      <dl className="detail-list">
        <dt>Sources</dt><dd className="mono">{segmentValue?.sources.join(", ") || "—"}</dd>
        <dt>Confidence</dt><dd className="mono">{formatConfidence(segmentValue?.confidence)}</dd>
        <dt>Device state</dt><dd>{device ? <StatePill state={device.state} /> : "—"}</dd>
        <dt>Mounting</dt><dd>{device?.calibration.find((c) => c.kind === "mounting")?.state ?? "—"}</dd>
        <dt>Heading</dt><dd>{device?.calibration.find((c) => c.kind === "heading_alignment")?.state ?? "—"}</dd>
      </dl>
      {segmentValue?.provenance === "inferred" && <p className="subtle">
        Inferred at the parent-relative rest pose. Capture mounting calibration to use tracker orientation.
      </p>}
    </div>
  );
}

function SensorProperties({ sensor }: { sensor: DeviceStatus }) {
  return (
    <div className="inspector-content">
      <span className="eyebrow">Sensor</span>
      <h3 className="mono">{sensor.descriptor.id}</h3>
      <div className="readout-meta"><StatePill state={sensor.state} /><OriginBadge label={sensor.descriptor.origin} /></div>
      <dl className="detail-list">
        <dt>Assignment</dt><dd>{sensor.assignment ? humanizeId(sensor.assignment) : "—"}</dd>
        <dt>Measured rate</dt><dd className="mono">{sensor.stream.measured_rate_hz.toFixed(1)} Hz</dd>
        {sensor.calibration.map((entry) => (
          <div key={entry.kind} className="detail-row"><dt>{humanizeId(entry.kind)}</dt><dd>{humanizeId(entry.state)}</dd></div>
        ))}
      </dl>
      <Reasons reasons={sensor.reasons} />
      <a href="#/devices">Manage devices</a>
    </div>
  );
}
