import { useState } from "react";
import type { SkeletonDefinition,SkeletonPose,TrackingQuality } from "@contracts";
import { Reasons } from "../../../ui/Reasons";
import { rotate } from "./trace";
import { Comparison3D } from "./Comparison3D";
import { Icon } from "../../../ui/Icon";

interface Result {pose:SkeletonPose|null;quality:TrackingQuality}
type View = "Front"|"Side"|"Top"|"3D";
const LABELS=["IMU only","Camera only","Hybrid (cold start)"];
const COLORS=["var(--accent)","var(--prov-inferred)","var(--state-tracking)"];

function Bones({skeleton,result,view,selected,onSelect,color,rootRelative=false}:{
  skeleton:SkeletonDefinition;result:Result;view:View;selected:string;onSelect:(id:string)=>void;color?:string;rootRelative?:boolean;
}) {
  if(!result.pose)return null;
  const pose=result.pose;
  const scale=105/Math.max(.5,skeleton.height_m);
  const root=pose.segments[0]?.position_m??[0,0,0];
  const project=(v:number[])=>{
    const [x,y,z]=v.map((n,i)=>n-(rootRelative?(root[i]??0):0)) as [number,number,number];
    const shift=rootRelative?130:224;
    return view==="Top"?[140+x*scale*1.7,130+z*scale*1.7]:[140+(view==="Front"?-x:z)*scale*1.7,shift-y*scale*1.7];
  };
  return <g>{skeleton.segments.map((def,i)=>{
    const s=pose.segments[i];if(!s||s.provenance==="unavailable")return null;
    const a=project(s.position_m),d=rotate(def.tail_m,s.rotation),b=project(s.position_m.map((v,j)=>v+d[j]!));
    const inferred=s.provenance==="inferred";
    return <g key={def.id} role="button" tabIndex={0} aria-label={"Inspect "+def.id} onClick={()=>onSelect(def.id)} onKeyDown={e=>{if(e.key==="Enter"||e.key===" "){e.preventDefault();onSelect(def.id);}}}>
      <title>{def.id} · {s.provenance}</title>
      <line x1={a[0]} y1={a[1]} x2={b[0]} y2={b[1]} stroke="transparent" strokeWidth="16"/>
      <line x1={a[0]} y1={a[1]} x2={b[0]} y2={b[1]} stroke={selected===def.id?"var(--state-degraded)":color??(inferred?"var(--prov-inferred)":"var(--prov-derived)")} strokeWidth={selected===def.id?6:4} strokeLinecap="round" strokeDasharray={inferred?"4 3":undefined}/>
      <circle cx={a[0]} cy={a[1]} r="2.6" fill="var(--ink-2)"/>
    </g>;
  })}</g>;
}
function Grid(){return <g stroke="var(--line)" opacity=".65">{[20,60,100,140,180,220,260].map(x=><line key={x} x1={x} x2={x} y1="12" y2="248"/>)}{[24,64,104,144,184,224].map(y=><line key={y} x1="12" x2="268" y1={y} y2={y}/>)}</g>;}
export function ComparisonFigures({skeleton,results,selected,onSelect}:{
  skeleton:SkeletonDefinition;results:Result[];selected:string;onSelect:(id:string)=>void;
}) {
  const [view,setView]=useState<View>("Front"),[overlay,setOverlay]=useState(false);
  return <section className="lab-card">
    <header><h3>Reconstruction comparison</h3><div className="studio-controls">
      {(["Front","Side","Top","3D"] as const).map(v=><button key={v} aria-pressed={view===v} onClick={()=>setView(v)}>{v}</button>)}
      <button aria-pressed={overlay} onClick={()=>setOverlay(v=>!v)}><Icon name="layers"/>Overlay</button>
    </div></header>
    {view==="3D"?<Comparison3D skeleton={skeleton} poses={results.map(r=>r.pose)} selected={selected} overlay={overlay}/>:<div className="studio-comparison-grid" data-overlay={overlay}>
      {overlay?<div className="studio-comparison-figure"><svg viewBox="0 0 280 260" className="lab-solver-figure" role="group" aria-label="Root-aligned reconstruction overlay"><Grid/>{results.map((result,i)=><Bones key={i} skeleton={skeleton} result={result} view={view} selected={selected} onSelect={onSelect} color={COLORS[i]} rootRelative/>)}</svg>
        <div className="studio-controls">{LABELS.map((label,i)=><span key={label} style={{color:COLORS[i]}}>{label}</span>)}</div><small className="subtle">Root-aligned overlay; translation differences are intentionally removed.</small>
      </div>:results.map((result,i)=><div className="studio-comparison-figure" key={i}>
        <div className="studio-test-head"><strong>{LABELS[i]}</strong><small className="mono">{result.quality.observed_segments}/{result.quality.total_segments}</small></div>
        <svg viewBox="0 0 280 260" className="lab-solver-figure" role="group" aria-label={LABELS[i]+" reconstruction"}><Grid/>{result.pose?<Bones skeleton={skeleton} result={result} view={view} selected={selected} onSelect={onSelect}/>:<text x="140" y="130" textAnchor="middle" fill="var(--ink-3)" fontSize="12">No solved pose</text>}</svg>
        <Reasons reasons={result.quality.reasons}/>
      </div>)}
    </div>}
    {view==="3D"&&<div className="studio-controls">{LABELS.map((label,i)=><span key={label} style={{color:COLORS[i]}}>{label}</span>)}</div>}
    <p className="subtle">Shared view and scale. In overlay mode, translation differences are removed. Dashed segments are inferred; unavailable segments are omitted. Select a segment to inspect disagreement below.</p>
  </section>;
}
