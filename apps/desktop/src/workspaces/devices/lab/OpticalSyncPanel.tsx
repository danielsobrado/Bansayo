import {useMemo,useState} from "react";
import type {Capture} from "./captureTypes";
import {opticalTiming} from "./opticalSyncMath";
import {downloadText} from "./signalMath";
export function OpticalSyncPanel({capture}:{capture:Capture}){
  const [selected,setSelected]=useState("");
  const [confirmed,setConfirmed]=useState<Capture|null>(null);
  const ids=useMemo(()=>[...new Set(capture.samples.filter(s=>s.channel==="gyro").map(s=>s.device))],[capture]);
  const device=ids.includes(selected)?selected:ids[0]??"";
  const optical=capture.optical;
  const report=useMemo(()=>optical&&confirmed===capture?opticalTiming(capture,device):null,[capture,device,optical,confirmed]);
  const result=report?.result;
  return <section className="lab-card"><header><h3>Camera-to-IMU timing</h3></header>
    {!optical?<p>Enable a camera landmark pair before starting browser capture. Engine-only journals do not contain camera observations.</p>:<>
      <p className="mono">{optical.selection.device} · landmarks {optical.selection.first} / {optical.selection.second} · {optical.frames.length} camera observations · origin {optical.origin??"unknown"}</p>
      <p>Rotate the rigid pair in the image plane with a fixed camera; use distinctive slow rotations, no axial or out-of-plane motion. The projected angular speed is compared to the gyroscope magnitude.</p>
      <label>Reference IMU <select value={device} onChange={e=>{setSelected(e.target.value);setConfirmed(null);}}>{ids.map(id=><option key={id}>{id}</option>)}</select></label>
      <label><input type="checkbox" checked={confirmed===capture} onChange={e=>setConfirmed(e.target.checked?capture:null)}/> Confirm rigid shared planar motion and fixed camera</label>
      {report?.error&&<p className="lab-result-warn">{report.error}</p>}
      {result&&<><p className="mono">Camera relative lag {result.lagMs===null?"inconclusive":result.lagMs.toFixed(1)+" ms"} · r {result.correlation?.toFixed(3)??"—"} · grid {result.stepUs===null?"—":(result.stepUs/1000).toFixed(2)+" ms"} · {result.pairs} pairs</p><p>{result.reason}</p></>}
      <button disabled={!report} onClick={()=>downloadText("bansayo-optical-sync.json",JSON.stringify({format:"bansayo-optical-sync-v1",started:capture.started,window:capture.window,device,optical,report,operatorConfirmed:confirmed===capture},null,2),"application/json")}>Export camera timing report</button>
    </>}
    <p className="subtle">Uses received camera landmark messages, their mapped capture timestamps, and accepted full-rate gyro observations. Browser transport can omit frames; no hardware triggering, exposure-time verification, automatic offset correction or absolute latency claim. Positive lag means camera feature is later.</p>
  </section>;
}
