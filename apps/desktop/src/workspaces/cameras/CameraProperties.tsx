import type { DeviceStatus } from "@contracts";
import { useSampled } from "../../engine/hooks";
import { landmarks3dStore, STALE_FRAME_MS } from "../../engine/stores";
import { humanizeId } from "../../lib/format";
import { OriginBadge } from "../../ui/OriginBadge";
import { StatePill } from "../../ui/StatePill";
import { Reasons } from "../../ui/Reasons";
import { CameraRuntime } from "./CameraRuntime";

export function CameraProperties({ camera }: { camera: DeviceStatus }) {
  const { descriptor, stream } = camera;
  const contribution = useSampled(4, () => {
    const set = landmarks3dStore.landmarks;
    if (!set || performance.now() - landmarks3dStore.receivedAt > STALE_FRAME_MS)
      return null;
    let used = 0;
    let rejected = 0;
    for (const point of set.points) {
      if (point?.used_views.includes(descriptor.id)) used++;
      if (point?.rejected_views.includes(descriptor.id)) rejected++;
    }
    return { used, rejected };
  }, descriptor.id);
  return (
    <div className="inspector-content">
      <span className="eyebrow">Camera</span>
      <h3 className="mono">{descriptor.id}</h3>
      <div className="readout-meta">
        <StatePill state={camera.state} />
        <OriginBadge label={descriptor.origin} />
      </div>
      <dl className="detail-list">
        <dt>Resolution</dt>
        <dd className="mono">{descriptor.camera?.resolution_px.join(" × ") ?? "—"}</dd>
        <dt>Measured rate</dt>
        <dd className="mono">{stream.measured_rate_hz.toFixed(1)} fps</dd>
        <dt>Nominal rate</dt>
        <dd className="mono">{descriptor.nominal_rate_hz ?? "—"}</dd>
        <dt>Time quality</dt>
        <dd>{humanizeId(stream.time_quality)}</dd>
        <dt>Dropped frames</dt>
        <dd className="mono">{stream.dropped}</dd>
        <dt>Landmark schema</dt>
        <dd className="mono">{descriptor.camera?.landmark_schema?.id ?? "—"}</dd>
        {camera.calibration.map((entry) => (
          <div key={entry.kind} className="detail-row">
            <dt>{humanizeId(entry.kind)}</dt><dd>{humanizeId(entry.state)}</dd>
          </div>
        ))}
        <dt>3D landmarks used</dt>
        <dd className="mono">{contribution?.used ?? "—"}</dd>
        <dt>3D landmarks rejected</dt>
        <dd className="mono">{contribution?.rejected ?? "—"}</dd>
      </dl>
      {camera.camera_runtime && (
        <CameraRuntime id={descriptor.id} runtime={camera.camera_runtime} />
      )}
      <Reasons reasons={camera.reasons} />
    </div>
  );
}
