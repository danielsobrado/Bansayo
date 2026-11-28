import type {StageTiming} from "@contracts";
export interface ResourceSample {t_us:number;cpu_percent_one_core:number|null;resident_bytes:number|null}
export interface PerformanceSnapshot {enabled:boolean;timing_enabled:boolean;epoch:string;samples:ResourceSample[];stages:StageTiming[]}
const obj=(v:unknown):v is Record<string,unknown>=>typeof v==="object"&&v!==null;
const nn=(v:unknown):v is number=>typeof v==="number"&&Number.isFinite(v)&&v>=0;
const nullable=(v:unknown)=>v===null||nn(v);
export function parsePerformance(v:unknown):PerformanceSnapshot|null{
  if(!obj(v)||typeof v.enabled!=="boolean"||typeof v.timing_enabled!=="boolean"||typeof v.epoch!=="string"||!Array.isArray(v.samples)||v.samples.length>300||!Array.isArray(v.stages)||v.stages.length>16)return null;
  if(!v.samples.every((p,i,a)=>obj(p)&&nn(p.t_us)&&Number.isSafeInteger(p.t_us)&&nullable(p.cpu_percent_one_core)&&nullable(p.resident_bytes)&&(i===0||p.t_us>a[i-1].t_us)))return null;
  if(!v.stages.every(s=>obj(s)&&typeof s.stage==="string"&&["p50_us","p95_us","p99_us","max_us","count","budget_us"].every(k=>nn(s[k]))))return null;
  return v as unknown as PerformanceSnapshot;
}
export interface EngineReliabilityEntry {id:number;epoch:string;at_ms:number;t_us:number;devices:Record<string,unknown>[]}
export interface EngineReliability {enabled:boolean;rows:EngineReliabilityEntry[]}
export function parseEngineReliability(v:unknown):EngineReliability|null{
  if(!obj(v)||typeof v.enabled!=="boolean"||!Array.isArray(v.rows)||v.rows.length>120)return null;
  if(!v.rows.every((r,i,a)=>obj(r)&&nn(r.id)&&Number.isSafeInteger(r.id)&&r.id>0&&(i===0||r.id>a[i-1].id)&&typeof r.epoch==="string"&&nn(r.at_ms)&&r.at_ms<=8.64e15&&nn(r.t_us)&&Array.isArray(r.devices)&&r.devices.every(d=>obj(d)&&typeof d.device==="string"&&typeof d.state==="string"&&typeof d.origin==="string"&&obj(d.stream)&&nn(d.stream.measured_rate_hz)&&nn(d.stream.received)&&nn(d.stream.dropped))))return null;
  return v as unknown as EngineReliability;
}
