import { useEffect,useState } from "react";
import type { DeviceStatus } from "@contracts";
import { type Channel,type Timing } from "./trace";
import { GRAVITY } from "./benchMath";

function percentile(values:number[],p:number) {
  const sorted=[...values].sort((a,b)=>a-b);
  return sorted[Math.min(sorted.length-1,Math.floor(p*(sorted.length-1)))];
}
function Histogram({ values, label }: {values:number[];label:string}) {
  if(!values.length)return <p>Waiting for intervals.</p>;
  const max=Math.max(...values,1), bins=Array.from({length:16},()=>0);
  for(const value of values){const i=Math.min(15,Math.floor(value/max*16));bins[i]=(bins[i]??0)+1;}
  const peak=Math.max(...bins,1);
  return <div><p>{label} · p50 {percentile(values,.5)?.toFixed(2)} ms · p95 {percentile(values,.95)?.toFixed(2)} ms</p>
    <div className="lab-histogram" role="img" aria-label={label+" histogram"}>{bins.map((n,i)=><span key={i} style={{height:`${n/peak*100}%`}} title={`${(i*max/16).toFixed(1)}–${((i+1)*max/16).toFixed(1)} ms: ${n}`}/>)}</div>
    <div className="lab-compare-values mono"><small>0 ms</small><small>{max.toFixed(1)} ms</small></div>
  </div>;
}
export function TimingPanel({timing}:{timing:Timing|undefined}) {
  return <section className="lab-card"><h3>Packet timing</h3>
    {timing?<><p className="mono">{timing.sequence_gaps} cumulative orientation sequence gaps · {timing.intervals_ms.length} recent intervals</p>
      <Histogram label="Mapped sample intervals" values={timing.intervals_ms.map(p=>p[0])}/>
      <Histogram label="Pipeline acceptance intervals" values={timing.intervals_ms.map(p=>p[1])}/>
      <p className="subtle">Includes queue and scheduling jitter; sequence gaps are not exclusively Wi-Fi losses.</p></>:<p>Waiting for fresh orientation timing.</p>}
  </section>;
}
function Magnitude({channel,label}:{channel:Channel|undefined;label:string}) {
  const points=channel?.points??[], values=points.map(p=>Math.hypot(...p.values)), last=values.at(-1);
  if(last===undefined)return <p>{label}: unavailable</p>;
  const max=Math.max(1,...values);
  const coords=values.map((v,i)=>`${i/Math.max(1,values.length-1)*240},${60-v/max*55}`).join(" ");
  return <div><p className="mono">{label}: {last.toFixed(1)} BSP units</p><svg viewBox="0 0 240 64" className="lab-chart" role="img" aria-label={label+" magnetic magnitude history"}><polyline points={coords} fill="none" stroke="var(--accent)" strokeWidth="1.5"/></svg></div>;
}
export function MagneticPanel({a,b}:{a:Channel|undefined;b:Channel|undefined}) {
  const x=a?.points.at(-1),y=b?.points.at(-1);
  const delta=x&&y&&Math.abs(x.t_us-y.t_us)<=100000?Math.abs(Math.hypot(...x.values)-Math.hypot(...y.values)):null;
  return <section className="lab-card"><h3>Magnetic field / interference</h3>
    <Magnitude channel={a} label="BMM150 #0"/><Magnitude channel={b} label="BMM150 #1"/>
    <p className="mono">Magnitude difference: {delta?.toFixed(1)??"—"} BSP units</p>
    <p className="subtle">Observe field changes near metal and magnets. Different placement, offsets and scaling can affect the difference; no calibrated pass/fail threshold.</p>
  </section>;
}
export function SaturationPanel({gyro,accel,native}:{gyro:Channel|undefined;accel:Channel|undefined;native:boolean}) {
  const peak=(c:Channel|undefined)=>c?.points.length?Math.max(...c.points.flatMap(p=>p.values.map(Math.abs))):null;
  const g=peak(gyro),a=peak(accel);
  const rangeG=2000*Math.PI/180,rangeA=16*GRAVITY;
  return <section className="lab-card"><h3>Range and saturation</h3>
    <p className="mono">Gyro peak axis: {g?.toFixed(2)??"—"} rad/s</p><p className="mono">Accel peak axis: {a?.toFixed(2)??"—"} m/s²</p>
    {native?<><p className={(g!==null&&g>=rangeG*.95)||(a!==null&&a>=rangeA*.95)?"lab-result-warn":"subtle"}>{g===null&&a===null?"Waiting for samples":(g!==null&&g>=rangeG*.95)||(a!==null&&a>=rangeA*.95)?"Near configured full scale":"No near-range sample observed"}</p><p className="subtle">Assumes Mosaico firmware ranges ±16 g / ±2000°/s; decimated samples may miss clipping.</p></>:<p className="subtle">Hardware range unknown; no saturation classification.</p>}
  </section>;
}
export function BatteryHistory({device,enabled}:{device:DeviceStatus;enabled:boolean}) {
  const [history,setHistory]=useState<{t:number;v:number}[]>([]);
  const battery=device.battery?.level;
  const lastSeen=device.stream.last_seen_t_us;
  useEffect(()=>{
    if(!enabled||battery===undefined||lastSeen===null)return;
    setHistory(prev=>{
      const last=prev.at(-1);
      if(last&&lastSeen<last.t)return [{t:lastSeen,v:battery}];
      if(last&&lastSeen-last.t<1_000_000)return prev;
      return [...prev,{t:lastSeen,v:battery}].slice(-300);
    });
  },[enabled,battery,lastSeen]);
  const first=history[0],last=history.at(-1);
  const span=first&&last?Math.max(1,last.t-first.t):1;
  return <section className="lab-card"><h3>Battery history</h3>
    {enabled&&last?<><p className="mono">{(last.v*100).toFixed(0)}% · {(span/1e6).toFixed(0)} s observed</p>
      <svg viewBox="0 0 240 70" className="lab-chart" role="img" aria-label="Battery percentage history"><polyline points={history.map(p=>`${(p.t-(first?.t??p.t))/span*240},${65-p.v*60}`).join(" ")} stroke="var(--accent)" fill="none" strokeWidth="1.5"/></svg></>:<p>Waiting for battery telemetry.</p>}
    <p className="subtle">Up to 300 browser-local reports while this view is open; not a battery discharge benchmark.</p>
  </section>;
}
