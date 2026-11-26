import { useState } from "react";
import type { DeviceStatus } from "@contracts";
import { actions } from "../../../engine/actions";
import { useAction } from "../../../engine/hooks";
import { motionSummary, type Snapshot } from "./trace";

export function MountingPanel({device,live,frozen,channels}:{device:DeviceStatus;live:boolean;frozen:boolean;channels:Snapshot["channels"]}) {
  const [ready, setReady] = useState(false);
  const [captured, setCaptured] = useState(false);
  const [capture, captureStatus] = useAction(actions.captureReferencePose);
  const motion = !frozen ? motionSummary(channels.gyro, channels.accel) : null;
  const qChannel = channels.orientation ?? channels.pose_rotation;
  const q = qChannel?.points.at(-1)?.values;
  const canCapture = live && !frozen && device.descriptor.kind === "imu_tracker" && !!device.assignment &&
    !!q && device.state !== "disconnected" && device.state !== "unavailable" && ready && !captureStatus.pending;
  return <section className="lab-card">
          <h3>Calibration bench</h3>
          <p>Keep still on a level surface, then rotate through each axis. Verify the cube and raw vectors.</p>
          <div className="lab-values mono">
            <span>Gyro peak: {motion ? motion.peak.toFixed(3) : "—"} rad/s</span>
            <span>Gravity error: {motion ? motion.gravityError.toFixed(3) : "—"} m/s²</span>
          </div>
          <p role="status">{frozen ? "Resume traces to check stillness." : motion ? (motion.still ? "Stationary in sampled window" : "Motion detected") : "Need fresh gyro and accelerometer samples for stillness check."}</p>
          <p className="subtle">Advisory only. Board-level bias recalibration uses the Mosaico touchscreen. BIMU v2 reference changes invalidate mounting; repeat a body reference pose afterwards.</p>
          <label><input type="checkbox" checked={ready} onChange={e => {setReady(e.target.checked);setCaptured(false);}}/> Tracker mounted; upright T-pose ready</label>
          <button disabled={!canCapture} onClick={async () => {
            setCaptured(false);
            const res = await capture({ device_ids: [device.descriptor.id] });
            if (res.ok) { setCaptured(true); setReady(false); }
          }}>{captureStatus.pending ? "Capturing…" : "Capture this tracker only"}</button>
          {captureStatus.error && <p role="alert" className="error">{captureStatus.error}</p>}
          {captured && <p role="status">Mounting capture succeeded.</p>}
          <p className="subtle">{device.calibration.map(c => `${c.kind}: ${c.state}`).join(" · ") || "No calibration status reported."}</p>
        </section>;
}
