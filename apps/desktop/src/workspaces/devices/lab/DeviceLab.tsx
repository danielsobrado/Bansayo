import { useState } from "react";
import type { DeviceStatus } from "@contracts";
import { actions } from "../../../engine/actions";
import { useAction, useSampledFrame } from "../../../engine/hooks";
import { StatePill } from "../../../ui/StatePill";
import { Reasons } from "../../../ui/Reasons";
import { CameraPreview } from "../../cameras/CameraPreview";
import { OrientationView, TracePlot } from "./Plots";
import { CHANNEL_IDS, CHANNELS, STALE_MS, freshChannels, motionSummary, type ChannelId, type Snapshot } from "./trace";
import { useInspection } from "./useInspection";
import { SolverBench } from "./SolverBench";
import { GuidedTests } from "./GuidedTests";
import { TimingPanel, MagneticPanel, SaturationPanel, BatteryHistory } from "./SignalPanels";
import { TrackerComparison } from "./TrackerComparison";
import "./lab.css";

export function DeviceLab({ devices, live }: { devices: DeviceStatus[]; live: boolean }) {
  const [selected, setSelected] = useState("");
  const [solver, setSolver] = useState(false);
  const [compare, setCompare] = useState(false);
  const device = devices.find(d => d.descriptor.id === selected) ?? devices[0];
  if (!device) return <p className="subtle">Configure a source to inspect it.</p>;
  return <div className="lab-layout">
    <nav className="lab-devices" aria-label="Isolate device">
      <h3 className="eyebrow">ISOLATE DEVICE</h3>
      <button aria-pressed={solver} onClick={() => {setSolver(!solver);setCompare(false);}}>Solver comparison</button>
      <button aria-pressed={compare} onClick={()=>{setCompare(!compare);setSolver(false);}}>Tracker comparison</button>
      {devices.map(d => <button key={d.descriptor.id} aria-pressed={!solver && !compare && device.descriptor.id === d.descriptor.id} onClick={() => { setSelected(d.descriptor.id); setSolver(false); setCompare(false); }}>
        <span className="mono">{d.descriptor.id}</span><StatePill state={d.state}/><small>{d.descriptor.kind} · {d.descriptor.origin}</small>
      </button>)}
    </nav>
    {solver ? <SolverBench devices={devices} live={live}/> : compare ? <TrackerComparison devices={devices} live={live}/> :
      <Inspector key={device.descriptor.id + (live ? ":live" : ":offline")} device={device} live={live}/>} 
  </div>;
}

