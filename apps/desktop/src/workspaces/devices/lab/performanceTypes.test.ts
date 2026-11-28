import {describe,it,expect} from "vitest";
import {parsePerformance,parseEngineReliability} from "./performanceTypes";
describe("engine diagnostic snapshots",()=>{
  it("preserves unavailable values and permits multicore CPU",()=>{
    const sample={t_us:1,cpu_percent_one_core:230,resident_bytes:null};
    const v={enabled:true,timing_enabled:false,epoch:"session",samples:[sample],stages:[]};
    expect(parsePerformance(v)?.samples[0]).toEqual(sample);
    expect(parsePerformance({...v,samples:[sample,sample]})).toBeNull();
    expect(parsePerformance({...v,samples:[{...sample,cpu_percent_one_core:-1}]})).toBeNull();
  });
  it("rejects malformed archive pages",()=>{
    expect(parseEngineReliability({enabled:false,rows:[]})).not.toBeNull();
    expect(parseEngineReliability({enabled:true,rows:[{epoch:"x",at_ms:1,t_us:1,devices:[{}]}]})).toBeNull();
  });
});
