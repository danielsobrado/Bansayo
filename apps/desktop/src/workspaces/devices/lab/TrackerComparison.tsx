import { useState } from "react";
import type { DeviceStatus } from "@contracts";
import { useInspection } from "./useInspection";
import { freshChannels } from "./trace";
import { nearestPairs,quaternionAngle } from "./benchMath";
import { OrientationView } from "./Plots";
import { CaptureHistory } from "./CaptureHistory";

export function TrackerComparison({devices,live}:{devices:DeviceStatus[];live:boolean}) {
  const trackers=devices.filter(d=>d.descriptor.kind==="imu_tracker");
  const [first,setFirst]=useState("");
  const [second,setSecond]=useState("");
  const a=trackers.find(d=>d.descriptor.id===first)??trackers[0];
  const b=trackers.find(d=>d.descriptor.id===second&&d.descriptor.id!==a?.descriptor.id)??trackers.find(d=>d.descriptor.id!==a?.descriptor.id);
  if(!a||!b)return <p>Configure at least two trackers to compare them.</p>;
  return <div className="lab-main">
    <section className="lab-card"><h2>Synchronized tracker comparison</h2>
      <div className="lab-bench-controls">
        <label>Reference <select value={a.descriptor.id} onChange={e=>setFirst(e.target.value)}>{trackers.map(d=><option key={d.descriptor.id}>{d.descriptor.id}</option>)}</select></label>
        <label>Test tracker <select value={b.descriptor.id} onChange={e=>setSecond(e.target.value)}>{trackers.filter(d=>d!==a).map(d=><option key={d.descriptor.id}>{d.descriptor.id}</option>)}</select></label>
      </div><p>Mount both on the same rigid object. Pair by mapped session time (≤40 ms); compare rotation magnitude from the first pair. Independent heading references are not assumed aligned.</p>
    </section>
    <Pair key={a.descriptor.id+":"+b.descriptor.id+":"+live} a={a} b={b} live={live}/>
  </div>;
}
function Pair({a,b,live}:{a:DeviceStatus;b:DeviceStatus;live:boolean}) {
  const ra=useInspection(a.descriptor.id,live,false),rb=useInspection(b.descriptor.id,live,false);
  const channels=(r:typeof ra,d:DeviceStatus)=>freshChannels(r.snapshot?.channels??{},performance.now()-r.receivedAt,live&&d.state!=="disconnected"&&d.state!=="unavailable");
  const ca=channels(ra,a),cb=channels(rb,b);
  const pairs=nearestPairs(ca.orientation?.points??[],cb.orientation?.points??[]);
  const start=pairs[0];
  const series=start?pairs.flatMap(([p,q])=>{
    const av=quaternionAngle(start[0].values,p.values),bv=quaternionAngle(start[1].values,q.values);
    return av===null||bv===null?[]:[{t:p.t_us,delta:Math.abs(av-bv),skew:Math.abs(p.t_us-q.t_us)/1000}];
  }):[];
  const last=series.at(-1), min=series[0]?.t??0, span=Math.max(1,(last?.t??min)-min),max=Math.max(1,...series.map(p=>p.delta));
  return <>
    <div className="lab-grid"><div><h3>{a.descriptor.id}</h3><OrientationView q={ca.orientation?.points.at(-1)?.values}/></div><div><h3>{b.descriptor.id}</h3><OrientationView q={cb.orientation?.points.at(-1)?.values}/></div></div>
    <section className="lab-card"><h3>Relative rotation magnitude disagreement</h3>
      {(ra.error||rb.error)&&<p role="alert" className="error">{ra.error||rb.error}</p>}
      {last?<><p className="mono">{last.delta.toFixed(2)}° · skew {last.skew.toFixed(1)} ms · {series.length} paired samples</p>
        <svg viewBox="0 0 320 100" className="lab-chart" role="img" aria-label="Synchronized magnitude disagreement"><polyline points={series.map(p=>`${(p.t-min)/span*320},${95-p.delta/max*85}`).join(" ")} fill="none" stroke="var(--accent)" strokeWidth="1.5"/></svg>
        <p className="subtle">Plot scale 0–{max.toFixed(1)}° · {(span/1e6).toFixed(2)} s. This does not measure axis alignment or absolute orientation accuracy.</p></>:<p>Waiting for overlapping fresh orientation measurements.</p>}
    </section>
    <CaptureHistory label={a.descriptor.id+" vs "+b.descriptor.id} enabled={live} point={last?{t_us:last.t,value:last.delta,skew_ms:last.skew}:null}/>
  </>;
}