function exportSnapshot(device: DeviceStatus, snapshot: Snapshot) {
  const file = new Blob([JSON.stringify({ device, snapshot, exported_at: new Date().toISOString(), note: "Decimated accepted live observations" }, null, 2)], {type: "application/json"});
  const url = URL.createObjectURL(file);
  const a = document.createElement("a");
  a.href = url;
  a.download = `${device.descriptor.id}-inspection.json`;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

function Inspector({ device, live }: { device: DeviceStatus; live: boolean }) {
  const [frozen, setFrozen] = useState(false);
  const [focus, setFocus] = useState<ChannelId | "all">("all");
  const [axis, setAxis] = useState<number | "all">("all");
  const [ready, setReady] = useState(false);
  const [captured, setCaptured] = useState(false);
  const [capture, captureStatus] = useAction(actions.captureReferencePose);
  const reading = useInspection(device.descriptor.id, live && device.descriptor.kind !== "camera", frozen);
  const snapshot = reading.snapshot;
  const rawChannels = snapshot?.channels ?? {};
  const available = live && device.state !== "disconnected" && device.state !== "unavailable";
  const channels = frozen ? rawChannels : freshChannels(rawChannels,
    performance.now() - reading.receivedAt, available);
  const timing = device.time_sync;
  const motion = !frozen ? motionSummary(channels.gyro, channels.accel) : null;
  const qChannel = channels.orientation ?? channels.pose_rotation;
  const q = qChannel?.points.at(-1)?.values;
  const used = useSampledFrame(4, (frame, age) => frame && age <= STALE_MS ? (frame.pose?.segments ?? []).filter(s => s.sources.includes(device.descriptor.id)).length : null);
  const canCapture = live && !frozen && device.descriptor.kind === "imu_tracker" && !!device.assignment &&
    !!q && device.state !== "disconnected" && device.state !== "unavailable" && ready && !captureStatus.pending;
  const fields = [
    ["Rate / nominal", `${device.stream.measured_rate_hz.toFixed(1)} / ${device.descriptor.nominal_rate_hz ?? "—"} Hz`],
    ["Received", String(device.stream.received)], ["Dropped", String(device.stream.dropped)],
    ["Battery", device.battery ? `${Math.round(device.battery.level*100)}%` : "—"],
    ["RSSI", device.signal?.rssi_dbm == null ? "—" : `${device.signal.rssi_dbm} dBm`],
    ["Clock uncertainty", timing?.uncertainty_us == null ? "—" : `±${(timing.uncertainty_us/1000).toFixed(2)} ms`],
    ["Clock drift", timing?.drift_ppm == null ? "—" : `${timing.drift_ppm.toFixed(1)} ppm`],
    ["Clock resets", String(timing?.resets ?? "—")],
    ["Clock quality", timing?.time_quality ?? device.stream.time_quality],
    ["Clock settled", timing ? (timing.settled ? "Yes" : "No") : "—"],
    ["Segment", device.assignment ?? "Unassigned"], ["Solver segments using device", String(used ?? "—")],
  ];
  const visible = focus === "all" ? CHANNEL_IDS.filter(id => rawChannels[id] !== undefined) : [focus];
  return <div className="lab-main">
    <header className="lab-header">
      <div><h2 className="mono">{device.descriptor.id}</h2><p className="subtle">{device.descriptor.model ?? device.descriptor.kind} · {device.descriptor.transport} · {device.descriptor.origin}</p></div>
      <StatePill state={device.state}/>
      <div className="lab-tools">
        <button disabled={!snapshot || !live} aria-pressed={frozen} onClick={() => setFrozen(!frozen)}>{frozen ? "Resume traces" : "Freeze traces"}</button>
        <button disabled={!snapshot} onClick={() => snapshot && exportSnapshot(device,snapshot)}>Export JSON</button>
      </div>
    </header>
    <p className="lab-notice">{!live ? "Live inspection is unavailable during replay or disconnection." : frozen ? "Traces frozen; device health remains live." : "View isolation only — the solver and other devices keep running."}</p>
    <div className="lab-metrics">{fields.map(([label,value]) => <div key={label}><span>{label}</span><strong className="mono">{value}</strong></div>)}</div>
    <Reasons reasons={device.reasons}/>
    {device.descriptor.kind === "camera" ? (
      <section className="lab-card"><h3>Isolated camera &amp; optical markers</h3>
        {live && device.state !== "disconnected" && device.state !== "unavailable" ? <CameraPreview device={device} showAprilTags/> : <p>Waiting for a connected camera.</p>}
        <p className="subtle">Camera calibration, capture mode and exposure are managed in the Cameras workspace.</p>
      </section>
    ) : <>
      {reading.error && <p role="alert" className="error">{reading.error}</p>}
      {live && !snapshot && !reading.error && <p role="status">Waiting for accepted sensor samples…</p>}
      {live && snapshot && !frozen && Object.keys(channels).length === 0 &&
        <p role="status" className="lab-notice">No fresh component data. Check the device, transport, and engine connection.</p>}
      <div className="lab-grid">
        <OrientationView q={q}/>
        <section className="lab-card">
          <h3>Calibration bench</h3>
          <p>Keep still on a level surface, then rotate through each axis. Verify the cube and raw vectors.</p>
          <div className="lab-values mono">
            <span>Gyro peak: {motion ? motion.peak.toFixed(3) : "—"} rad/s</span>
            <span>Gravity error: {motion ? motion.gravityError.toFixed(3) : "—"} m/s²</span>
          </div>
          <p role="status">{frozen ? "Resume traces to check stillness." : motion ? (motion.still ? "Stationary in sampled window" : "Motion detected") : "Need fresh gyro and accelerometer samples for stillness check."}</p>
          <p className="subtle">Advisory only. Board-level bias recalibration uses the Mosaico touchscreen. Mounting calibration requires a body reference pose.</p>
          <label><input type="checkbox" checked={ready} onChange={e => {setReady(e.target.checked);setCaptured(false);}}/> Tracker mounted; upright T-pose ready</label>
          <button disabled={!canCapture} onClick={async () => {
            setCaptured(false);
            const res = await capture({ device_ids: [device.descriptor.id] });
            if (res.ok) { setCaptured(true); setReady(false); }
          }}>{captureStatus.pending ? "Capturing…" : "Capture this tracker only"}</button>
          {captureStatus.error && <p role="alert" className="error">{captureStatus.error}</p>}
          {captured && <p role="status">Mounting capture succeeded.</p>}
          <p className="subtle">{device.calibration.map(c => `${c.kind}: ${c.state}`).join(" · ") || "No calibration status reported."}</p>
        </section>
      </div>
      <div className="lab-grid">
        <TimingPanel timing={(frozen || channels.orientation) ? snapshot?.timing : undefined}/>
        <MagneticPanel a={channels.mag0} b={channels.mag1}/>
        <SaturationPanel gyro={channels.gyro} accel={channels.accel} native={device.descriptor.model?.includes("bansayo.imu.v1")===true}/>
        <BatteryHistory device={device} enabled={available}/>
      </div>
      <GuidedTests deviceId={device.descriptor.id} channels={channels} enabled={available && !frozen}/>
      <div className="lab-tools">
        <label>Component <select value={focus} onChange={e => {setFocus(e.target.value as ChannelId | "all");setAxis("all");}}>
          <option value="all">All available</option>
          {CHANNEL_IDS.map(id => <option key={id} value={id}>{CHANNELS[id].title}</option>)}
        </select></label>
        <label>Axis <select value={axis} onChange={e => setAxis(e.target.value === "all" ? "all" : Number(e.target.value))}>
          <option value="all">All</option>{(focus === "all" ? ["X","Y","Z","W"] : CHANNELS[focus].axes).map((a,i) => <option key={i} value={i}>{a}</option>)}
        </select></label>
        <span className="subtle">Up to 50 Hz display history · five seconds · not a full-rate recording</span>
      </div>
      {visible.length === 0 && <p className="subtle">No supported component data yet. Enable BMAG diagnostics in compatible firmware to see magnetometers.</p>}
      <div className="lab-grid">{visible.map(id => {
        const channel = channels[id];
        const indices = CHANNELS[id].axes.map((_,i) => i).filter(i => axis === "all" || i === axis);
        if (!indices.length) return null;
        return <TracePlot key={id} id={id} channel={channel} axes={indices}/>;
      })}</div>
    </>}
  </div>;
}
