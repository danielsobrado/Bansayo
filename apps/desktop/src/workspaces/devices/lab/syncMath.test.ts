import {describe,it,expect} from "vitest";
import {compareTiming,signalHealth,scalarPoints,type SyncPoint} from "./syncMath";
const signal=(t:number)=>Math.exp(-(((t-1.4)/.05)**2))+.6*Math.exp(-(((t-2.3)/.08)**2))-.8*Math.exp(-(((t-3.1)/.07)**2));
const trace=(delay=0,step=5000):SyncPoint[]=>Array.from({length:Math.floor(4_000_000/step)+1},(_,i)=>({t_us:i*step,value:signal((i*step-delay)/1e6)}));
describe("recorded sensor timing",()=>{
  it("recovers signed feature lag across unequal sample rates",()=>{
    const a=trace(),b=trace(40000,10000);
    const forward=compareTiming(a,b),reverse=compareTiming(b,a);
    expect(forward.status).toBe("estimated");expect(forward.lagMs).toBe(40);expect(forward.stepUs).toBe(10000);
    expect(reverse.lagMs).toBe(-40);expect(forward.correlation).toBeGreaterThan(.99);
  });
  it("refuses missing intervals, duplicate times, and stationary signals",()=>{
    const a=trace();
    expect(compareTiming(a,a.filter((_,i)=>i!==300)).status).toBe("insufficient");
    expect(compareTiming(a,[a[0]!,...a]).status).toBe("insufficient");
    expect(compareTiming(a.map(p=>({...p,value:1})),a).lagMs).toBeNull();
    expect(compareTiming(a,a.map(p=>({...p,t_us:p.t_us+10_000_000}))).status).toBe("insufficient");
  });
  it("does not declare periodic or search-boundary peaks reliable",()=>{
    const periodic=trace().map(p=>({...p,value:Math.sin(2*Math.PI*p.t_us/100000)}));
    expect(compareTiming(periodic,periodic).lagMs).toBeNull();
    expect(compareTiming(trace(),trace(200000)).status).toBe("boundary");
  });
  it("reports observed rate, jitter and gaps independently of lag",()=>{
    const a=trace().filter((_,i)=>i!==300),health=signalHealth(a);
    expect(health.count).toBe(800);expect(health.rateHz).toBeCloseTo(199.75);
    expect(health.medianIntervalMs).toBe(5);expect(health.longIntervals).toBe(1);
    expect(signalHealth([]).rateHz).toBeNull();
    expect(scalarPoints([{t_us:0,seq:1,device_time_us:null,values:[3,4,0]}],"magnitude")[0]?.value).toBe(5);
  });
});
