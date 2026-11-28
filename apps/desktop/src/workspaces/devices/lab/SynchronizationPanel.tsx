import {useMemo,useState} from "react";
import type {Capture} from "./captureTypes";
import {CHANNELS,type Point} from "./trace";
import {compareTiming,scalarPoints,signalHealth,type SyncAxis,type SyncResult} from "./syncMath";
import {SYNC_ANALYSIS} from "./syncConfig";
import {downloadText} from "./signalMath";

type Channel="gyro"|"accel";
const fmt=(v:number|null,digits=2)=>v===null?"—":v.toFixed(digits);
function ComparisonPlot({result,label}:{result:SyncResult;label:string}){
  if(!result.overlay.length)return null;
  const points=result.overlay,span=Math.max(1,points.at(-1)!.t_us-points[0]!.t_us);
  let lo=Infinity,hi=-Infinity;
  for(const p of points){lo=Math.min(lo,p.a,p.b);hi=Math.max(hi,p.a,p.b);}
  const series=(key:"a"|"b")=>points.map(p=>`${20+(p.t_us-points[0]!.t_us)/span*340},${100-(p[key]-lo)/Math.max(1e-9,hi-lo)*90}`).join(" ");
  return <><svg viewBox="0 0 380 120" className="lab-chart" role="img" aria-label={label}>
    {[10,40,70,100].map(y=><line key={y} x1="20" x2="360" y1={y} y2={y} stroke="var(--line)"/>)}
    <polyline points={series("a")} fill="none" stroke="var(--studio-axis-x)" strokeWidth="1.5"/>
    <polyline points={series("b")} fill="none" stroke="var(--studio-axis-z)" strokeWidth="1.5"/>
  </svg><small className="mono subtle">Unshifted · {lo.toPrecision(3)} to {hi.toPrecision(3)} · {(span/1e6).toFixed(2)} s · reference (X color), test (Z color)</small></>;
}
export function SynchronizationPanel({capture}:{capture:Capture}){
  const [channel,setChannel]=useState<Channel>("gyro"),[axis,setAxis]=useState<SyncAxis>("magnitude"),[reference,setReference]=useState("");
  const groups=useMemo(()=>{
    const result=new Map<string,{points:Point[];epochs:Set<number>}>();
    for(const s of capture.samples){
      if(s.channel!==channel)continue;
      if(capture.window&&(s.point.t_us<capture.window.start_us||s.point.t_us>=capture.window.end_us))continue;
      let g=result.get(s.device);if(!g){g={points:[],epochs:new Set()};result.set(s.device,g);}
      g.points.push(s.point);g.epochs.add(s.epoch);
    }
    return result;
  },[capture,channel]);
  const ids=[...groups.keys()],id=groups.has(reference)?reference:ids[0]??"";
  const report=useMemo(()=>{
    const data=[...groups].map(([device,g])=>({device,epochs:g.epochs.size,points:scalarPoints(g.points,axis)}));
    const a=data.find(d=>d.device===id);
    return {format:"bansayo-sensor-sync-v1",started:capture.started,reason:capture.reason,window:capture.window??null,lost:capture.lost,resets:capture.resets,channel,axis,reference:id,settings:SYNC_ANALYSIS,
      health:data.map(d=>({device:d.device,epochs:d.epochs,...signalHealth(d.points)})),
      comparisons:data.filter(d=>d!==a).map(d=>({device:d.device,result:a&&a.epochs===1&&d.epochs===1&&capture.resets===0&&capture.lost===0?compareTiming(a.points,d.points):null}))};
  },[groups,axis,id,channel,capture]);
  return <section className="lab-card"><header><h3>Multi-device timing and signal health</h3></header>
    <p>Mount sensors on the same rigid object and use distinctive motion. Magnitude avoids assuming axis alignment; axes require aligned mounting. Positive lag means the test signal is later in mapped engine time.</p>
    <div className="studio-subbar">
      <label>Sync channel <select value={channel} onChange={e=>setChannel(e.target.value as Channel)}><option value="gyro">Gyroscope</option><option value="accel">Accelerometer</option></select></label>
      <label>Sync signal <select value={axis} onChange={e=>setAxis(e.target.value==="magnitude"?"magnitude":Number(e.target.value) as SyncAxis)}><option value="magnitude">Magnitude</option>{["X","Y","Z"].map((a,i)=><option key={a} value={i}>{a}</option>)}</select></label>
      <label>Sync reference <select value={id} onChange={e=>setReference(e.target.value)}>{ids.map(d=><option key={d}>{d}</option>)}</select></label>
      <button disabled={!ids.length} onClick={()=>downloadText("bansayo-sensor-sync.json",JSON.stringify(report,null,2),"application/json")}>Export sync report</button>
    </div>
    <div className="studio-stats"><table><thead><tr><th>Device</th><th>Samples</th><th>Rate Hz</th><th>Median ms</th><th>P95 ms</th><th>Long intervals</th><th>Non-increasing</th><th>Mean</th><th>Std dev</th><th>RMS</th></tr></thead><tbody>
      {report.health.map(h=><tr key={h.device}><td>{h.device}</td><td>{h.count}</td><td>{fmt(h.rateHz)}</td><td>{fmt(h.medianIntervalMs)}</td><td>{fmt(h.p95IntervalMs)}</td><td>{h.longIntervals}</td><td>{h.nonIncreasing}</td><td>{fmt(h.mean,3)}</td><td>{fmt(h.std,3)}</td><td>{fmt(h.rms,3)}</td></tr>)}
    </tbody></table></div>
    <small className="subtle">Signal statistics in {CHANNELS[channel].unit}, over all selected recorded observations. Long interval: &gt;1.5× median. Standard deviation during motion is not sensor noise.</small>
    {ids.length<2&&<p>Record at least two devices with the selected channel for timing comparison.</p>}
    {(capture.lost>0||capture.resets>0)&&<p className="lab-result-warn">Capture reported loss or reference reset; lag estimation is disabled.</p>}
    {report.comparisons.map(({device,result})=><div key={device} className="studio-pinned"><h4>{id} vs {device}</h4>
      {result?<><p className="mono">Lag {fmt(result.lagMs)} ms · peak r {fmt(result.correlation,3)} · zero-lag r {fmt(result.zeroCorrelation,3)} · grid {fmt(result.stepUs===null?null:result.stepUs/1000)} ms · {result.pairs} pairs</p><p className={result.status==="estimated"?"subtle":"lab-result-warn"}>{result.reason}</p><ComparisonPlot result={result} label={id+" vs "+device+" unshifted signals"}/></>:<p>Reference changed or capture integrity is insufficient.</p>}
    </div>)}
    <p className="subtle">Latest common interval up to 8 s; ±200 ms search on a grid no faster than the slower median sample rate or 200 Hz. Linear interpolation, no gap bridging; fixed comparison sample count. No automatic clock correction, anti-alias filter, hardware synchronization or physical latency claim. Repeat with independent reference events.</p>
  </section>;
}
