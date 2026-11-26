import { describe,it,expect } from "vitest";
import { quaternionAngle,faceMeasurement,GRAVITY,nearestPairs } from "./benchMath";

describe("bench measurement",()=>{
  it("uses shortest quaternion arc and treats signs equally",()=>{
    expect(quaternionAngle([0,0,0,1],[0,0,0,-1])).toBeCloseTo(0);
    expect(quaternionAngle([0,0,0,1],[Math.SQRT1_2,0,0,Math.SQRT1_2])).toBeCloseTo(90);
    expect(quaternionAngle([0,0,0,0],[0,0,0,1])).toBeNull();
  });
  it("checks face sign and sufficient duration",()=>{
    const points=Array.from({length:51},(_,i)=>({t_us:i*20000,seq:i,device_time_us:null,values:[GRAVITY,0,0]}));
    expect(faceMeasurement({age_ms:0,points},0)?.pass).toBe(true);
    expect(faceMeasurement({age_ms:0,points},1)?.pass).toBe(false);
    expect(faceMeasurement({age_ms:0,points:points.slice(-2)},0)).toBeNull();
    expect(nearestPairs(points,[points[0]!],10000)).toHaveLength(1);
  });
});
