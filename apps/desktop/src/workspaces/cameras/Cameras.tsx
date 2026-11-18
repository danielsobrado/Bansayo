import { useMemo, useState } from "react";
import type {
  DeviceStatus,
  FeatureStatus,
  LandmarkSchema,
  SystemState,
} from "@contracts";
import { CameraPreview } from "./CameraPreview";
import { Icon } from "../../ui/Icon";
import {
  calibratedCameras,
  type CameraPose,
} from "../../engine/cameraCalibration";
import { useSampled } from "../../engine/hooks";
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
  const [landmark, setLandmark] = useState<number | null>(null);
  const [viewMode, setViewMode] = useState<"split" | "camera" | "reconstruction">("split");
  const [showAprilTags, setShowAprilTags] = useState(false);
  const camera =
    cameras.find((d) => d.descriptor.id === selected) ?? cameras[0];
  const calibration = state?.calibration;
  const poses = useMemo(
    () => calibratedCameras(calibration ?? []),
    [calibration],
  );
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
  return (
    <div className="workspace-page">
      <header className="workspace-heading">
        <div>
          <span className="eyebrow">Workspace / Cameras</span>
          <h1>
            Cameras <span className="heading-count mono">{cameras.length}</span>
          </h1>
        </div>
        <span className="subtle">2D image pixels · 3D world metres, +Y up</span>
      </header>
      {camera ? (
        <div className="cameras-layout">
          <section className="camera-main">
            <CameraTabs
              cameras={cameras}
              selected={camera.descriptor.id}
              onSelect={setSelected}
            />
            <label className="subtle">
              <input
                type="checkbox"
                checked={showAprilTags}
                onChange={(event) => setShowAprilTags(event.target.checked)}
              />
              {" "}Show AprilTag markers
            </label>
            <div className="camera-mode-toolbar" role="toolbar" aria-label="Camera workspace view">
              <button type="button" aria-pressed={viewMode === "split"} onClick={() => setViewMode("split")}>
                <Icon name="grid" size={14} /> Split
              </button>
              <button type="button" aria-pressed={viewMode === "camera"} onClick={() => setViewMode("camera")}>
                <Icon name="video" size={14} /> Camera
              </button>
              <button type="button" aria-pressed={viewMode === "reconstruction"} onClick={() => setViewMode("reconstruction")}>
                <Icon name="rotate" size={14} /> 3D reconstruction
              </button>
            </div>
            <div className={`camera-views camera-mode-${viewMode}`}>
              {viewMode !== "reconstruction" && (
                <CameraPreview device={camera} highlight={landmark} showAprilTags={showAprilTags} />
              )}
              {viewMode !== "camera" && <Reconstruction
                feature={state?.features.find(
                  (f) => f.id === "landmarks_3d_triangulated",
                )}
                poses={poses}
                cameraCount={cameras.length}
                selectedCamera={camera.descriptor.id}
                schema={schema}
                sampled={sampled}
                landmark={landmark}
                onSelect={setLandmark}
              />}
            </div>
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
                Each camera gives 2D landmarks in image pixels. Metric 3D
                positions come only from triangulating two or more calibrated,
                synchronised cameras. Simulated cameras project the simulated
                body; physical cameras run person detection and RTMPose.
              </p>
            </div>
            <LandmarkInspector inspection={sampled.inspection} />
            <div className="panel-heading">
              LANDMARKS{" "}
              <span className="mono">
                {sampled.live
                  ? `${sampled.reconstructed}/${sampled.total}`
                  : "—"}
              </span>
            </div>
            <div className="landmark-list" role="group" aria-label="Landmarks">
              {schema?.names.map((name, index) => {
                const status = sampled.statuses?.[index];
                return (
                  <button
                    key={`${name}-${index}`}
                    className={`segment-row ${landmark === index ? "selected" : ""}`}
                    aria-pressed={landmark === index}
                    onClick={() => setLandmark(index)}
                  >
                    <span
                      className="segment-glyph"
                      data-status={status}
                      aria-hidden="true"
                    >
                      {status ? STATUS[status][0] : "–"}
                    </span>
                    <span className="segment-name">{humanizeId(name)}</span>
                    <span className="subtle">
                      {status ? STATUS[status][1] : "waiting"}
                    </span>
                  </button>
                );
              })}
            </div>
          </aside>
        </div>
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
  selectedCamera,
  schema,
  sampled,
  landmark,
  onSelect,
}: {
  feature: FeatureStatus | undefined;
  poses: CameraPose[];
  cameraCount: number;
  selectedCamera: string;
  schema: LandmarkSchema | null;
  sampled: Readout;
  landmark: number | null;
  onSelect: (index: number) => void;
}) {
  return (
    <section className="reconstruction" aria-label="3D reconstruction">
      <header className="reconstruction-heading">
        <span className="eyebrow">3D reconstruction</span>
        {feature && <StatePill state={feature.state} />}
      </header>
      <div className="reconstruction-view">
        {poses.length >= 2 && schema ? (
          <>
            <ViewportCanvas
              skeleton={null}
              landmarkSchema={schema}
              selected={landmark ?? -1}
              onSelect={onSelect}
              view="3D"
              skeletonVisible={false}
              floorVisible
              cameras={poses}
              selectedCamera={selectedCamera}
            />
            <div className="viewport-top">
              <div>
                {sampled.origin && <OriginBadge label={sampled.origin} />}
              </div>
            </div>
            {!sampled.live && (
              <div className="no-pose">
                <span aria-hidden="true">◇</span>
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
      <div className="panel-heading">LANDMARK INSPECTOR</div>
      {inspection ? (
        <div className="page-panel-content landmark-inspector">
          <h2>{humanizeId(inspection.name)}</h2>
          <p data-testid="landmark-status">
            {inspection.status === "high_error"
              ? `◇ accepted · reprojection RMS above ${HIGH_REPROJECTION_PX} px`
              : inspection.status
                ? STATUS[inspection.status].join(" ")
                : "Waiting for a live 3D set"}
          </p>
          {inspection.status === "not_reconstructed" && (
            <p className="subtle">
              Too few views or a failed reprojection check. Compare the camera
              views below and the reasons under the 3D view.
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
          Select a landmark in the 3D view or the list below.
        </p>
      )}
    </>
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
