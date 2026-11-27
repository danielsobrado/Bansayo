import {describe,it,expect} from "vitest";
import {spectrum,allan,uniformRun,spectralAnalysis} from "./analysisMath";
import {fitMagnetic} from "./magneticFit";
import {parseBatch} from "./captureTypes";
describe("advanced instruments",()=>{
  it("identifies a known sinusoid and constant Allan deviation",()=>{
    const values=Array.from({length:1024},(_,i)=>2*Math.sin(2*Math.PI*16*i/128));
    const result=spectrum({values,hz:128});const peak=result.reduce((a,b)=>a.y>b.y?a:b);
    expect(peak.x).toBe(16);expect(peak.y).toBeCloseTo(2,2);
    const psd=spectralAnalysis({values,hz:128});
    expect(psd.resolutionHz).toBe(.125);
    expect(psd.density.reduce((sum,p)=>sum+p.y*psd.resolutionHz,0)).toBeCloseTo(2,4);
    expect(psd.density.reduce((a,b)=>a.y>b.y?a:b).x).toBe(16);
    expect(allan({values:Array(1024).fill(2),hz:128}).every(p=>p.y===0)).toBe(true);
    expect(uniformRun([],0)).toBeNull();
  });
  it("recovers a shifted anisotropic sphere and rejects planar measurements",()=>{
    const points=Array.from({length:800},(_,i)=>{const z=1-2*(i+.5)/800,r=Math.sqrt(1-z*z),phi=i*2.3999632297;return [10+3*r*Math.cos(phi)+r*Math.sin(phi),-4+2*r*Math.sin(phi),7+5*z];});
    const fit=fitMagnetic(points);expect(fit).not.toBeNull();expect(fit?.center[0]).toBeCloseTo(10,4);expect(fit?.rms).toBeLessThan(1e-6);expect(fit?.validationRms).toBeLessThan(1e-6);
    expect(fit?.validationCount).toBe(160);
    const corrupted=points.map((p,i)=>i%5===0?p.map(x=>x*10):p);
    expect(fitMagnetic(corrupted)).toBeNull();
    expect(fitMagnetic(points.map(p=>[p[0]!,p[1]!,0]))).toBeNull();
  });
  it("rejects invalid incremental responses",()=>{
    expect(parseBatch({epoch:1,next:0,lost:0,reset:false,entries:[]})).not.toBeNull();
    expect(parseBatch({epoch:0,next:0,lost:0,reset:false,entries:[]})).toBeNull();
  });
});
