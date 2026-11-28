import {describe,it,expect} from "vitest";
import {benchQuality,profileReadiness} from "./calibrationQuality";
import {contiguous,reliabilitySummary,retainRows,isReliabilityRow,type ReliabilityRow} from "./reliabilityMath";
const row:ReliabilityRow={run:"run",at:1000,device:"A",source:"s",origin:"live",state:"tracking",lastSeen:1000,rate:200,received:10,dropped:2,drift:5,offset:100,uncertainty:20,resets:0,settled:true,timeQuality:"estimated"};
describe("reliability evidence",()=>{
  it("does not merge resets, collection gaps or different runs",()=>{
    const next={...row,at:6000,received:20,dropped:3,lastSeen:6000};
    expect(contiguous(row,next)).toBe(true);
    expect(contiguous(row,{...next,resets:1})).toBe(false);
    expect(contiguous(row,{...next,at:100000})).toBe(false);
    expect(contiguous(row,{...next,run:"other"})).toBe(false);
    expect(reliabilitySummary([row,next])).toMatchObject({samples:2,accepted:10,dropped:1,intervals:1});
    expect(isReliabilityRow(row)).toBe(true);expect(isReliabilityRow({...row,received:-1})).toBe(false);
    expect(retainRows([row],8*86400000)).toEqual([]);
  });
  it("requires complete bench evidence and keeps profile readiness separate",()=>{
    expect(benchQuality({}, {},null).score).toBeNull();
    const faces=Object.fromEntries(Array.from({length:6},(_,i)=>[i,{error:.1}]));
    const turns={X:{angle:90,axisError:1},Y:{angle:90,axisError:1},Z:{angle:90,axisError:1}};
    expect(benchQuality(faces,turns,{angle:1,seconds:10}).score).toBeNull();
    expect(benchQuality(faces,turns,{angle:1,seconds:30}).score).toBe(100);
    expect(benchQuality(faces,turns,{angle:3,seconds:30}).score).toBe(90);
    expect(profileReadiness([]).percent).toBeNull();
  });
});
