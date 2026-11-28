import {useEffect,useMemo,useState} from "react";
import type {Capture} from "./captureTypes";
import {STATIONARY_DEFAULTS,STATIONARY_KEYS,STATIONARY_LABELS,loadLimits,saveLimits,validLimits,type StationaryLimits} from "./stationaryConfig";
import {stationaryReport} from "./stationaryMath";
import {downloadText} from "./signalMath";
const fmt=(v:number|null)=>v===null?"—":v.toPrecision(4);
export function StationaryPanel({capture}:{capture:Capture}){
  const [limits,setLimits]=useState(loadLimits);
  const [confirmed,setConfirmed]=useState<Capture|null>(null);
  const valid=validLimits(limits);
  useEffect(()=>{if(valid)saveLimits(limits);},[limits,valid]);
  const ids=useMemo(()=>[...new Set(capture.samples.map(s=>s.device))],[capture]);
  const reports=useMemo(()=>valid?ids.map(id=>stationaryReport(capture,id,limits,confirmed===capture)):[],[capture,ids,limits,confirmed,valid]);
  return <section className="lab-card"><header><h3>Stationary noise and drift acceptance</h3></header>
    <p>Record a rigid stationary tracker after warm-up. These configurable limits are preliminary engineering screens, not manufacturer specifications or physical certification. Standard deviation includes slow changes; slope is an observed linear trend.</p>
    <label><input type="checkbox" checked={confirmed===capture} onChange={e=>setConfirmed(e.target.checked?capture:null)}/> Confirm this entire recording was stationary</label>
    <details><summary>Acceptance thresholds</summary><div className="lab-grid">{STATIONARY_KEYS.map(k=><label key={k}>{STATIONARY_LABELS[k]} <input type="number" min="0.000001" step="any" value={Number.isFinite(limits[k])?limits[k]:""} onChange={e=>setLimits(v=>({...v,[k]:e.target.valueAsNumber} as StationaryLimits))}/></label>)}</div><button onClick={()=>setLimits({...STATIONARY_DEFAULTS})}>Reset thresholds</button></details>
    {!valid&&<p role="alert">Enter finite positive limits within the supported duration and rate.</p>}
    <button disabled={!valid||!reports.length} onClick={()=>downloadText("bansayo-stationary-report.json",JSON.stringify({format:"bansayo-stationary-v1",started:capture.started,reason:capture.reason,window:capture.window??null,lost:capture.lost,resets:capture.resets,operatorConfirmed:confirmed===capture,reports},null,2),"application/json")}>Export stationary report</button>
    {reports.map(r=><div key={r.device} className="studio-pinned"><h4>{r.device} · {r.status.toUpperCase()}</h4>{r.reasons.map(reason=><p key={reason} className="lab-result-warn">{reason}</p>)}
      <div className="studio-stats"><table><thead><tr><th>Gate</th><th>Measured</th><th>Limit</th><th>Result</th></tr></thead><tbody>{r.gates.map(g=><tr key={g.name}><td>{g.name}</td><td>{fmt(g.value)}</td><td>{g.direction==="min"?"≥":"≤"} {g.limit}</td><td>{r.status==="inconclusive"?"Not accepted":g.pass?"Pass":"Fail"}</td></tr>)}</tbody></table></div>
      <div className="studio-stats"><table><thead><tr><th>Channel / axis</th><th>Mean</th><th>Std dev</th><th>RMS</th><th>Trend / min</th></tr></thead><tbody>{[["Gyro",r.gyroAxes],["Accel",r.accelAxes]].flatMap(([name,axes])=>(axes as typeof r.gyroAxes).map((a,i)=><tr key={name+String(i)}><td>{String(name)} {"XYZ"[i]}</td><td>{fmt(a.mean)}</td><td>{fmt(a.std)}</td><td>{fmt(a.rms)}</td><td>{fmt(a.slopePerMinute)}</td></tr>))}</tbody></table></div>
      <p className="mono">Final orientation difference {fmt(r.finalAngle)}° · maximum excursion {fmt(r.orientationMax)}°</p>
    </div>)}
  </section>;
}
