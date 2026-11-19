import { useMemo, useState } from "react";
import type {
  DeviceStatus,
  FeatureStatus,
  LandmarkSchema,
  SystemState,
} from "@contracts";
import { CameraPreview } from "./CameraPreview";
import { CameraProperties } from "./CameraProperties";
import type { SceneSelection } from "../../lib/sceneSelection";
import { DockLayout } from "../../shell/DockLayout";
import { SceneExplorer, type SceneGroup } from "../../shell/SceneExplorer";
import { Icon } from "../../ui/Icon";
import {
  calibratedCameras,
  roomsOf,
  type CameraPose,
} from "../../engine/cameraCalibration";
import { actions } from "../../engine/actions";
import { useAction, useSampled } from "../../engine/hooks";
import {
  HIGH_REPROJECTION_PX,
  reconstructionReadout,
  type CameraView,
  type LandmarkStatus,
} from "../../engine/readouts";
import {
  freshLandmarks,
  landmarks3dStore,
  STALE_FRAME_MS,
} from "../../engine/stores";
import { formatConfidence, humanizeId } from "../../lib/format";
import { EmptyState } from "../../ui/EmptyState";
import { OriginBadge } from "../../ui/OriginBadge";
import { StatePill } from "../../ui/StatePill";
import { Reasons } from "../../ui/Reasons";
import { ViewportCanvas } from "../../viewport/ViewportCanvas";

type Readout = ReturnType<typeof reconstructionReadout>;

/** Glyph and word, so status is never colour alone. */
const STATUS: Record<LandmarkStatus, [string, string]> = {
  accepted: ["●", "accepted"],
  high_error: ["◇", "high error"],
  not_reconstructed: ["○", "not reconstructed"],
};
const ROLE: Record<CameraView["role"], string> = {
  used: "used",
  rejected: "rejected",
  not_used: "not used",
};

