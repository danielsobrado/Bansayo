import { useEffect, useState } from "react";
import type { DeviceStatus, SkeletonDefinition, SkeletonPose, TrackingQuality } from "@contracts";
import { request } from "../../../engine/http";
import { Reasons } from "../../../ui/Reasons";
import { rotate } from "./trace";
import { quaternionAngle, MAX_PAIR_SKEW_US } from "./benchMath";
import { CaptureHistory } from "./CaptureHistory";

interface Result { pose: SkeletonPose|null; quality: TrackingQuality; t_ref:number|null; }
interface Preview { t_us:number; selected:string[]; skeleton:SkeletonDefinition; imu:Result; camera:Result; hybrid:Result; }

function Figure({ skeleton, result, label }: { skeleton:SkeletonDefinition;result:Result;label:string }) {
  const pose = result.pose;
  const project = (v:number[]) => [150+75*(v[0]??0),240-110*(v[1]??0)] as const;
  return <section className="lab-card">
    <header><h3>{label}</h3><span className="mono">{result.quality.observed_segments}/{result.quality.total_segments}</span></header>
    {pose ? <svg viewBox="0 0 300 260" className="lab-solver-figure" role="img" aria-label={label+" isolated solver skeleton"}>
      {skeleton.segments.map((def,i)=>{
        const s=pose.segments[i]; if(!s)return null;
        const a=project(s.position_m),delta=rotate(def.tail_m,s.rotation);
        const b=project(s.position_m.map((v,j)=>v+delta[j]!));
        const inferred=s.provenance==="inferred"||s.provenance==="unavailable";
        return <g key={def.id}><line x1={a[0]} y1={a[1]} x2={b[0]} y2={b[1]} stroke={inferred?"var(--prov-inferred)":"var(--prov-derived)"} strokeWidth="4" strokeDasharray={inferred?"4 3":undefined}/><circle cx={a[0]} cy={a[1]} r="3" fill="var(--ink-2)"/></g>;
      })}
    </svg>:<p>No solved pose.</p>}
    <Reasons reasons={result.quality.reasons}/>
  </section>;
}

