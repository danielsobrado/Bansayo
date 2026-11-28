import type {DeviceStatus} from "@contracts";
import type {TransportSnapshot} from "./trace";
export function transportMetrics(s:TransportSnapshot){
  const jitter=s.intervals_ms.map(([device,arrival])=>Math.abs(arrival-device)).sort((a,b)=>a-b);
  const mean=jitter.length?jitter.reduce((sum,v)=>sum+v,0)/jitter.length:null;
  const p95=jitter.length?jitter[Math.ceil(jitter.length*.95)-1]!:null;
  const total=s.received+s.missing;
  return {mean,p95,lossRatio:total?s.missing/total:null};
}
export function transportFresh(s:TransportSnapshot|undefined,device:DeviceStatus,elapsedMs:number){
  const last=device.stream.last_seen_t_us;
  return !!s&&Number.isFinite(elapsedMs)&&elapsedMs<=1500&&last!==null&&Math.abs(last-s.last_receive_us)<=2_000_000&&device.state!=="disconnected"&&device.state!=="unavailable";
}

export function packetGapDelta(a:TransportSnapshot,b:TransportSnapshot):number|null {
  if(a.generation!==b.generation||b.last_receive_us<=a.last_receive_us||b.received<a.received||b.missing<a.missing||b.last_receive_us-a.last_receive_us>3_000_000)return null;
  const received=b.received-a.received,missing=b.missing-a.missing;
  return received+missing?missing/(received+missing):null;
}