export function Cameras({ state }: { state: SystemState | null }) {
  const cameras =
    state?.devices.filter((d) => d.descriptor.kind === "camera") ?? [];
  const [selected, setSelected] = useState<string | null>(null);
  const [selection, setSelection] = useState<SceneSelection | null>(null);
  const [viewMode, setViewMode] = useState<"split" | "camera" | "grid" | "reconstruction">("split");
  const [showAprilTags, setShowAprilTags] = useState(false);
  const [floorVisible, setFloorVisible] = useState(true);
  const landmark = selection?.kind === "landmark" ? selection.index : null;
  const select = (target: SceneSelection) => {
    if (target.kind === "camera") setSelected(target.id);
    setSelection(target);
  };
  const camera =
    cameras.find((d) => d.descriptor.id === selected) ?? cameras[0];
  const visibleFeeds = viewMode === "grid" ? cameras : camera && viewMode !== "reconstruction" ? [camera] : [];
  const canShowAprilTags = visibleFeeds.some(({ descriptor }) => descriptor.origin !== "simulated");
  const calibration = state?.calibration;
  const room = state?.active_room ?? null;
  const poses = useMemo(
    () => calibratedCameras(calibration ?? [], room),
    [calibration, room],
  );
  const rooms = roomsOf(calibration ?? [], room);
  const [switchRoom, roomStatus] = useAction(actions.selectRoom);
  const rawSchema =
    camera?.descriptor.camera?.landmark_schema ??
    cameras.find((c) => c.descriptor.camera?.landmark_schema)?.descriptor
      .camera?.landmark_schema ??
    null;
  // State is a new object every second; key the schema by content so the 3D view is not rebuilt.
  const schemaKey = JSON.stringify(rawSchema);
  const schema = useMemo(() => rawSchema, [schemaKey]);
  const cameraIds = cameras.map((c) => c.descriptor.id);
  const sampled = useSampled(
    4,
    () => {
      const now = performance.now();
      const set = landmarks3dStore.landmarks;
      return reconstructionReadout(
        set && now - landmarks3dStore.receivedAt <= STALE_FRAME_MS ? set : null,
        freshLandmarks(now),
        schema,
        cameraIds,
        landmark,
      );
    },
    `${landmark}/${schemaKey}`,
  );
  const activeSelection = selection?.kind === "landmark" && schema?.names[selection.index] !== undefined
    ? selection
    : camera ? { kind: "camera" as const, id: camera.descriptor.id } : null;
  const groups: SceneGroup[] = [
    { label: "Cameras", icon: "camera", expanded: true, nodes: cameras.map(({ descriptor, state }) => ({
      selection: { kind: "camera", id: descriptor.id }, label: descriptor.id, detail: humanizeId(state),
    })) },
    { label: "Landmarks", icon: "crosshair", nodes: (schema?.names ?? []).map((name, index) => {
      const status = sampled.statuses?.[index];
      return { selection: { kind: "landmark", index }, label: humanizeId(name),
        detail: status ? STATUS[status].join(" ") : "Waiting" };
    }) },
  ];
  return (
    <div className="workbench-workspace">
      <header className="workspace-heading workbench-heading">
        <h1>Cameras</h1>
        <span className="mono subtle">{cameras.length} sources · {poses.length} calibrated</span>
      </header>
      {camera ? (
        <DockLayout workspace="cameras"
          toolbar={() => <>
            <div className="tool-group" role="group" aria-label="Camera workspace view">
              <button type="button" aria-pressed={viewMode === "split"} onClick={() => setViewMode("split")}>
                <Icon name="grid" size={14} /> Split
              </button>
              <button type="button" aria-pressed={viewMode === "camera"} onClick={() => setViewMode("camera")}>
                <Icon name="video" size={14} /> Camera
              </button>
              <button type="button" aria-pressed={viewMode === "grid"} onClick={() => setViewMode("grid")}>
                <Icon name="layers" size={14} /> Grid
              </button>
              <button type="button" aria-pressed={viewMode === "reconstruction"} onClick={() => setViewMode("reconstruction")}>
                <Icon name="rotate" size={14} /> 3D
              </button>
            </div>
            <select aria-label="Active camera" value={camera.descriptor.id}
              onChange={(event) => select({ kind: "camera", id: event.target.value })}>
              {cameras.map(({ descriptor }) => <option key={descriptor.id}>{descriptor.id}</option>)}
            </select>
            <select aria-label="Room" title={roomStatus.error ?? "Room: which camera placement is in use"}
              value={room ?? ""} disabled={roomStatus.pending || state?.engine.mode === "replay"}
              onChange={(event) => void switchRoom({ active_room: event.target.value || null })}>
              {rooms.map((r) => <option key={r ?? ""} value={r ?? ""}>{r ? `Room: ${r}` : "Room: default"}</option>)}
            </select>
          </>}
          tools={<div className="dock-tool-section">
            <h3>Overlays</h3>
            <label title={canShowAprilTags ? undefined : "AprilTag markers require a visible physical camera"}>
              <input type="checkbox" checked={showAprilTags && canShowAprilTags}
                disabled={!canShowAprilTags}
                onChange={(event) => setShowAprilTags(event.target.checked)} /> AprilTag markers
            </label>
            <label><input type="checkbox" checked={floorVisible} onChange={(event) => setFloorVisible(event.target.checked)} /> 3D floor grid</label>
            <a className="tool-action" href="#/studio"><Icon name="crosshair" size={14} /> Motion Studio</a>
          </div>}
          viewport={<div className={`capture-views capture-mode-${viewMode}`}>
            {visibleFeeds.map((device) => {
              const id = device.descriptor.id;
              const active = id === camera.descriptor.id;
              return <section key={id} className="capture-feed" aria-label={`2D camera view ${id}`}>
                <header className="reconstruction-heading">
                  <h2>Camera feed</h2>
                  <button type="button" className="mono feed-camera" aria-pressed={activeSelection?.kind === "camera" && active}
                    onClick={() => select({ kind: "camera", id })}>{id}</button>
                </header>
                <CameraPreview device={device} highlight={active ? landmark : null}
                  showAprilTags={showAprilTags && device.descriptor.origin !== "simulated"}
                  onSelect={(index) => {
                    setSelected(id);
                    select({ kind: "landmark", index });
                  }} />
              </section>;
            })}
            <div className="capture-3d" hidden={viewMode !== "split" && viewMode !== "reconstruction"}>
              <Reconstruction
                feature={state?.features.find((f) => f.id === "landmarks_3d_triangulated")}
                poses={poses} cameraCount={cameras.length} schema={schema}
                sampled={sampled} selection={activeSelection} onSelect={select}
                floorVisible={floorVisible} active={viewMode === "split" || viewMode === "reconstruction"} />
            </div>
          </div>}
          explorer={<SceneExplorer groups={groups} selection={activeSelection} onSelect={select} />}
          properties={activeSelection?.kind === "landmark"
            ? <LandmarkInspector key={activeSelection.index} inspection={sampled.inspection} />
            : <CameraProperties key={camera.descriptor.id} camera={camera} />}
        />
      ) : (
        <EmptyState
          title={state ? "No cameras configured" : "Waiting for the engine"}
        >
          <p>
            Add a camera source to the configuration file (cameras-two.yaml for
            USB cameras), or run sim-camera-only.yaml. Each camera gives 2D
            landmarks; metric 3D needs two or more calibrated cameras.
          </p>
        </EmptyState>
      )}
    </div>
  );
}

