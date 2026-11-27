import { useRef,useState } from "react";
import { CHANNELS,type ChannelId } from "./trace";
import type { Capture,Sample } from "./captureTypes";
import { AXIS_CSS } from "./studioConfig";
const WIDTH=720,HEIGHT=180,BINS=600;

export function Timeline({capture}:{capture:Capture}) {
  const [device,setDevice]=useState(""),[channel,setChannel]=useState<ChannelId>("gyro"),[axis,setAxis]=useState(0);
  const [windowSec,setWindowSec]=useState(30),[offset,setOffset]=useState(0);
  const index=useRef<{capture:Capture;count:number;groups:Map<string,Map<ChannelId,Sample[]>>;min:number;max:number}|null>(null);
  if(!index.current||index.current.capture!==capture)index.current={capture,count:0,groups:new Map(),min:Infinity,max:0};
  const indexed=index.current;
  const groups=indexed.groups;
  for(let i=indexed.count;i<capture.samples.length;i++){
    const sample=capture.samples[i]!;
    let channels=groups.get(sample.device);if(!channels){channels=new Map();groups.set(sample.device,channels);}
    let points=channels.get(sample.channel);if(!points){points=[];channels.set(sample.channel,points);}points.push(sample);
    indexed.min=Math.min(indexed.min,sample.point.t_us);indexed.max=Math.max(indexed.max,sample.point.t_us);
  }
  indexed.count=capture.samples.length;
  const id=groups.has(device)?device:groups.keys().next().value??"";
  const channels=groups.get(id);
  const selected=channels?.has(channel)?channel:channels?.keys().next().value??"gyro";
  const points=channels?.get(selected)??[];
  const a=Math.min(axis,CHANNELS[selected].axes.length-1);
  const bounds={min:Number.isFinite(indexed.min)?indexed.min:0,max:indexed.max};
  const duration=(bounds.max-bounds.min)/1e6;
  const span=Math.max(.001,Math.min(windowSec,duration));
  const end=bounds.max-Math.min(offset,Math.max(0,duration-span))*1e6,start=end-span*1e6;
  const buckets=Array.from({length:BINS},()=>({min:Infinity,max:-Infinity}));
  let limit=.001;
  for(const p of points){const t=p.point.t_us,v=p.point.values[a]!;if(t<start||t>end)continue;const bin=buckets[Math.min(BINS-1,Math.floor((t-start)/(span*1e6)*BINS))]!;bin.min=Math.min(bin.min,v);bin.max=Math.max(bin.max,v);limit=Math.max(limit,Math.abs(v));}
  const y=(v:number)=>HEIGHT/2-v/limit*(HEIGHT/2-10);
  return <section className="lab-card"><header><h3>Recorded session timeline</h3><small className="subtle">Min/max envelope · shared session timestamps</small></header>
    <div className="studio-subbar"><label>Device <select value={id} onChange={e=>setDevice(e.target.value)}>{[...groups.keys()].map(d=><option key={d}>{d}</option>)}</select></label><label>Channel <select value={selected} onChange={e=>{setChannel(e.target.value as ChannelId);setAxis(0);}}>{[...(channels?.keys()??[])].map(c=><option key={c} value={c}>{CHANNELS[c].title}</option>)}</select></label><label>Axis <select value={a} onChange={e=>setAxis(Number(e.target.value))}>{CHANNELS[selected].axes.map((v,i)=><option key={v} value={i}>{v}</option>)}</select></label><label>Window <select value={windowSec} onChange={e=>setWindowSec(Number(e.target.value))}>{[5,30,60,300,600,1800,3600].map(v=><option key={v} value={v}>{v} s</option>)}</select></label></div>
    <svg viewBox={`0 0 ${WIDTH} ${HEIGHT}`} className="studio-capture-chart" role="img" aria-label="Recorded signal and event markers">
      {[.1,.3,.5,.7,.9].map(v=><line key={v} x1="0" x2={WIDTH} y1={v*HEIGHT} y2={v*HEIGHT} stroke="var(--line)"/>)}
      <path d={buckets.map((b,i)=>Number.isFinite(b.min)?`M${i/BINS*WIDTH},${y(b.min)}V${y(b.max)}`:"").join(" ")} stroke={AXIS_CSS[a]} fill="none" strokeWidth="1.4"/>
      {capture.markers.filter(m=>m.t_us>=start&&m.t_us<=end).map((m,i)=>{const x=(m.t_us-start)/(span*1e6)*WIDTH;return <g key={i}><title>{m.label}</title><line x1={x} x2={x} y1="0" y2={HEIGHT} stroke="var(--state-degraded)" strokeDasharray="4 3"/><circle cx={x} cy="8" r="4" fill="var(--state-degraded)"/></g>;})}
    </svg>
    <div className="studio-timeline"><span className="mono">±{limit.toPrecision(3)} {CHANNELS[selected].unit}</span><input aria-label="Recorded timeline position" type="range" min={0} max={Math.max(0,duration-span)} step=".1" value={Math.max(0,duration-span-offset)} onChange={e=>setOffset(Math.max(0,duration-span-Number(e.target.value)))}/><button onClick={()=>setOffset(0)}>Latest</button></div>
    <small className="mono subtle">{((start-bounds.min)/1e6).toFixed(2)}–{((end-bounds.min)/1e6).toFixed(2)} s / {duration.toFixed(2)} s · blank bins have no samples</small>
    <div className="studio-marker-list">{capture.markers.map((m,i)=><button key={i} onClick={()=>setOffset(Math.max(0,(bounds.max-m.t_us)/1e6-span/2))}>{((m.t_us-bounds.min)/1e6).toFixed(2)} s · {m.device?m.device+": ":""}{m.label}</button>)}</div>
  </section>;
}
