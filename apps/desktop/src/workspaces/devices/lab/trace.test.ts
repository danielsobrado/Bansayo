import { describe, expect, it } from "vitest";
import { freshChannels, motionSummary, parseSnapshot, rotate } from "./trace";

describe("device inspection", () => {
  it("rejects malformed measurements", () => {
    expect(parseSnapshot({channels:{gyro:{age_ms:0,points:[{t_us:1,seq:1,device_time_us:null,values:[1,2]}]}}})).toBeNull();
    expect(parseSnapshot({channels:{}})).toEqual({channels:{}});
    expect(parseSnapshot({reference:4,channels:{}})?.reference).toBe(4);
    expect(parseSnapshot({reference:-1,channels:{}})).toBeNull();
  });
  it("validates full-rate timing metadata", () => {
    expect(parseSnapshot({channels:{},timing:{intervals_ms:[[5,6]],sequence_gaps:2}})?.timing?.sequence_gaps).toBe(2);
    expect(parseSnapshot({channels:{},timing:{intervals_ms:[[5,-6]],sequence_gaps:2}})).toBeNull();
  });
  it("expires samples if the HTTP poll stalls or the device disconnects", () => {
    const channels = {gyro: {age_ms: 30, points: []}};
    expect(freshChannels(channels, 50, true).gyro?.age_ms).toBe(80);
    expect(freshChannels(channels, 600, true)).toEqual({});
    expect(freshChannels(channels, 0, false)).toEqual({});
  });
  it("rotates sensor axes using XYZW", () => {
    const r = rotate([1,0,0],[0,0,Math.SQRT1_2,Math.SQRT1_2]);
    expect(r[0]).toBeCloseTo(0);
    expect(r[1]).toBeCloseTo(1);
    expect(r[2]).toBeCloseTo(0);
  });
  it("requires fresh gyro and acceleration over a sustained window", () => {
    const channel = (values: number[]) => ({age_ms:0,points:Array.from({length:51},(_,i)=>({t_us:i*20_000,seq:i,device_time_us:null,values}))});
    const gyro = channel([0,0,0]), accel = channel([0,9.80665,0]);
    expect(motionSummary(gyro,accel)?.still).toBe(true);
    expect(motionSummary(channel([1,0,0]),accel)?.still).toBe(false);
    expect(motionSummary({...gyro,age_ms:1000},accel)).toBeNull();
    expect(motionSummary({...gyro,points:gyro.points.slice(-2)},accel)).toBeNull();
  });
});
