import { useId, useState } from "react";
import type { DeviceStatus } from "@contracts";
import { Icon } from "../../../ui/Icon";
import { CHANNEL_IDS, CHANNELS, type ChannelId, type Snapshot } from "./trace";
import { deviceChannels } from "./DeviceExplorer";
import { downloadText, nearestPoint, sharedEnd, signalCsv, stats, windowPoints } from "./signalMath";
import { AXIS_CSS, HISTORY_SECONDS, TIME_WINDOWS, type SignalFocus } from "./studioConfig";

const LEFT=42,RIGHT=352,TOP=10,BOTTOM=118;
const INITIAL_AXES=[0,1,2,3];

function Instrument({id,channel,end,seconds,cursor,onCursor,expanded,onExpand}:{
  id:ChannelId;channel:Snapshot["channels"][ChannelId];end:number;seconds:number;cursor:number;onCursor:(v:number)=>void;expanded:boolean;onExpand:()=>void;
}) {
  const meta=CHANNELS[id], [axes,setAxes]=useState(INITIAL_AXES),[range,setRange]=useState("");
  const active=axes.filter(i=>i<meta.axes.length);
  const points=windowPoints(channel?.points??[],end,seconds);
  const sample=nearestPoint(points,end-cursor*1e6);
  const values=points.flatMap(p=>active.map(i=>p.values[i]!));
  const auto=Math.max(.001,...values.map(Math.abs));
  const fixed=Number(range),limit=Number.isFinite(fixed)&&fixed>0?fixed:auto*1.1;
  const x=(t:number)=>LEFT+(t-(end-seconds*1e6))/(seconds*1e6)*(RIGHT-LEFT);
  const y=(v:number)=>BOTTOM-(v+limit)/(2*limit)*(BOTTOM-TOP);
  const cursorX=x(end-cursor*1e6);
  const clipId=useId();
  return <section className="lab-card studio-scope">
    <header><div><h3>{meta.title}</h3><small className="subtle">{meta.unit} · {channel?"Sampled history":"Unavailable"}</small></div><button aria-label={expanded?"Restore "+meta.title:"Maximize "+meta.title} onClick={onExpand}><Icon name={expanded?"grid":"expand"}/></button></header>
    <div className="studio-scope-controls">{meta.axes.map((axis,i)=><label key={axis} style={{color:AXIS_CSS[i]}}><input type="checkbox" checked={axes.includes(i)} onChange={e=>setAxes(v=>e.target.checked?[...v,i]:v.filter(n=>n!==i))}/>{axis}</label>)}<label className="studio-range">± range <input type="number" min="0" step="any" aria-label={meta.title+" fixed range"} placeholder="Auto" value={range} onChange={e=>setRange(e.target.value)}/></label></div>
    <svg viewBox="0 0 362 142" className="lab-chart studio-chart" role="img" aria-label={meta.title+" synchronized signal plot"} onPointerDown={e=>{
      const matrix=e.currentTarget.getScreenCTM();
      if(!matrix)return;
      const px=new DOMPoint(e.clientX,e.clientY).matrixTransform(matrix.inverse()).x;
      onCursor(Math.max(0,Math.min(seconds,(RIGHT-px)/(RIGHT-LEFT)*seconds)));
    }}>
      <defs><clipPath id={clipId}><rect x={LEFT} y={TOP} width={RIGHT-LEFT} height={BOTTOM-TOP}/></clipPath></defs>
      {[0,1,2,3,4].map(i=>{const v=limit*(1-i/2);return <g key={i}><line x1={LEFT} x2={RIGHT} y1={y(v)} y2={y(v)} stroke="var(--line)" strokeDasharray={i===2?"4 3":undefined}/><text x={LEFT-5} y={y(v)+3} textAnchor="end" fill="var(--ink-3)" fontSize="9">{v.toPrecision(3)}</text></g>;})}
      {[0,1,2,3,4,5].map(i=><g key={i}><line x1={LEFT+i*(RIGHT-LEFT)/5} x2={LEFT+i*(RIGHT-LEFT)/5} y1={TOP} y2={BOTTOM} stroke="var(--line)"/><text x={LEFT+i*(RIGHT-LEFT)/5} y="134" textAnchor="middle" fill="var(--ink-3)" fontSize="9">{(-seconds+i*seconds/5).toFixed(1)}</text></g>)}
      <g clipPath={`url(#${clipId})`}>{active.map(i=>{
        let path="",previous=-Infinity;
        for(const p of points){path+=`${p.t_us-previous>150000?"M":"L"}${x(p.t_us).toFixed(2)},${y(p.values[i]!).toFixed(2)} `;previous=p.t_us;}
        return <path key={i} d={path} stroke={AXIS_CSS[i]} fill="none" strokeWidth="1.4" vectorEffect="non-scaling-stroke"/>;
      })}<line x1={cursorX} x2={cursorX} y1={TOP} y2={BOTTOM} stroke="var(--ink-2)" strokeDasharray="3 3"/></g>
      {!points.length&&<text x="195" y="68" textAnchor="middle" fill="var(--ink-3)" fontSize="12">No samples in this window</text>}
    </svg>
    <div className="studio-scope-values">{active.map(i=><span key={i} style={{color:AXIS_CSS[i]}}>{meta.axes[i]} <strong>{sample?.values[i]?.toFixed(3)??"—"}</strong></span>)}</div>
    {expanded&&<table className="studio-stats"><thead><tr><th>Axis</th><th>Min</th><th>Max</th><th>Mean</th><th>RMS</th><th>σ</th></tr></thead><tbody>{active.map(i=>{const s=stats(points.map(p=>p.values[i]!));return <tr key={i}><th>{meta.axes[i]}</th>{[s?.min,s?.max,s?.mean,s?.rms,s?.std].map((v,j)=><td key={j}>{v?.toFixed(3)??"—"}</td>)}</tr>;})}</tbody></table>}
    <small className="subtle mono">{points.length} samples · {sample?`seq ${sample.seq} · cursor sample`:"No nearby cursor sample"}</small>
  </section>;
}
export function SignalWorkspace({device,channels,known,focus,onFocus,compact=false,frozen}:{
  device:DeviceStatus;channels:Snapshot["channels"];known:Snapshot["channels"];focus:SignalFocus;onFocus:(focus:SignalFocus)=>void;compact?:boolean;frozen:boolean;
}) {
  const [seconds,setSeconds]=useState<number>(HISTORY_SECONDS);
  const [cursor,setCursor]=useState(0);
  const [offset,setOffset]=useState(0);
  const [maximized,setMaximized]=useState<ChannelId|null>(null);
  const knownIds=CHANNEL_IDS.filter(id=>known[id]!==undefined||deviceChannels(device).includes(id));
  const primary=(["gyro","accel","orientation","pose_rotation","position","distance","contact"] as const).filter(id=>knownIds.includes(id));
  const base=compact?primary.slice(0,2):focus==="all"?knownIds:[focus];
  const ids=maximized&&base.includes(maximized)?[maximized]:base;
  const latest=sharedEnd(channels);
  const end=latest-Math.min(offset,HISTORY_SECONDS-seconds)*1e6;
  const selected=Math.min(cursor,seconds);
  return <section className="studio-signals" aria-label="Signal analyzer">
    <div className="studio-subbar">
      <strong>{compact?"Live signals":"Signal analyzer"}</strong>
      {!compact&&<select aria-label="Signal component" value={focus} onChange={e=>{onFocus(e.target.value as SignalFocus);setMaximized(null);}}><option value="all">All components</option>{knownIds.map(id=><option key={id} value={id}>{CHANNELS[id].title}</option>)}</select>}
      <label>Window <select aria-label="Signal time window" value={seconds} onChange={e=>{setSeconds(Number(e.target.value));setCursor(0);setOffset(0);}}>{TIME_WINDOWS.map(v=><option key={v} value={v}>{v} s</option>)}</select></label>
      {seconds<HISTORY_SECONDS&&<label>History offset <input aria-label="Signal history offset" type="range" min={0} max={HISTORY_SECONDS-seconds} step={0.1} value={offset} onChange={e=>setOffset(Number(e.target.value))}/><span className="mono">{offset.toFixed(1)} s</span></label>}
      <button disabled={!latest||!ids.length} onClick={()=>downloadText(device.descriptor.id+"-signals.csv",signalCsv(channels,ids,end,seconds),"text/csv")}>Export CSV</button>
      <small className="subtle">{frozen?"Frozen":"Live"} · sampled, not full-rate</small>
    </div>
    <div className="studio-scope-grid" data-maximized={!!maximized}>{ids.map(id=><Instrument key={id} id={id} channel={channels[id]} end={end} seconds={seconds} cursor={selected} onCursor={setCursor} expanded={maximized===id} onExpand={()=>setMaximized(v=>v===id?null:id)}/>)}</div>
    {!ids.length&&<p className="studio-empty">No supported signals announced by this device.</p>}
    <div className="studio-timeline"><Icon name="crosshair"/><label htmlFor={compact?"overview-cursor":"signal-cursor"}>Shared cursor</label><input id={compact?"overview-cursor":"signal-cursor"} aria-label="Shared signal cursor" type="range" min={0} max={seconds} step={0.01} value={seconds-selected} onChange={e=>setCursor(seconds-Number(e.target.value))}/><span className="mono" title="Offset from newest sample">−{(selected+offset).toFixed(2)} s</span><button onClick={()=>{setCursor(0);setOffset(0);}}>Latest</button></div>
  </section>;
}
