import {describe,it,expect} from "vitest";
import {transportMetrics,packetGapDelta} from "./networkMath";
describe("socket transport timing",()=>{
  it("breaks packet gap trends on reset and stale intervals",()=>{
    const a={generation:1,received:98,missing:2,rejected_order:0,last_receive_us:1000,intervals_ms:[]};
    const b={...a,received:107,missing:3,last_receive_us:1001000};
    expect(packetGapDelta(a,b)).toBe(.1);
    expect(packetGapDelta(a,{...b,generation:2})).toBeNull();
    expect(packetGapDelta(a,{...b,last_receive_us:4001000})).toBeNull();
  });
  it("keeps datagram gaps distinct from interval residuals",()=>{
    const m=transportMetrics({generation:1,received:98,missing:2,rejected_order:5,last_receive_us:1,intervals_ms:[[5,7],[5,4],[5,5]]});
    expect(m.lossRatio).toBe(.02);expect(m.mean).toBe(1);expect(m.p95).toBe(1);
  });
});
