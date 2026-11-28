import type {PoseLandmarks2D} from "@contracts";
import type {Capture} from "./captureTypes";
import {OPTICAL_SYNC as C,type OpticalRecording,type OpticalSelection,type OpticalObservation} from "./opticalSyncConfig";
import {compareTiming,scalarPoints,type SyncPoint} from "./syncMath";

export function opticalObservation(frame:PoseLandmarks2D,selection:OpticalSelection):OpticalObservation{
  const a=frame.points[selection.first],b=frame.points[selection.second],m=frame.frame_meta;
  const dx=(b?.x_px??NaN)-(a?.x_px??NaN),dy=(b?.y_px??NaN)-(a?.y_px??NaN);
  const valid=!!a&&!!b&&Number.isFinite(dx)&&Number.isFinite(dy)&&a.confidence>=C.minConfidence&&b.confidence>=C.minConfidence&&Math.hypot(dx,dy)>=C.minSeparationPx;
  return {t_us:m.t_us,seq:m.frame_seq,angle:valid?Math.atan2(dy,dx):null,quality:m.time_quality};
}
export function appendOptical(record:OpticalRecording,frame:PoseLandmarks2D){
  if(record.invalidReason||frame.frame_meta.device_id!==record.selection.device)return;
  const m=frame.frame_meta,identity=JSON.stringify([frame.schema_id,frame.detector,m.image_size_px,frame.origin]);
  const prev=record.frames.at(-1);
  if(prev&&m.frame_seq===prev.seq&&m.t_us===prev.t_us)return;
  if(record.identity!==null&&record.identity!==identity){record.invalidReason="Camera schema, resolution, detector or origin changed.";return;}
  if(!Number.isSafeInteger(m.t_us)||m.t_us<0||!Number.isSafeInteger(m.frame_seq)||m.frame_seq<0||(prev&&(m.frame_seq<=prev.seq||m.t_us<=prev.t_us))){record.invalidReason="Camera frame time or sequence reset.";return;}
  if(record.frames.length>=C.maxFrames){record.invalidReason="Optical capture capacity reached.";return;}
  record.identity=identity;record.origin=frame.origin;
  record.frames.push(opticalObservation(frame,record.selection));
}
export function opticalAngularSpeed(frames:readonly OpticalObservation[]):{points:SyncPoint[];error:string|null}{
  const points:SyncPoint[]=[];
  for(let i=1;i<frames.length;i++){
    const a=frames[i-1]!,b=frames[i]!,dt=b.t_us-a.t_us;
    if(a.angle===null||b.angle===null||!Number.isFinite(a.angle)||!Number.isFinite(b.angle))return {points:[],error:"Low confidence or missing rigid landmark pair."};
    if(a.quality==="receive_time"||b.quality==="receive_time")return {points:[],error:"Camera uses receive-time timestamps; capture-time comparison unavailable."};
    if(dt<=0||dt>C.maxFrameGapUs)return {points:[],error:"Camera frame timing gap."};
    const delta=Math.atan2(Math.sin(b.angle-a.angle),Math.cos(b.angle-a.angle));
    if(Math.abs(delta)>C.maxAngleStepRad)return {points:[],error:"Rotation between frames too large for an unambiguous projected angle."};
    points.push({t_us:Math.floor((a.t_us+b.t_us)/2),value:Math.abs(delta)*1e6/dt});
  }
  return {points,error:null};
}
export function opticalTiming(capture:Capture,device:string){
  const optical=capture.optical,window=capture.window;
  if(!optical||!window||optical.invalidReason||capture.lost||capture.resets||!/^(Stopped by operator|Duration completed)$/.test(capture.reason))return {error:optical?.invalidReason??"Complete common-window capture required.",result:null};
  const frames=optical.frames.filter(p=>p.t_us>=window.start_us&&p.t_us<window.end_us);
  // Compare only the latest analysis window; earlier camera detection failures do not enter it.
  const end=frames.at(-1)?.t_us??0;
  const selected=frames.filter(p=>p.t_us>=end-8_200_000);
  const {points,error}=opticalAngularSpeed(selected);
  if(error)return {error,result:null};
  const gyro=capture.samples.filter(s=>s.device===device&&s.channel==="gyro"&&s.point.t_us>=window.start_us&&s.point.t_us<window.end_us);
  if(new Set(gyro.map(s=>s.epoch)).size!==1)return {error:"Missing gyro or changed reference.",result:null};
  return {error:null,result:compareTiming(scalarPoints(gyro.map(s=>s.point),"magnitude"),points)};
}