export function SolverBench({ devices, live }: { devices:DeviceStatus[];live:boolean }) {
  const [ids,setIds]=useState(()=>devices.map(d=>d.descriptor.id));
  const [running,setRunning]=useState(false);
  const [segment,setSegment]=useState("");
  const [preview,setPreview]=useState<Preview|null>(null);
  const [error,setError]=useState<string|null>(null);
  const [received,setReceived]=useState(0);
  const selected=ids.filter(id=>devices.some(d=>d.descriptor.id===id));
  const key=selected.join("\n");
  useEffect(()=>{
    if(!running||!live||!key)return;
    const ctl=new AbortController();
    let timer:number|undefined;
    async function poll(){
      if(!document.hidden){
        const result=await request<Preview>("POST","/inspection/solver",{device_ids:key.split("\n")},
          (url,init)=>fetch(url,{...init,signal:AbortSignal.any([ctl.signal,AbortSignal.timeout(2000)])}));
        if(ctl.signal.aborted)return;
        if(result.ok){setPreview(result.data);setReceived(performance.now());setError(null);}
        else{setPreview(null);setError(result.error.message);}
      }
      timer=window.setTimeout(poll,500);
    }
    void poll();
    return()=>{ctl.abort();window.clearTimeout(timer);};
  },[running,live,key]);
  function choose(next:string[]){setIds(next);setPreview(null);setError(null);}
  const data=live&&preview&&preview.selected.length===selected.length&&preview.selected.every(id=>selected.includes(id))&&(performance.now()-received<2000)?preview:null;
  const activeSegment=data?.skeleton.segments.findIndex(s=>s.id===segment)??-1;
  const pick=activeSegment>=0?activeSegment:0;
  const segmentId=data?.skeleton.segments[pick]?.id??"";
  const x=data?.imu.pose?.segments[pick], y=data?.camera.pose?.segments[pick];
  const skew=x?.t_obs_us!=null&&y?.t_obs_us!=null?Math.abs(x.t_obs_us-y.t_obs_us):null;
  const valid=x&&y&&!["inferred","unavailable"].includes(x.provenance)&&!["inferred","unavailable"].includes(y.provenance);
  const angle=valid&&skew!==null&&skew<=MAX_PAIR_SKEW_US?quaternionAngle(x.rotation,y.rotation):null;
  return <div className="lab-main">
    <section className="lab-card">
      <h2>Solver ablation and comparison</h2>
      <p>Runs the real solver with only the selected device inputs. Live training and recording remain untouched. Each preview starts without accumulated heading correction or held root anchors.</p>
      <div className="lab-bench-controls">
        <button onClick={()=>choose(devices.map(d=>d.descriptor.id))}>All</button>
        <button onClick={()=>choose(devices.filter(d=>d.descriptor.kind==="imu_tracker").map(d=>d.descriptor.id))}>IMUs</button>
        <button onClick={()=>choose(devices.filter(d=>d.descriptor.kind==="camera").map(d=>d.descriptor.id))}>Cameras</button>
        <button onClick={()=>choose([])}>None</button>
      </div>
      <div className="lab-bench-controls">{devices.map(d=><label key={d.descriptor.id}><input type="checkbox" checked={selected.includes(d.descriptor.id)} onChange={e=>choose(e.target.checked?[...selected,d.descriptor.id]:selected.filter(id=>id!==d.descriptor.id))}/>{d.descriptor.id}</label>)}</div>
      <button disabled={!live||!selected.length||selected.length>32} aria-pressed={running} onClick={()=>{setRunning(!running);setPreview(null);}}>{running?"Stop preview":"Run preview (2 Hz)"}</button>
      {error&&<p className="error" role="alert">{error}</p>}
    </section>
    {data&&<>
      <div className="lab-grid"><Figure skeleton={data.skeleton} result={data.imu} label="IMU only"/><Figure skeleton={data.skeleton} result={data.camera} label="Camera only"/><Figure skeleton={data.skeleton} result={data.hybrid} label="Hybrid (cold start)"/></div>
      <section className="lab-card lab-bench-results">
        <h3>Optical vs IMU segment disagreement</h3>
        <p className="subtle">Only non-inferred segment orientations with observation timestamps within 40 ms. Disagreement is not ground-truth error; camera axial twist can be underconstrained.</p>
        <label>History segment <select value={segmentId} onChange={e=>setSegment(e.target.value)}>{data.skeleton.segments.map(s=><option key={s.id}>{s.id}</option>)}</select></label>
        <table><thead><tr><th>Segment</th><th>IMU method</th><th>Camera method</th><th>Skew</th><th>Angle</th></tr></thead><tbody>
          {data.skeleton.segments.map((s,i)=>{
            const a=data.imu.pose?.segments[i],b=data.camera.pose?.segments[i];
            const skew=a?.t_obs_us!=null&&b?.t_obs_us!=null?Math.abs(a.t_obs_us-b.t_obs_us):null;
            const measured=a&&b&&!["inferred","unavailable"].includes(a.provenance)&&!["inferred","unavailable"].includes(b.provenance);
            const angle=measured&&skew!==null&&skew<=MAX_PAIR_SKEW_US?quaternionAngle(a.rotation,b.rotation):null;
            return <tr key={s.id}><td>{s.id}</td><td>{a?.method??"—"}</td><td>{b?.method??"—"}</td><td>{skew===null?"—":(skew/1000).toFixed(1)+" ms"}</td><td>{angle===null?"—":angle.toFixed(1)+"°"}</td></tr>;
          })}
        </tbody></table>
      </section>
    </>}
    <CaptureHistory key={key+":"+segment} label={segmentId||segment||"Optical vs IMU"} enabled={running&&live&&selected.length>0} point={data&&angle!==null&&skew!==null?{t_us:data.t_us,value:angle,skew_ms:skew/1000}:null}/>
  </div>;
}
