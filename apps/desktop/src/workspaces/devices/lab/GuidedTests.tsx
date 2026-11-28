import { useState } from "react";
import {benchQuality} from "./calibrationQuality";
import {CALIBRATION_SCORE} from "./reliabilityConfig";
import { Icon } from "../../../ui/Icon";
import { FaceReference } from "./FaceReference";
import { FACES, faceMeasurement, quaternionAngle, rotationCheck, sixFaceEstimate, ROTATION_TOLERANCE_DEG } from "./benchMath";
import { motionSummary, type Snapshot } from "./trace";

export function GuidedTests({ deviceId, channels, enabled }: {
  deviceId: string; channels: Snapshot["channels"]; enabled: boolean;
}) {
  const [faces,setFaces] = useState<Record<number,NonNullable<ReturnType<typeof faceMeasurement>>>>({});
  const [face,setFace] = useState(0);
  const [start,setStart] = useState<{q:number[];t:number}|null>(null);
  const [turn,setTurn] = useState<ReturnType<typeof rotationCheck>>(null);
  const [turns,setTurns]=useState<Partial<Record<string,NonNullable<ReturnType<typeof rotationCheck>>>>>({});
  const [drift,setDrift] = useState<{q:number[];t:number}|null>(null);
  const [report,setReport] = useState<{angle:number;seconds:number}|null>(null);
  const [axis,setAxis] = useState("X");
  const motion = motionSummary(channels.gyro,channels.accel);
  const q = channels.orientation?.points.at(-1);
  const faceSample = channels.accel && faceMeasurement(channels.accel,face);
  const still = enabled && motion?.still === true;
  const estimate = sixFaceEstimate(faces);
  const [test,setTest] = useState<"face"|"rotation"|"drift">("face");
  const complete = Object.keys(faces).length;
  const passing = Object.values(faces).filter(v=>v.pass).length;
  const quality=benchQuality(faces,turns,report);
  function download() {
    const blob = new Blob([JSON.stringify({deviceId,faces,accelerationEstimate:estimate,rotation:{axis,result:turn,axes:turns},drift:report,quality,
      thresholds:"Advisory, not metrological calibration",exported_at:new Date().toISOString()},null,2)],{type:"application/json"});
    const url=URL.createObjectURL(blob);
    const a=document.createElement("a"); a.href=url; a.download=deviceId+"-bench.json"; a.click();
    setTimeout(()=>URL.revokeObjectURL(url),1000);
  }
  return <section className="lab-card">
    <header><h3>Guided hardware tests</h3><button onClick={download}>Export test report</button></header>
    <p className="subtle">Advisory checks using sampled telemetry; no coefficients are written to firmware.</p>
    <p className="mono">Calibration bench score: {quality.score===null?"Incomplete":quality.score+"/100"} · {quality.completed}/10 checks captured · {quality.passing} within tolerance</p>
    <p className="subtle">Complete six faces, 90° rotations on all three axes, and a stationary endpoint drift test of at least {CALIBRATION_SCORE.minimumDriftSeconds} s. Equal-weight gate score, not calibrated accuracy; endpoint stillness is not continuous proof.</p>
    <div className="studio-subbar" aria-label="Calibration tests">
      <button aria-pressed={test==="face"} onClick={()=>setTest("face")}>Accelerometer · {complete}/6</button>
      <button aria-pressed={test==="rotation"} onClick={()=>setTest("rotation")}>90° rotation</button>
      <button aria-pressed={test==="drift"} onClick={()=>setTest("drift")}>Stationary drift</button>
    </div>
    <div className="studio-test-panel" hidden={test!=="face"}>
      <div className="studio-test-head"><div><h3>Step {face+1} of 6 · {FACES[face]} upward</h3><small className="subtle">{complete}/6 captured · {passing}/6 within tolerance</small></div><strong className={still?"lab-result-ok":"subtle"}>{still?"Stable":"Waiting for stillness"}</strong></div>
      <progress aria-label="Six-face calibration progress" value={complete} max={6}/>
      <div className="studio-face-layout">
        <FaceReference face={face}/>
        <div className="studio-test-instructions">
        <h3>Six-face acceleration check</h3>
        <p>Point the selected positive or negative sensor axis vertically upward. Hold still for one second.</p>
        <div className="studio-face-steps">{FACES.map((name,i)=><button key={name} aria-pressed={face===i} data-result={faces[i]?faces[i].pass?"pass":"warn":undefined} onClick={()=>setFace(i)}><Icon name={faces[i]?faces[i].pass?"check":"warning":"target"} size={14}/>{name}</button>)}</div>
        <p className="mono">{faceSample ? `Vector error ${faceSample.error.toFixed(3)} m/s²` : "Waiting for samples"}</p>
        <button disabled={!still || !faceSample} onClick={()=>{if(faceSample){setFaces(old=>({...old,[face]:faceSample}));setFace((face+1)%6);}}}>Capture face</button>
        </div>
      </div>
      <table className="studio-stats"><thead><tr><th>Face</th><th>Vector error</th><th>Result</th></tr></thead><tbody>{FACES.map((name,i)=><tr key={name}><th>{name}</th><td>{faces[i]?.error.toFixed(3)??"—"} m/s²</td><td className={faces[i]?faces[i].pass?"lab-result-ok":"lab-result-warn":"subtle"}>{faces[i]?faces[i].pass?"Within tolerance":"Review":"Not captured"}</td></tr>)}</tbody></table>
        {estimate&&<div className="mono">
          <p>Estimated bias: {estimate.bias.map(v=>v.toFixed(3)).join(" / ")} m/s²</p>
          <p>Measured scale: {estimate.scale.map(v=>v.toFixed(4)).join(" / ")}</p>
          <p className="subtle">Diagnostic diagonal estimates only; not applied to hardware.</p>
        </div>}
        <button onClick={()=>{setFaces({});setFace(0);}}>Reset faces</button>
      </div>
      <div className="studio-test-panel" hidden={test!=="rotation"}>
        <h3>90° rotation check</h3>
        <label>Physical axis <select value={axis} onChange={e=>{setAxis(e.target.value);setStart(null);setTurn(null);}}>{["X","Y","Z"].map(a=><option key={a}>{a}</option>)}</select></label>
        <p>Hold still, capture start, rotate exactly 90° about {axis} using a physical reference, then stop and capture end.</p>
        <button disabled={!still || !q} onClick={()=>{if(q){setStart({q:q.values,t:q.t_us});setTurn(null);}}}>Capture start</button>
        <button disabled={!still || !q || !start || q.t_us<=start.t} onClick={()=>{if(q&&start){const result=rotationCheck(start.q,q.values,["X","Y","Z"].indexOf(axis));setTurn(result);setTurns(old=>{const next={...old};if(result)next[axis]=result;else delete next[axis];return next;});}}}>Capture end</button>
        <p className="subtle">Axes captured: {Object.keys(turns).join(", ")||"None"}</p><button onClick={()=>{setTurns({});setTurn(null);setStart(null);}}>Reset rotation tests</button>
        {turn!==null&&<p className={Math.abs(turn.angle-90)<=ROTATION_TOLERANCE_DEG&&turn.axisError<=ROTATION_TOLERANCE_DEG?"lab-result-ok":"lab-result-warn"}>Rotation {turn.angle.toFixed(2)}° · angle error {Math.abs(turn.angle-90).toFixed(2)}° · axis error {turn.axisError.toFixed(2)}° (unsigned axis)</p>}
      </div>
      <div className="studio-test-panel" hidden={test!=="drift"}>
        <h3>Stationary orientation drift</h3>
        <p>Leave the unit untouched. Start and stop manually; use a full session recording to prove continuous stillness.</p>
        <button disabled={!still || !q} onClick={()=>{if(q){setDrift({q:q.values,t:q.t_us});setReport(null);}}}>Start drift baseline</button>
        <p className="mono">{drift&&q?`${Math.max(0,(q.t_us-drift.t)/1e6).toFixed(1)} s · net ${quaternionAngle(drift.q,q.values)?.toFixed(2)??"—"}°`:"No baseline"}</p>
        <button disabled={!still || !q || !drift || q.t_us<=drift.t} onClick={()=>{if(q&&drift){const angle=quaternionAngle(drift.q,q.values);if(angle!==null)setReport({angle,seconds:(q.t_us-drift.t)/1e6});setDrift(null);}}}>Finish test</button>
        {report&&<p>Net orientation change {report.angle.toFixed(2)}° over {report.seconds.toFixed(1)} s; not a pure yaw estimate.</p>}
      </div>
  </section>;
}
