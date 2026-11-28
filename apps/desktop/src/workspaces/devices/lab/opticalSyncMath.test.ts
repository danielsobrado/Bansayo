import {describe,it,expect} from "vitest";
import {opticalAngularSpeed,opticalObservation,appendOptical} from "./opticalSyncMath";
import type {OpticalRecording,OpticalObservation} from "./opticalSyncConfig";
import type {PoseLandmarks2D} from "@contracts";
const selection={device:"cam",first:0,second:1};
const frame=(seq:number,t:number):PoseLandmarks2D=>({frame_meta:{device_id:"cam",frame_seq:seq,t_us:t,device_time_us:null,time_quality:"estimated",image_size_px:[640,480]},frame:{type:"image",id:"cam"},origin:"fixture",schema_id:"fixture",detector:"test",points:[{x_px:100,y_px:100,confidence:1},{x_px:150,y_px:100,confidence:1}]});
describe("camera timing evidence",()=>{
  it("measures image angle and rejects absent or short landmark pairs",()=>{
    const f=frame(1,1000);expect(opticalObservation(f,selection).angle).toBe(0);
    f.points[1]=null;expect(opticalObservation(f,selection).angle).toBeNull();
  });
  it("unwraps crossing pi and timestamps speed at the midpoint",()=>{
    const a:OpticalObservation={seq:1,t_us:0,angle:Math.PI-.05,quality:"estimated"};
    const b:OpticalObservation={seq:2,t_us:50000,angle:-Math.PI+.05,quality:"estimated"};
    const v=opticalAngularSpeed([a,b]);expect(v.error).toBeNull();expect(v.points[0]?.t_us).toBe(25000);expect(v.points[0]?.value).toBeCloseTo(2);
    expect(opticalAngularSpeed([a,{...b,quality:"receive_time"}]).error).not.toBeNull();
    expect(opticalAngularSpeed([a,{...b,t_us:300000}]).error).not.toBeNull();
  });
  it("deduplicates store notifications and rejects a camera reference change",()=>{
    const rec:OpticalRecording={selection,frames:[],identity:null,origin:null,invalidReason:null};
    appendOptical(rec,frame(1,1000));appendOptical(rec,frame(1,1000));expect(rec.frames).toHaveLength(1);
    appendOptical(rec,{...frame(2,2000),schema_id:"other"});expect(rec.invalidReason).not.toBeNull();
  });
});
