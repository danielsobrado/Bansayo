import type {OpticalRecording} from "./opticalSyncConfig";
import { CHANNEL_IDS,CHANNELS,type ChannelId,type Point } from "./trace";
export const CAPTURE_SECONDS=[10,60,300,600] as const;
export const MAX_ROWS=500_000;
export const MAX_DEVICES=4;
export const BATCH_MS=250;
export const CAPTURE_DRAIN_MS=250;
export interface EngineClock {session_epoch_utc:string;t_us:number}
export function parseClock(value:unknown):EngineClock|null {
  if(!value||typeof value!=="object")return null;
  const v=value as EngineClock;
  return typeof v.session_epoch_utc==="string"&&v.session_epoch_utc.length>0&&Number.isSafeInteger(v.t_us)&&v.t_us>=0?v:null;
}
export interface Sample { device:string; epoch:number; cursor:number; channel:ChannelId; point:Point }
export interface Marker { t_us:number; label:string; device?:string }
export interface Capture { optical?:OpticalRecording; samples:Sample[];markers:Marker[];lost:number;resets:number;started:string;reason:string;window?:{start_us:number;end_us:number;excluded:number} }
export interface Batch {epoch:number;next:number;lost:number;reset:boolean;entries:Omit<Sample,"device"|"epoch">[]}
export function parseBatch(value:unknown):Batch|null {
  if(!value||typeof value!=="object")return null;
  const b=value as Batch;
  const integer=(v:unknown)=>typeof v==="number"&&Number.isSafeInteger(v)&&v>=0;
  if(!integer(b.epoch)||b.epoch===0||!integer(b.next)||!integer(b.lost)||typeof b.reset!=="boolean"||!Array.isArray(b.entries)||b.entries.length>4096)return null;
  let previous=-1;
  for(const e of b.entries){
    if(!e||!integer(e.cursor)||e.cursor===0||(previous>=0&&e.cursor!==previous+1)||e.cursor>b.next||!CHANNEL_IDS.includes(e.channel)||!e.point||!integer(e.point.t_us)||!integer(e.point.seq)||!(e.point.device_time_us===null||integer(e.point.device_time_us))||!Array.isArray(e.point.values)||e.point.values.length!==CHANNELS[e.channel].axes.length||!e.point.values.every(Number.isFinite))return null;
    previous=e.cursor;
  }
  if(b.entries.length&&previous!==b.next)return null;
  return b;
}

export interface JournalCursor {epoch:number;next:number}
/** Server batches contain contiguous cursors; only an explicitly reported overrun can skip entries. */
export function continuesBatch(previous:JournalCursor,batch:Batch):boolean {
  if(batch.reset||batch.epoch!==previous.epoch)return false;
  const expected=previous.next+batch.lost+batch.entries.length;
  return Number.isSafeInteger(expected)&&batch.next===expected&&
    (batch.entries.length===0||batch.entries[0]!.cursor===previous.next+batch.lost+1);
}
