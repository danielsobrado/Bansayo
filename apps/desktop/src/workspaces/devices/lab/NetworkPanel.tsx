import {useEffect,useState} from "react";
import type {TransportSnapshot} from "./trace";
import {transportMetrics,packetGapDelta} from "./networkMath";
const fmt=(v:number|null)=>v===null?"—":v.toFixed(2);
export function NetworkPanel({snapshot}:{snapshot?:TransportSnapshot}){
  const [history,setHistory]=useState<{last:TransportSnapshot|null;values:number[]}>({last:null,values:[]});
  useEffect(()=>{
    setHistory(old=>{
      if(!snapshot)return old.last===null?old:{last:null,values:[]};
      if(old.last?.generation===snapshot.generation&&old.last.last_receive_us===snapshot.last_receive_us)return old;
      const delta=old.last?packetGapDelta(old.last,snapshot):null;
      return {last:snapshot,values:delta===null?[]:[...old.values,delta].slice(-120)};
    });
  },[snapshot]);
  const metrics=snapshot?transportMetrics(snapshot):null;
  const points=snapshot?.intervals_ms??[];
  const residual=points.map(([device,arrival])=>arrival-device);
  const limit=Math.max(1,...residual.map(Math.abs));
  return <section className="lab-card"><h3>UDP transport / packet loss</h3>
    {snapshot&&metrics?<><p className="mono">Received {snapshot.received} · missing sequence slots {snapshot.missing} · rejected order/reference {snapshot.rejected_order}</p>
      <p className="mono">Sequence-gap ratio {metrics.lossRatio===null?"—":fmt(metrics.lossRatio*100)+"%"} · mean |jitter| {fmt(metrics.mean)} ms · p95 |jitter| {fmt(metrics.p95)} ms</p>
      {residual.length>0&&<svg viewBox="0 0 320 110" className="lab-chart" role="img" aria-label="UDP inter-arrival jitter"><path d="M0 55H320" stroke="var(--line)"/><polyline fill="none" stroke="var(--accent)" strokeWidth="1.5" points={residual.map((v,i)=>`${i/Math.max(1,residual.length-1)*320},${55-v/limit*50}`).join(" ")}/></svg>}
      {history.values.length>0&&<><h4>Recent packet-gap ratio</h4><svg viewBox="0 0 320 100" className="lab-chart" role="img" aria-label="UDP packet gap ratio history"><path d="M0 95H320" stroke="var(--line)"/>{history.values.map((v,i)=><rect key={i} x={i*320/history.values.length} y={95-v*85} width={Math.max(1,320/history.values.length-1)} height={v*85} fill="var(--accent)"/>)}</svg><small className="subtle">0–100% · consecutive receive-counter intervals · latest {history.values.length}</small></>}
      <small className="mono subtle">±{limit.toFixed(2)} ms · {residual.length} consecutive accepted packet intervals</small></>:<p>No fresh native UDP timing evidence. Other transports may not expose packet-level counters.</p>}
    <p className="subtle">Socket receive interval minus device sample interval. Includes OS scheduling, buffering and sender timing; not isolated Wi-Fi latency. Sequence gaps are inferred, not proven network loss. Rejected late, duplicate and foreign-reference packets are separate. Counters reset on reference change. Engine observation drops are separate.</p>
  </section>;
}
