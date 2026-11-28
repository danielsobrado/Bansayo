import {describe,it,expect} from "vitest";
import {stationaryReport} from "./stationaryMath";
import {STATIONARY_DEFAULTS} from "./stationaryConfig";
import type {Capture,Sample} from "./captureTypes";
function capture():Capture{
  const samples:Sample[]=[];
  for(let i=0;i<=3100;i++)for(const [channel,values] of [["gyro",[.001,0,0]],["accel",[0,9.80665,0]],["orientation",[0,0,0,1]]] as const){
    samples.push({device:"A",epoch:1,cursor:samples.length+1,channel,point:{t_us:i*10000,seq:i,device_time_us:null,values:[...values]}});
  }
  return {samples,markers:[],lost:0,resets:0,started:"fixture",reason:"Duration completed",window:{start_us:0,end_us:31_010_000,excluded:0}};
}
describe("stationary acceptance",()=>{
  it("requires operator confirmation and evaluates thresholds",()=>{
    const c=capture();
    expect(stationaryReport(c,"A",STATIONARY_DEFAULTS,false).status).toBe("inconclusive");
    expect(stationaryReport(c,"A",STATIONARY_DEFAULTS,true).status).toBe("pass");
    expect(stationaryReport(c,"A",{...STATIONARY_DEFAULTS,maxGyroBias:.0001},true).status).toBe("fail");
  });
  it("rejects motion, missing samples and lost data",()=>{
    const c=capture();
    c.samples[0]!.point.values[0]=1;
    expect(stationaryReport(c,"A",STATIONARY_DEFAULTS,true).status).toBe("inconclusive");
    const gap=capture();gap.samples.splice(300,1);
    expect(stationaryReport(gap,"A",STATIONARY_DEFAULTS,true).status).toBe("inconclusive");
    const lost=capture();lost.lost=1;
    expect(stationaryReport(lost,"A",STATIONARY_DEFAULTS,true).status).toBe("inconclusive");
  });
});
