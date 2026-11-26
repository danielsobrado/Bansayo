import { useState } from "react";
import { FACES, faceMeasurement, quaternionAngle, ROTATION_TOLERANCE_DEG } from "./benchMath";
import { motionSummary, type Snapshot } from "./trace";

export function GuidedTests({ deviceId, channels, enabled }: {
  deviceId: string; channels: Snapshot["channels"]; enabled: boolean;
}) {
  const [faces,setFaces] = useState<Record<number,NonNullable<ReturnType<typeof faceMeasurement>>>>({});
  const [face,setFace] = useState(0);
  const [start,setStart] = useState<{q:number[];t:number}|null>(null);
  const [turn,setTurn] = useState<number|null>(null);
  const [drift,setDrift] = useState<{q:number[];t:number}|null>(null);
  const [report,setReport] = useState<{angle:number;seconds:number}|null>(null);
  const [axis,setAxis] = useState("X");
  const motion = motionSummary(channels.gyro,channels.accel);
  const q = channels.orientation?.points.at(-1);
  const faceSample = channels.accel && faceMeasurement(channels.accel,face);
  const still = enabled && motion?.still === true;
  function download() {
    const blob = new Blob([JSON.stringify({deviceId,faces,rotation:{axis,angle:turn},drift:report,
      thresholds:"Advisory, not metrological calibration",exported_at:new Date().toISOString()},null,2)],{type:"application/json"});
    const url=URL.createObjectURL(blob);
    const a=document.createElement("a"); a.href=url; a.download=deviceId+"-bench.json"; a.click();
    setTimeout(()=>URL.revokeObjectURL(url),1000);
  }
  return <section className="lab-card">
    <header><h3>Guided hardware tests</h3><button onClick={download}>Export test report</button></header>
    <p className="subtle">Advisory checks using sampled telemetry; no coefficients are written to firmware.</p>
    <div className="lab-grid">
      <div className="lab-card">
        <h3>Six-face acceleration check</h3>
        <p>Point the selected positive or negative sensor axis vertically upward. Hold still for one second.</p>
        <div className="lab-bench-controls">{FACES.map((name,i)=><button key={name} aria-pressed={face===i} onClick={()=>setFace(i)}>{name}{faces[i] ? faces[i].pass ? " ✓" : " !" : ""}</button>)}</div>
        <p className="mono">{faceSample ? `Vector error ${faceSample.error.toFixed(3)} m/s²` : "Waiting for samples"}</p>
        <button disabled={!still || !faceSample} onClick={()=>{if(faceSample){setFaces(old=>({...old,[face]:faceSample}));setFace((face+1)%6);}}}>Capture face</button>
        {Object.entries(faces).map(([i,r])=><p key={i} className={r.pass?"lab-result-ok":"lab-result-warn"}>{FACES[Number(i)]}: {r.error.toFixed(3)} m/s² · {r.pass?"Within tolerance":"Check orientation or bias"}</p>)}
        <button onClick={()=>{setFaces({});setFace(0);}}>Reset faces</button>
      </div>
      <div className="lab-card">
        <h3>90° rotation check</h3>
        <label>Physical axis <select value={axis} onChange={e=>{setAxis(e.target.value);setStart(null);setTurn(null);}}>{["X","Y","Z"].map(a=><option key={a}>{a}</option>)}</select></label>
        <p>Hold still, capture start, rotate exactly 90° about {axis} using a physical reference, then stop and capture end.</p>
        <button disabled={!still || !q} onClick={()=>{if(q){setStart({q:q.values,t:q.t_us});setTurn(null);}}}>Capture start</button>
        <button disabled={!still || !q || !start || q.t_us<=start.t} onClick={()=>{if(q&&start)setTurn(quaternionAngle(start.q,q.values));}}>Capture end</button>
        {turn!==null&&<p className={Math.abs(turn-90)<=ROTATION_TOLERANCE_DEG?"lab-result-ok":"lab-result-warn"}>Rotation {turn.toFixed(2)}° · error {Math.abs(turn-90).toFixed(2)}° (magnitude only)</p>}
      </div>
      <div className="lab-card">
        <h3>Stationary orientation drift</h3>
        <p>Leave the unit untouched. Start and stop manually; use a full session recording to prove continuous stillness.</p>
        <button disabled={!still || !q} onClick={()=>{if(q){setDrift({q:q.values,t:q.t_us});setReport(null);}}}>Start drift baseline</button>
        <p className="mono">{drift&&q?`${Math.max(0,(q.t_us-drift.t)/1e6).toFixed(1)} s · net ${quaternionAngle(drift.q,q.values)?.toFixed(2)??"—"}°`:"No baseline"}</p>
        <button disabled={!still || !q || !drift || q.t_us<=drift.t} onClick={()=>{if(q&&drift){const angle=quaternionAngle(drift.q,q.values);if(angle!==null)setReport({angle,seconds:(q.t_us-drift.t)/1e6});setDrift(null);}}}>Finish test</button>
        {report&&<p>Net orientation change {report.angle.toFixed(2)}° over {report.seconds.toFixed(1)} s; not a pure yaw estimate.</p>}
      </div>
    </div>
  </section>;
}
