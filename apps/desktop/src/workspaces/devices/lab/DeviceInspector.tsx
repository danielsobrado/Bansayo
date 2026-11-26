import { useState } from "react";
import type { DeviceStatus, SkeletonDefinition } from "@contracts";
import { useSampledFrame } from "../../../engine/hooks";
import { StatePill } from "../../../ui/StatePill";
import { Reasons } from "../../../ui/Reasons";
import { Icon } from "../../../ui/Icon";
import { BodyMap } from "../../../ui/BodyMap";
import { CameraPreview } from "../../cameras/CameraPreview";
import { OrientationView } from "./Plots";
import { GuidedTests } from "./GuidedTests";
import { TimingPanel, MagneticPanel, SaturationPanel, BatteryHistory } from "./SignalPanels";
import { MountingPanel } from "./MountingPanel";
import { SignalWorkspace } from "./SignalWorkspace";
import { STALE_MS, freshChannels, motionSummary, type Snapshot } from "./trace";
import { useInspection } from "./useInspection";
import { downloadText } from "./signalMath";
import type { SignalFocus, StudioView } from "./studioConfig";

function exportSnapshot(device: DeviceStatus, snapshot: Snapshot) {
  downloadText(device.descriptor.id+"-inspection.json",JSON.stringify({device,snapshot,exported_at:new Date().toISOString(),note:"Decimated accepted live observations"},null,2),"application/json");
}
export function DeviceInspector({device,devices,live,skeleton,view,focus,onFocus,showInspector,onSelectCamera}:{
  device:DeviceStatus;devices:DeviceStatus[];live:boolean;skeleton:SkeletonDefinition|null;view:StudioView;focus:SignalFocus;onFocus:(focus:SignalFocus)=>void;showInspector:boolean;onSelectCamera:(id:string)=>void;
}) {
  const [frozen,setFrozen]=useState(false);
  const camera=device.descriptor.kind==="camera";
  const reading=useInspection(device.descriptor.id,live&&!camera,frozen);
  const snapshot=reading.snapshot;
  const raw=snapshot?.channels??{};
  const available=live&&device.state!=="disconnected"&&device.state!=="unavailable";
  const channels=frozen?raw:freshChannels(raw,performance.now()-reading.receivedAt,available);
  const q=(channels.orientation??channels.pose_rotation)?.points.at(-1)?.values;
  const motion=!frozen?motionSummary(channels.gyro,channels.accel):null;
  const used=useSampledFrame(4,(frame,age)=>frame&&age<=STALE_MS?(frame.pose?.segments??[]).filter(s=>s.sources.includes(device.descriptor.id)).length:null);
  const timing=device.time_sync;
  const metrics=[
    ["Rate",`${device.stream.measured_rate_hz.toFixed(1)} Hz`,`Nominal ${device.descriptor.nominal_rate_hz??"—"} Hz`],
    ["Dropped",String(device.stream.dropped),"Observations"],
    ["Battery",device.battery?`${Math.round(device.battery.level*100)}%`:"—","Reported charge"],
    ["Signal",device.signal?.rssi_dbm==null?"—":`${device.signal.rssi_dbm} dBm`,"RSSI"],
    ["Clock",timing?.uncertainty_us==null?"—":`±${(timing.uncertainty_us/1000).toFixed(2)} ms`,"Uncertainty"],
  ];
  const fields=[
    ["Source",device.descriptor.source_id],["Model",device.descriptor.model??"—"],["Firmware",device.descriptor.firmware??"—"],
    ["Origin",device.descriptor.origin],["Transport",device.descriptor.transport],
    ["Received",String(device.stream.received)],["Clock drift",timing?.drift_ppm==null?"—":timing.drift_ppm.toFixed(1)+" ppm"],
    ["Clock resets",String(timing?.resets??"—")],["Clock quality",timing?.time_quality??device.stream.time_quality],
    ["Clock settled",timing?(timing.settled?"Yes":"No"):"—"],["Assignment",device.assignment??"Unassigned"],
    ["Solver segments",String(used??"—")],
  ];
  const native=device.descriptor.source_id==="mosaico"&&device.descriptor.kind==="imu_tracker";
  const cameras=devices.filter(d=>d.descriptor.kind==="camera");
  const optical=view==="optical"||camera;
  return <div className="studio-inspection">
    <header className="studio-device-toolbar">
      <div className="studio-device-title"><h3 className="mono">{device.descriptor.id}</h3><StatePill state={device.state}/><span className="subtle">{device.descriptor.kind}</span></div>
      {!camera&&<div className="studio-controls">
        <button disabled={!snapshot||!live} aria-pressed={frozen} onClick={()=>setFrozen(v=>!v)}><Icon name={frozen?"play":"pause"}/>{frozen?"Resume traces":"Freeze traces"}</button>
        <button disabled={!snapshot} onClick={()=>snapshot&&exportSnapshot(device,snapshot)}>Export JSON</button>
      </div>}
    </header>
    <div className="studio-metrics">{metrics.map(([label,value,unit])=><div key={label}><span>{label}</span><strong className="mono">{value}</strong><small>{unit}</small></div>)}</div>
    {(!available||frozen||reading.error||(!camera&&live&&!Object.keys(channels).length))&&<p className="lab-notice" role={reading.error?"alert":"status"}>{reading.error??(!live?"Live inspection unavailable during replay or disconnection.":frozen?"Frozen snapshot — health remains live.":!available?"Device offline — measurements unavailable.":"Waiting for fresh accepted observations.")}</p>}
    <div className="studio-instrument-grid" data-inspector={showInspector}>
      <div className="studio-instruments">
        {optical?<section className="lab-card"><header><h3>Optical inspection</h3><select aria-label="Optical camera" value={camera?device.descriptor.id:""} onChange={e=>onSelectCamera(e.target.value)}><option value="" disabled>Select camera</option>{cameras.map(c=><option key={c.descriptor.id}>{c.descriptor.id}</option>)}</select></header>
          {camera&&available?<CameraPreview device={device} showAprilTags/>:<p className="studio-empty">Select a connected camera to inspect images, landmarks and optical markers.</p>}
          <p className="subtle">Use Cameras for intrinsic/extrinsic calibration, capture settings and exposure.</p></section>:
          <>
            {view==="overview"&&<><div className="studio-overview-grid"><OrientationView q={q} accel={channels.accel?.points.at(-1)?.values} magnetic={channels.mag0?.points.at(-1)?.values} frozen={frozen}/>
              <section className="lab-card studio-attitude"><header><h3>Motion state</h3></header><strong className="studio-motion" data-still={motion?.still===true}>{motion?motion.still?"Stationary":"In motion":"—"}</strong>
              <dl className="studio-properties"><div><dt>Gyro peak</dt><dd>{motion?.peak.toFixed(3)??"—"} rad/s</dd></div><div><dt>Gravity error</dt><dd>{motion?.gravityError.toFixed(3)??"—"} m/s²</dd></div><div><dt>Mounting</dt><dd>{device.calibration.find(c=>c.kind==="mounting")?.state??"Unavailable"}</dd></div></dl><p className="subtle">Advisory, based on recent samples.</p></section></div>
              <SignalWorkspace key="overview" device={device} channels={channels} known={raw} focus="all" onFocus={onFocus} compact frozen={frozen}/></>}
            {view==="signals"&&<SignalWorkspace device={device} channels={channels} known={raw} focus={focus} onFocus={onFocus} frozen={frozen}/>}
            <div hidden={view!=="calibration"} className="studio-calibration">
              <MountingPanel device={device} live={live} frozen={frozen} channels={channels}/>
              <GuidedTests deviceId={device.descriptor.id} channels={channels} enabled={available&&!frozen}/>
            </div>
            {view==="timing"&&<div className="lab-grid"><TimingPanel timing={(frozen||channels.orientation)?snapshot?.timing:undefined}/><MagneticPanel a={channels.mag0} b={channels.mag1}/><SaturationPanel gyro={channels.gyro} accel={channels.accel} native={native}/><BatteryHistory device={device} enabled={available}/></div>}
          </>}
      </div>
      {showInspector&&<aside className="studio-inspector" aria-label="Device properties">
        <div className="studio-panel-title"><Icon name="sliders"/><strong>INSPECTOR</strong></div>
        <dl className="studio-properties">{fields.map(([label,value])=><div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>
        <div className="studio-panel-title"><strong>CALIBRATION</strong></div>
        <dl className="studio-properties">{device.calibration.length?device.calibration.map(c=><div key={c.id}><dt>{c.kind}</dt><dd>{c.state}</dd></div>):<div><dt>Profiles</dt><dd>None</dd></div>}</dl>
        {device.assignment&&skeleton&&<><div className="studio-panel-title"><strong>BODY ASSIGNMENT</strong></div><BodyMap skeleton={skeleton} values={[]} highlight={device.assignment}/><p className="subtle studio-no-match">Reference assignment, not live pose.</p></>}
        <Reasons reasons={device.reasons}/>
      </aside>}
    </div>
  </div>;
}
