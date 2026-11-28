export const OPTICAL_SYNC={maxFrames:180_000,minConfidence:.7,minSeparationPx:25,maxFrameGapUs:150_000,maxAngleStepRad:Math.PI/2} as const;
export interface OpticalSelection {device:string;first:number;second:number}
export interface OpticalObservation {t_us:number;seq:number;angle:number|null;quality:"synchronized"|"estimated"|"receive_time"}
export interface OpticalRecording {
  selection:OpticalSelection;frames:OpticalObservation[];identity:string|null;origin:string|null;invalidReason:string|null;
}
