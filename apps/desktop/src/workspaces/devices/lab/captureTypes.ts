import { CHANNEL_IDS,CHANNELS,type ChannelId,type Point } from "./trace";
export const CAPTURE_SECONDS=[10,60,300,600] as const;
export const MAX_ROWS=500_000;
export const MAX_DEVICES=4;
export const BATCH_MS=250;
export interface Sample { device:string; epoch:number; cursor:number; channel:ChannelId; point:Point }
export interface Marker { t_us:number; label:string; device?:string }
export interface Capture { samples:Sample[];markers:Marker[];lost:number;resets:number;started:string;reason:string }
export interface Batch {epoch:number;next:number;lost:number;reset:boolean;entries:Omit<Sample,"device"|"epoch">[]}
export function parseBatch(value:unknown):Batch|null {
  if(!value||typeof value!=="object")return null;
  const b=value as Batch;
  const integer=(v:unknown)=>typeof v==="number"&&Number.isSafeInteger(v)&&v>=0;
  if(!integer(b.epoch)||b.epoch===0||!integer(b.next)||!integer(b.lost)||typeof b.reset!=="boolean"||!Array.isArray(b.entries)||b.entries.length>4096)return null;
  let previous=-1;
  for(const e of b.entries){
    if(!e||!integer(e.cursor)||e.cursor<=previous||e.cursor>b.next||!CHANNEL_IDS.includes(e.channel)||!e.point||!integer(e.point.t_us)||!integer(e.point.seq)||!(e.point.device_time_us===null||integer(e.point.device_time_us))||!Array.isArray(e.point.values)||e.point.values.length!==CHANNELS[e.channel].axes.length||!e.point.values.every(Number.isFinite))return null;
    previous=e.cursor;
  }
  return b;
}
