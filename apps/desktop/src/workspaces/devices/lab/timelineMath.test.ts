import {describe,it,expect} from "vitest";
import {lowerTimeBound} from "./timelineMath";
import type {Sample} from "./captureTypes";
const points:Sample[]=[10,20,20,40].map((t_us,i)=>({device:"A",epoch:1,cursor:i+1,channel:"gyro",point:{t_us,seq:i,device_time_us:null,values:[0,0,0]}}));
describe("timeline range search",()=>{
  it("handles empty and repeated timestamps and range boundaries",()=>{
    expect(lowerTimeBound([],10)).toBe(0);
    expect(lowerTimeBound(points,0)).toBe(0);
    expect(lowerTimeBound(points,20)).toBe(1);
    expect(lowerTimeBound(points,21)).toBe(3);
    expect(lowerTimeBound(points,41)).toBe(4);
  });
});
