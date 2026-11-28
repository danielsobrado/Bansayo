import {useState} from "react";
import type {DeviceStatus} from "@contracts";
import {contiguous,reliabilitySummary,type ReliabilityRow} from "./reliabilityMath";
import {profileReadiness} from "./calibrationQuality";
import {RELIABILITY} from "./reliabilityConfig";
import {downloadText} from "./signalMath";
type Metric="drift"|"offset"|"uncertainty"|"rate"|"dropped";
const fmt=(v:number|null)=>v===null?"—":v.toFixed(2);
function Trend({rows,metric,label}:{rows:ReliabilityRow[];metric:Metric;label:string}){
  const first=rows[0]?.at??0,span=Math.max(1,(rows.at(-1)?.at??first)-first);
  const values=rows.map((r,i)=>metric==="dropped"?(i&&contiguous(rows[i-1]!,r)?r.dropped-rows[i-1]!.dropped:null):r[metric]);
  const valid=values.filter((v):v is number=>v!==null);
  if(!valid.length)return <p>{label}: no evidence</p>;
  const min=Math.min(...valid),max=Math.max(...valid),range=Math.max(1e-9,max-min);
  const coords=rows.map((r,i)=>{const v=values[i];if(v==null)return "";const connected=i>0&&values[i-1]!=null&&contiguous(rows[i-1]!,r);return (connected?"L":"M")+`${(r.at-first)/span*320},${95-(v-min)/range*85}`;}).join(" ");
  return <div><h4>{label}</h4><p className="mono">{fmt(valid.at(-1)??null)} · range {fmt(min)}–{fmt(max)}</p><svg viewBox="0 0 320 105" className="lab-chart" role="img" aria-label={label+" history"}><path d="M0 95H320 M0 10H320" stroke="var(--line)"/><path d={coords} fill="none" stroke="var(--accent)" strokeWidth="1.5"/>{rows.length===1&&<circle cx="0" cy="95" r="3" fill="var(--accent)"/>}</svg></div>;
}
export function ReliabilityPanel({devices,rows,storageError,clear}:{devices:DeviceStatus[];rows:ReliabilityRow[];storageError:boolean;clear:()=>void}){
  const [choice,setChoice]=useState(""),[confirm,setConfirm]=useState(false);
  const ids=[...new Set([...devices.map(d=>d.descriptor.id),...rows.map(r=>r.device)])];
  const id=ids.includes(choice)?choice:ids[0]??"";
  const series=rows.filter(r=>r.device===id),latest=series.at(-1),summary=reliabilitySummary(series);
  const current=devices.find(d=>d.descriptor.id===id),quality=current?profileReadiness(current.calibration):null;
  return <div className="lab-main"><section className="lab-card"><header><h3>Sensor reliability history</h3></header>
    <p className="subtle">Browser-observed live status, sampled every five seconds when new device state is received. Up to {RELIABILITY.maxRows} rows across devices, retained for at most seven days in this browser. Not continuous uptime or a server audit log.</p>
    <div className="studio-controls"><label>Reliability device <select value={id} onChange={e=>setChoice(e.target.value)}>{ids.map(d=><option key={d}>{d}</option>)}</select></label><button disabled={!rows.length} onClick={()=>downloadText("bansayo-reliability.json",JSON.stringify({format:"bansayo-reliability-v1",rows},null,2),"application/json")}>Export reliability history</button><button onClick={()=>setConfirm(v=>!v)}>Clear history</button>{confirm&&<button onClick={()=>{clear();setConfirm(false);}}>Confirm clear</button>}</div>
    {storageError&&<p role="alert">History storage unavailable; observations remain in memory only.</p>}
    <p className="mono">{summary.samples} observed states · tracking fraction {summary.trackingFraction===null?"—":(100*summary.trackingFraction).toFixed(1)+"%"} · {summary.accepted} accepted / {summary.dropped} engine drops in {summary.intervals} comparable intervals</p>
    {latest&&<p className="mono">Last observation {new Date(latest.at).toISOString()} · {latest.origin} · {latest.state} · clock {latest.timeQuality} · {latest.settled?"settled":"not settled"} · resets {latest.resets??"—"}</p>}
    {quality&&<p>Calibration profile readiness: {quality.valid}/{quality.total} valid ({quality.percent??"—"}{quality.percent===null?"":"%"}). Profile validity is not measurement accuracy.</p>}
    <div className="lab-grid"><Trend rows={series} metric="drift" label="Engine clock drift (ppm)"/><Trend rows={series} metric="offset" label="Estimated clock offset (µs)"/><Trend rows={series} metric="uncertainty" label="Clock uncertainty (µs)"/><Trend rows={series} metric="rate" label="Observed rate (Hz)"/><Trend rows={series} metric="dropped" label="Engine observation drops per interval"/></div>
    <p className="subtle">Positive drift means the device clock runs slow. The engine continuously estimates from per-second receive-minus-device clock minima; the estimate is unavailable until sufficient evidence exists. Lines break across collection gaps, restarts and resets. Counter deltas are observations, not UDP datagrams.</p>
    <div className="studio-stats"><table><thead><tr><th>Time (UTC)</th><th>State</th><th>Rate Hz</th><th>Drift ppm</th><th>Clock resets</th><th>Dropped (total)</th></tr></thead><tbody>{series.slice(-20).reverse().map((r,i)=><tr key={r.run+":"+r.at+":"+i}><td>{new Date(r.at).toISOString()}</td><td>{r.state}</td><td>{fmt(r.rate)}</td><td>{fmt(r.drift)}</td><td>{r.resets??"—"}</td><td>{r.dropped}</td></tr>)}</tbody></table></div>
  </section></div>;
}
