import { useState } from "react";
import type { CameraRuntimeStatus, CaptureMode, DeviceId } from "@contracts";
import { actions } from "../../engine/actions";
import { humanizeId } from "../../lib/format";

const mode = (m: CaptureMode | null): string =>
  m
    ? `${m.pixel_format} ${m.resolution_px.join("×")} @ ${m.fps == null ? "rate unknown" : `${m.fps} fps`}${m.verified ? "" : " (unverified)"}`
    : "—";

/** Identity, lifecycle, requested against measured capture, and frame timing of one camera. */
export function CameraRuntime({ id, runtime }: { id: DeviceId; runtime: CameraRuntimeStatus }) {
  const [message, setMessage] = useState<string | null>(null);
  const t = runtime.timing;
  const identity = runtime.identity;
  return (
    <>
      <dl className="detail-list">
        <dt>Lifecycle</dt>
        <dd>{humanizeId(runtime.lifecycle)}</dd>
        <dt>Hardware</dt>
        <dd className="mono" title={identity?.interface_id ?? undefined}>
          {identity ? identity.fingerprint : "not identified"}
        </dd>
        <dt>Serial</dt>
        <dd className="mono">{identity?.serial ?? "none exposed"}</dd>
        <dt>Identity</dt>
        <dd>{runtime.identity_confirmed ? "Matches its calibration" : "Differs from its calibration"}</dd>
        <dt>Requested</dt>
        <dd className="mono">{mode(runtime.requested)}</dd>
        <dt>Negotiated</dt>
        <dd className="mono">{mode(runtime.negotiated)}</dd>
        <dt>Actual rate</dt>
        <dd className="mono">{t ? `${t.measured_fps.toFixed(1)} fps` : "—"}</dd>
        <dt>Interval mean / p95 / p99</dt>
        <dd className="mono">
          {t ? `${t.mean_interval_ms.toFixed(1)} / ${t.p95_interval_ms.toFixed(1)} / ${t.p99_interval_ms.toFixed(1)} ms` : "—"}
        </dd>
        <dt>Jitter</dt>
        <dd className="mono">{t ? `${t.jitter_ms.toFixed(1)} ms` : "—"}</dd>
        <dt>Replaced before inference</dt>
        <dd className="mono">{runtime.replaced_frames}</dd>
        <dt>Inference rate</dt>
        <dd className="mono">{runtime.inference_hz == null ? "—" : `${runtime.inference_hz.toFixed(1)} Hz`}</dd>
        <dt>Decode</dt>
        <dd className="mono">{runtime.decode_ms == null ? "—" : `${runtime.decode_ms.toFixed(1)} ms`}</dd>
        <dt>Reconnects</dt>
        <dd className="mono">{runtime.reconnects}</dd>
      </dl>
      {!runtime.identity_confirmed && identity && (
        <div>
          <button
            onClick={() =>
              void actions.confirmCameraIdentity(id).then((r) =>
                setMessage(r.ok ? "Identity confirmed." : r.error.message),
              )
            }
          >
            This is the same camera
          </button>
          {message && <p className="subtle" role="status">{message}</p>}
        </div>
      )}
    </>
  );
}
