import {landmarksStore} from "../../../engine/stores";
import {appendOptical} from "./opticalSyncMath";
import type {OpticalRecording,OpticalSelection} from "./opticalSyncConfig";

export function startOpticalCapture(selection:OpticalSelection,signal:AbortSignal):OpticalRecording{
  const record:OpticalRecording={selection:{...selection},frames:[],identity:null,origin:null,invalidReason:null};
  let previous=landmarksStore.get().get(selection.device);
  const unsubscribe=landmarksStore.subscribe(()=>{
    const frame=landmarksStore.get().get(selection.device);
    if(!frame){if(record.frames.length)record.invalidReason="Camera stream cleared during recording.";return;}
    if(frame===previous)return;
    previous=frame;
    appendOptical(record,frame);
  });
  signal.addEventListener("abort",unsubscribe,{once:true});
  return record;
}
