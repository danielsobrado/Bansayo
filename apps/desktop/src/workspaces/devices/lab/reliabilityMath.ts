import type {DeviceStatus} from "@contracts";
import {RELIABILITY as C} from "./reliabilityConfig";
export interface ReliabilityRow {
  run:string;at:number;device:string;source:string;origin:string;state:string;lastSeen:number|null;
  rate:number;received:number;dropped:number;drift:number|null;offset:number|null;uncertainty:number|null;resets:number|null;settled:boolean;timeQuality:string;
}
const states=["tracking","degraded","acquiring","disconnected","unavailable"];
const origins=["live","simulated","fixture"];
const qualities=["synchronized","estimated","receive_time"];
const finite=(v:unknown):v is number=>typeof v==="number"&&Number.isFinite(v);
const count=(v:unknown):v is number=>finite(v)&&Number.isSafeInteger(v)&&v>=0;
const optional=(v:unknown)=>v===null||finite(v);
export function isReliabilityRow(v:unknown):v is ReliabilityRow {
  if(!v||typeof v!=="object")return false;
  const r=v as ReliabilityRow;
  return [r.run,r.device,r.source].every(v=>typeof v==="string"&&v.length>0&&v.length<=256)&&count(r.at)&&states.includes(r.state)&&origins.includes(r.origin)&&
    (r.lastSeen===null||count(r.lastSeen))&&finite(r.rate)&&r.rate>=0&&count(r.received)&&count(r.dropped)&&optional(r.drift)&&optional(r.offset)&&
    (r.uncertainty===null||finite(r.uncertainty)&&r.uncertainty>=0)&&(r.resets===null||count(r.resets))&&typeof r.settled==="boolean"&&qualities.includes(r.timeQuality);
}
export function historyRow(device:DeviceStatus,run:string,at:number):ReliabilityRow{
  const t=device.time_sync,active=device.state==="tracking"||device.state==="degraded"||device.state==="acquiring";
  return {run,at,device:device.descriptor.id,source:device.descriptor.source_id,origin:device.descriptor.origin,state:device.state,lastSeen:device.stream.last_seen_t_us,
    rate:device.stream.measured_rate_hz,received:device.stream.received,dropped:device.stream.dropped,drift:active&&t?.settled?t.drift_ppm:null,offset:active&&t?.settled?t.offset_us:null,uncertainty:active&&t?.settled?t.uncertainty_us:null,resets:t?.resets??null,settled:active&&t?.settled===true,timeQuality:t?.time_quality??device.stream.time_quality};
}
export function retainRows(rows:readonly ReliabilityRow[],now:number){
  return rows.filter(r=>r.at>=now-C.maxAgeMs&&r.at<=now).slice(-C.maxRows);
}
export function contiguous(a:ReliabilityRow,b:ReliabilityRow){
  return a.run===b.run&&a.device===b.device&&a.source===b.source&&a.origin===b.origin&&b.at>a.at&&b.at-a.at<=C.maxGapMs&&a.resets===b.resets&&a.received<=b.received&&a.dropped<=b.dropped&&a.lastSeen!==null&&b.lastSeen!==null&&a.lastSeen<=b.lastSeen;
}
export function reliabilitySummary(rows:readonly ReliabilityRow[]){
  let accepted=0,dropped=0,intervals=0;
  for(let i=1;i<rows.length;i++){
    const a=rows[i-1]!,b=rows[i]!;
    if(!contiguous(a,b))continue;
    accepted+=b.received-a.received;dropped+=b.dropped-a.dropped;intervals++;
  }
  const tracked=rows.filter(r=>r.state==="tracking").length;
  return {samples:rows.length,tracked,trackingFraction:rows.length?tracked/rows.length:null,accepted,dropped,intervals};
}
