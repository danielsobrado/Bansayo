import { useMemo, useState } from "react";
import type { CSSProperties } from "react";
import { Icon } from "../../ui/Icon";
import { SceneExplorer } from "./SceneExplorer";
import { DockDivider, useDockWidths, type LayoutPreset } from "./useDockWidths";
import type { SkeletonDefinition, SystemState } from "@contracts";
import { calibratedCameras } from "../../engine/cameraCalibration";
import { readouts } from "../../engine/readouts";
import { jointHistory, TRACE_WINDOW_US } from "../../engine/jointHistory";
import { featureReasons, segmentRows } from "../../engine/selectors";
import {
  formatAngle,
  formatConfidence,
  formatMs,
  humanizeId,
} from "../../lib/format";
import { useShortcuts } from "../../lib/shortcuts";
import { OriginBadge } from "../../ui/OriginBadge";
import { ProvenanceTag } from "../../ui/ProvenanceTag";
import { Reasons } from "../../ui/Reasons";
import { Sparkline } from "../../ui/Sparkline";
import { StatePill } from "../../ui/StatePill";
import { ViewportCanvas } from "../../viewport/ViewportCanvas";
import { CameraInset } from "../cameras/Cameras";
import type { ViewPreset } from "../../viewport/MotionViewport";

interface Props {
  skeleton: SkeletonDefinition | null;
  sampled: ReturnType<typeof readouts>;
  state: SystemState | null;
  onCalibrate: () => void;
  connected: boolean;
}
const VIEWS: ViewPreset[] = ["Front", "Side", "Top", "3D"];
export function Studio({
  skeleton,
  sampled,
  state,
  onCalibrate,
  connected,
}: Props) {
  const [selectedSegment, setSelectedSegment] = useState(0);
  const [selectedJoint, setSelectedJoint] = useState<number | null>(null);
  const [view, setView] = useState<ViewPreset>("3D");
  const [skeletonVisible, setSkeletonVisible] = useState(true);
  const [floorVisible, setFloorVisible] = useState(true);
  const [camerasVisible, setCamerasVisible] = useState(false);
  const { layout, resize, setPreset } = useDockWidths();
  const calibration = state?.calibration;
  const poses = useMemo(
    () => calibratedCameras(calibration ?? []),
    [calibration],
  );
  const [leftOpen, setLeftOpen] = useState(false);
  const [rightOpen, setRightOpen] = useState(false);
  const cameras =
    state?.devices.filter((d) => d.descriptor.kind === "camera") ?? [];
  const cameraOnly =
    cameras.length > 0 &&
    state?.features.find((f) => f.id === "skeleton_pose")?.state ===
      "unavailable";
  const rows = useMemo(() => segmentRows(skeleton), [skeleton]);
  const segment = skeleton?.segments[selectedSegment];
  const segmentValue = sampled.segments[selectedSegment];
  const joint = selectedJoint !== null ? skeleton?.joints[selectedJoint] : null;
  const jointValue =
    selectedJoint !== null ? sampled.joints[selectedJoint] : null;
  const device = state?.devices.find(
    (d) => d.descriptor.id === segmentValue?.sources[0],
  );
  const history =
    selectedJoint !== null ? jointHistory.valuesOf(selectedJoint) : null;
  const selectSegment = (index: number) => {
    setSelectedSegment(index);
    setSelectedJoint(null);
  };
  useShortcuts({
    f: () => setView("Front"),
    l: () => setView("Side"),
    t: () => setView("Top"),
    p: () => setView("3D"),
    Home: () => setView("3D"),
    Escape: () => {
      setLeftOpen(false);
      setRightOpen(false);
    },
  });
  return (
    <div className="studio-workspace">
      <header className="workspace-heading">
        <div>
          <span className="eyebrow">Workspace / Studio</span>
          <h1>Motion studio</h1>
        </div>
        <div className="workspace-tools">
          <span className="mono skeleton-model">
            {skeleton?.model_id ?? "Awaiting skeleton"}
          </span>
          <button
            className="layers-toggle"
            aria-expanded={leftOpen}
            onClick={() => setLeftOpen(!leftOpen)}
          >
            Layers & segments
          </button>
          <button
            className="inspector-toggle"
            aria-expanded={rightOpen}
            onClick={() => setRightOpen(!rightOpen)}
          >
            Inspector
          </button>
          <button onClick={onCalibrate} disabled={!connected}>
            Calibrate mounting
          </button>
        </div>
      </header>
      <div className="studio-toolstrip" role="toolbar" aria-label="Motion viewport tools">
        {VIEWS.map((preset) => (
          <button type="button" key={preset} aria-pressed={view === preset}
            title={`${preset} camera view`} onClick={() => setView(preset)}>
            <Icon name={preset === "3D" ? "rotate" : "monitor"} size={14} />
            {preset}
          </button>
        ))}
        <span className="tool-separator" aria-hidden="true" />
        <button type="button" aria-pressed={floorVisible} onClick={() => setFloorVisible(!floorVisible)}
          title="Show or hide floor grid"><Icon name="grid" size={14} /> Floor</button>
        <button type="button" aria-pressed={camerasVisible} onClick={() => setCamerasVisible(!camerasVisible)}
          title="Show or hide calibrated cameras"><Icon name="camera" size={14} /> Cameras</button>
        <label className="studio-layout-select">
          Layout
          <select aria-label="Studio layout preset" value={layout.preset}
            onChange={(event) => setPreset(event.target.value as LayoutPreset)}>
            <option value="analysis">Analysis</option>
            <option value="capture">Capture</option>
            <option value="focus">Focus</option>
          </select>
        </label>
      </div>
      <div className={`studio-grid workbench-studio-grid ${layout.preset === "focus" ? "focus-layout" : ""}`}
        style={{ "--dock-left": `${layout.left}px`, "--dock-right": `${layout.right}px` } as CSSProperties}>
        {(leftOpen || rightOpen) && (
          <button
            className={`panel-backdrop ${leftOpen ? "left-backdrop" : ""}`}
            onClick={() => {
              setLeftOpen(false);
              setRightOpen(false);
            }}
            aria-label="Close panels"
          />
        )}
        <aside
          className={`segment-panel dock ${leftOpen ? "sheet-open" : ""}`}
          aria-label="Layers and segments"
        >
          <div className="panel-heading">
            LAYERS
            <button
              className="sheet-close"
              aria-label="Close layers"
              onClick={() => setLeftOpen(false)}
            >
              ×
            </button>
          </div>
          <div className="layer-controls">
            <label>
              <input
                type="checkbox"
                checked={skeletonVisible}
                onChange={(e) => setSkeletonVisible(e.target.checked)}
              />
              Skeleton
            </label>
            <label>
              <input
                type="checkbox"
                checked={floorVisible}
                onChange={(e) => setFloorVisible(e.target.checked)}
              />
              Floor grid<span className="subtle mono">0.5 m</span>
            </label>
            <label>
              <input
                type="checkbox"
                checked={camerasVisible}
                onChange={(e) => setCamerasVisible(e.target.checked)}
              />
              Cameras
              <span className="subtle mono">{poses.length} calibrated</span>
            </label>
          </div>
          <div className="panel-heading segment-heading">
            SEGMENTS{" "}
            <span className="mono">{skeleton?.segments.length ?? "—"}</span>
          </div>
          <div className="segments-list">
            {rows.map((row) => {
              const value = sampled.segments[row.index];
              return (
                <button
                  key={row.id}
                  aria-pressed={
                    selectedJoint === null && selectedSegment === row.index
                  }
                  className={`segment-row ${selectedJoint === null && selectedSegment === row.index ? "selected" : ""}`}
                  data-depth={Math.min(row.depth, 3)}
                  onClick={() => selectSegment(row.index)}
                >
                  <span
                    className="segment-glyph"
                    data-prov={value?.provenance ?? "unavailable"}
                  >
                    {value?.provenance === "derived" ||
                    value?.provenance === "measured"
                      ? "●"
                      : "◌"}
                  </span>
                  <span className="segment-name">{humanizeId(row.id)}</span>
                  <ProvenanceTag
                    provenance={value?.provenance ?? "unavailable"}
                    short
                  />
                </button>
              );
            })}
            {!skeleton && (
              <p className="empty-inline">
                Waiting for the engine’s skeleton definition.
              </p>
            )}
          </div>
          <div className="segment-footer">
            <span className="eyebrow">Coordinate frame</span>
            <span className="mono">+Y up · +Z forward · m</span>
          </div>
        </aside>
        <DockDivider side="left" width={layout.left} onResize={resize} />
        <section
          className={`viewport-panel ${cameraOnly ? "camera-only-viewport" : ""}`}
          aria-label="Motion viewport"
        >
          <ViewportCanvas
            skeleton={skeleton}
            selected={selectedJoint === null ? selectedSegment : -1}
            onSelect={selectSegment}
            view={view}
            skeletonVisible={skeletonVisible}
            floorVisible={floorVisible}
            cameras={camerasVisible ? poses : undefined}
          />
          {cameras.length > 0 && (
            <CameraInset cameras={cameras} main={cameraOnly} />
          )}
          <div className="viewport-top">
            <div>
              {state?.engine.mode === "replay" && (
                <OriginBadge label="replay" />
              )}
              {[...new Set(sampled.origins.filter((o) => o !== "live"))].map(
                (origin) => (
                  <OriginBadge key={origin} label={origin} />
                ),
              )}
            </div>
            <div className="view-presets" aria-label="View presets">
              {VIEWS.map((preset) => (
                <button
                  key={preset}
                  aria-pressed={view === preset}
                  onClick={() => setView(preset)}
                  title={`${preset} view (${preset === "Front" ? "F" : preset === "Side" ? "L" : preset === "Top" ? "T" : "P"})`}
                >
                  {preset}
                </button>
              ))}
            </div>
          </div>
          {!sampled.hasPose && (
            <div className="no-pose">
              <span aria-hidden="true">◇</span>
              <h2>No pose data</h2>
              {connected ? (
                <Reasons reasons={featureReasons(state, "skeleton_pose")} />
              ) : (
                <p>Waiting for a live engine connection.</p>
              )}
            </div>
          )}
          <div className="viewport-bottom">
            <div className="viewport-readouts">
              <span>
                Coverage <strong className="mono">{sampled.coverage}</strong>
              </span>
              <span>
                Pose age{" "}
                <strong className="mono">{formatMs(sampled.ageMs)}</strong>
              </span>
            </div>
            <div className="axis-key mono">
              <span>Y ↑</span>
              <span>X ↗</span>
              <span>Z →</span>
            </div>
          </div>
          <div className="viewport-help">
            Drag to orbit <span>·</span> Scroll to zoom <span>·</span> Select a
            segment to inspect
          </div>
        </section>
        <DockDivider side="right" width={layout.right} onResize={resize} />
        <aside
          className={`inspector-panel dock ${rightOpen ? "sheet-open" : ""}`}
          aria-label="Inspector"
        >
          <SceneExplorer rows={rows} cameras={cameras} selectedSegment={selectedSegment}
            onSelectSegment={selectSegment} />
          <div className="studio-properties">
          <div className="panel-heading">
            INSPECTOR
            <button
              className="sheet-close"
              aria-label="Close inspector"
              onClick={() => setRightOpen(false)}
            >
              ×
            </button>
          </div>
          <div className="inspector-content">
            <span className="eyebrow">
              {joint ? "Joint measurement" : "Body segment"}
            </span>
            <h2>
              {joint?.label ??
                (segment ? humanizeId(segment.id) : "Select a segment")}
            </h2>
            {joint ? (
              <>
                <div
                  className="large-readout mono"
                  data-testid="selected-angle"
                >
                  {formatAngle(jointValue?.value)}
                </div>
                <div className="readout-meta">
                  <ProvenanceTag
                    provenance={jointValue?.provenance ?? "unavailable"}
                  />
                  <span className="mono">
                    conf {formatConfidence(jointValue?.confidence)}
                  </span>
                </div>
                {!joint.validated && (
                  <span className="validation-note">Not validated</span>
                )}
                <Sparkline
                  series={
                    history && sampled.seq >= 0
                      ? {
                          times: jointHistory.times,
                          values: history,
                          head: jointHistory.head,
                          count: jointHistory.count,
                        }
                      : null
                  }
                  windowUs={TRACE_WINDOW_US}
                  version={sampled.seq}
                  label={`${joint.label}, last ten seconds`}
                />
                <div className="trace-caption">
                  <span>Rolling trace</span>
                  <span className="mono">10 s</span>
                </div>
                <Reasons reasons={jointValue?.reasons ?? []} />
              </>
            ) : (
              <>
                <div className="segment-provenance">
                  <ProvenanceTag
                    provenance={segmentValue?.provenance ?? "unavailable"}
                  />
                </div>
                <dl className="detail-list">
                  <dt>Source</dt>
                  <dd className="mono">
                    {segmentValue?.sources.join(", ") || "—"}
                  </dd>
                  <dt>Confidence</dt>
                  <dd className="mono">
                    {formatConfidence(segmentValue?.confidence)}
                  </dd>
                  <dt>Device state</dt>
                  <dd>{device ? <StatePill state={device.state} /> : "—"}</dd>
                  <dt>Mounting</dt>
                  <dd>
                    {device?.calibration.find((c) => c.kind === "mounting")
                      ?.state ?? "—"}
                  </dd>
                  <dt>Heading</dt>
                  <dd>
                    {device?.calibration.find(
                      (c) => c.kind === "heading_alignment",
                    )?.state ?? "—"}
                  </dd>
                </dl>
                {segmentValue?.provenance === "inferred" && (
                  <p className="subtle">
                    Inferred at the parent-relative rest pose. Capture mounting
                    calibration to use tracker orientation.
                  </p>
                )}
              </>
            )}
          </div>
          <div className="panel-heading">
            JOINT ANGLES <span className="mono">rad → °</span>
          </div>
          <div className="joint-table" role="table" aria-label="Joint angles">
            <div className="joint-table-heading" role="row">
              <span role="columnheader">Measurement</span>
              <span role="columnheader">Degrees</span>
            </div>
            {skeleton?.joints.map((definition, index) => {
              const value = sampled.joints[index];
              return (
                <div role="row" key={definition.id}>
                  <div role="cell">
                    <button
                      aria-label={`Inspect ${definition.label}`}
                      className={`joint-row ${selectedJoint === index ? "selected" : ""}`}
                      aria-pressed={selectedJoint === index}
                      onClick={() => {
                        setSelectedJoint(index);
                        setRightOpen(true);
                      }}
                      title={
                        value?.reasons.map((r) => r.message).join(" ") ||
                        definition.label
                      }
                    >
                      <span>{definition.label}</span>
                      <span
                        className="mono joint-value"
                        data-prov={value?.provenance}
                      >
                        {formatAngle(value?.value)}
                      </span>
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
          <p className="measurement-note">
            Derived from segment rotations.
            <br />
            Joint angles are not clinically validated.
          </p>
          </div>
        </aside>
      </div>
    </div>
  );
}
