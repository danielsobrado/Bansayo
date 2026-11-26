import { describe,expect,it } from "vitest";
import { nearestPoint,sharedEnd,signalCsv,stats,windowPoints } from "./signalMath";
const point=(t_us:number,value:number)=>({t_us,seq:t_us,device_time_us:null,values:[value,0,0]});
describe("synchronized signal instruments",()=>{
  it("uses a single time base and does not invent cursor samples in a gap",()=>{
    const points=[point(1000000,1),point(2000000,2)];
    expect(sharedEnd({gyro:{age_ms:0,points},accel:{age_ms:0,points:[point(2100000,0)]}})).toBe(2100000);
    expect(windowPoints(points,2100000,.5)).toEqual([points[1]]);
    expect(nearestPoint(points,1500000)).toBeUndefined();
    expect(nearestPoint(points,2050000)).toEqual(points[1]);
  });
  it("computes statistics and exports only the selected range",()=>{
    expect(stats([])).toBeNull();
    expect(stats([-1,1])).toEqual({min:-1,max:1,mean:0,rms:1,std:1});
    const csv=signalCsv({gyro:{age_ms:0,points:[point(0,9),point(2000000,2)]}},["gyro"],2000000,1);
    expect(csv).toContain("gyro,rad/s,2000000,2000000,,X,2");
    expect(csv).not.toContain("gyro,rad/s,0,");
  });
});