function Reconstruction({
  feature,
  poses,
  cameraCount,
  schema,
  sampled,
  selection,
  onSelect,
  floorVisible,
  active,
}: {
  feature: FeatureStatus | undefined;
  poses: CameraPose[];
  cameraCount: number;
  schema: LandmarkSchema | null;
  sampled: Readout;
  selection: SceneSelection | null;
  onSelect: (selection: SceneSelection) => void;
  floorVisible: boolean;
  active: boolean;
}) {
  return (
    <section className="reconstruction" aria-label="3D reconstruction">
      <header className="reconstruction-heading">
        <h2>3D reconstruction</h2>
        {feature && <StatePill state={feature.state} />}
      </header>
      <div className="reconstruction-view">
        {poses.length >= 2 && schema ? (
          <>
            <ViewportCanvas
              skeleton={null}
              landmarkSchema={schema}
              selection={selection}
              onSelect={onSelect}
              view="3D"
              skeletonVisible={false}
              floorVisible={floorVisible}
              cameras={poses}
              active={active}
            />
            <div className="viewport-top">
              <div>
                {sampled.origin && <OriginBadge label={sampled.origin} />}
              </div>
            </div>
            {!sampled.live && (
              <div className="no-pose">
                <Icon name="focus" />
                <h2>Waiting for triangulated landmarks</h2>
              </div>
            )}
          </>
        ) : (
          <EmptyState
            title={
              schema
                ? "3D reconstruction needs two calibrated cameras"
                : "Cameras report no landmark schema"
            }
          >
            <p>
              {poses.length} of {cameraCount} cameras report usable intrinsics
              and extrinsics. Metric 3D landmarks are triangulated only from two
              or more calibrated, synchronised cameras; one camera gives 2D
              image measurements only.
            </p>
          </EmptyState>
        )}
      </div>
      <div className="camera-preview-label">
        <span className="mono">
          {sampled.live
            ? `${sampled.reconstructed} of ${sampled.total} reconstructed`
            : "No live 3D set"}
        </span>
        <span>● opacity and size = confidence</span>
        <span>◇ wire = reprojection error &gt; {HIGH_REPROJECTION_PX} px</span>
      </div>
      <div className="reconstruction-reasons">
        <Reasons reasons={feature?.reasons ?? []} />
      </div>
    </section>
  );
}

function LandmarkInspector({
  inspection,
}: {
  inspection: Readout["inspection"];
}) {
  return (
    <>
      {inspection ? (
        <div className="inspector-content landmark-inspector">
          <span className="eyebrow">Landmark</span>
          <h3>{humanizeId(inspection.name)}</h3>
          <p data-testid="landmark-status">
            {inspection.status === "high_error"
              ? `◇ accepted · reprojection RMS above ${HIGH_REPROJECTION_PX} px`
              : inspection.status
                ? STATUS[inspection.status].join(" ")
                : "Waiting for a live 3D set"}
          </p>
          {inspection.status === "not_reconstructed" && (
            <p className="subtle">
              Too few views or a failed reprojection check. See the camera observations below.
            </p>
          )}
          <dl className="detail-list">
            <dt>Position x y z</dt>
            <dd className="mono">
              {inspection.point
                ? `${inspection.point.m.map((v) => v.toFixed(3)).join(" ")} m`
                : "—"}
            </dd>
            <dt>Views used</dt>
            <dd className="mono">{inspection.point?.views ?? "—"}</dd>
            <dt>Reprojection RMS</dt>
            <dd className="mono">
              {inspection.point
                ? `${inspection.point.reprojection_error_px.toFixed(2)} px`
                : "—"}
            </dd>
            <dt>Confidence</dt>
            <dd className="mono">
              {formatConfidence(inspection.point?.confidence)}
            </dd>
          </dl>
          <h3 className="eyebrow">Camera · role · 2D observation</h3>
          <ul className="landmark-views">
            {inspection.cameras.map((view) => (
              <li key={view.id} data-role={view.role}>
                <span className="mono">{view.id}</span>
                <span>{ROLE[view.role]}</span>
                <span className="mono">
                  {view.observation === undefined
                    ? "no 2D data"
                    : view.observation === null
                      ? "not detected"
                      : `${view.observation.x_px.toFixed(1)}, ${view.observation.y_px.toFixed(1)} px · ${formatConfidence(view.observation.confidence)}`}
                </span>
              </li>
            ))}
          </ul>
        </div>
      ) : (
        <p className="empty-inline">
          Select a landmark in the viewport or scene explorer.
        </p>
      )}
    </>
  );
}

function CameraTabs({
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
  selected,
  onSelect,
}: {
  cameras: DeviceStatus[];
  main?: boolean;
  selected: string | null;
  onSelect: (id: string) => void;
}) {
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
            <Icon name={expanded ? "focus" : "expand"} size={14} />
          </button>
        )}
      </header>
      <CameraPreview device={camera} />
      {cameras.length > 1 && (
        <CameraTabs
          cameras={cameras}
          selected={camera.descriptor.id}
          onSelect={onSelect}
        />
      )}
    </div>
  );
}
