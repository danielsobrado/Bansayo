import type {Point} from "./trace";
import {SYNC_ANALYSIS as C} from "./syncConfig";

export type SyncAxis=0|1|2|"magnitude";
export interface SyncPoint {t_us:number;value:number}
export interface SignalHealth {count:number;durationSeconds:number;rateHz:number|null;medianIntervalMs:number|null;p95IntervalMs:number|null;longIntervals:number;nonIncreasing:number;mean:number|null;std:number|null;rms:number|null}
export interface SyncResult {
  status:"estimated"|"ambiguous"|"weak"|"boundary"|"insufficient";
  reason:string;stepUs:number|null;pairs:number;lagMs:number|null;candidateLagMs:number|null;
  correlation:number|null;zeroCorrelation:number|null;alternativeCorrelation:number|null;
  curve:{lagMs:number;correlation:number}[];
  overlay:{t_us:number;a:number;b:number}[];
}
export function scalarPoints(points:readonly Point[],axis:SyncAxis):SyncPoint[]{
  return points.map(p=>({t_us:p.t_us,value:axis==="magnitude"?Math.hypot(...p.values):p.values[axis]??NaN}));
}
function quantile(sorted:readonly number[],p:number):number|null{
  return sorted.length?sorted[Math.floor((sorted.length-1)*p)]!:null;
}
function intervals(points:readonly SyncPoint[]){
  const positive:number[]=[];let nonIncreasing=0;
  for(let i=1;i<points.length;i++){
    const dt=points[i]!.t_us-points[i-1]!.t_us;
    if(dt>0&&Number.isFinite(dt))positive.push(dt);else nonIncreasing++;
  }
  positive.sort((a,b)=>a-b);
  return {positive,nonIncreasing,median:quantile(positive,.5)};
}
export function signalHealth(points:readonly SyncPoint[]):SignalHealth {
  const {positive,nonIncreasing,median}=intervals(points);
  const durationSeconds=points.length>1?Math.max(0,(points.at(-1)!.t_us-points[0]!.t_us)/1e6):0;
  let mean=0,m2=0,energy=0,count=0;
  for(const p of points){if(!Number.isFinite(p.value))continue;count++;const delta=p.value-mean;mean+=delta/count;m2+=delta*(p.value-mean);energy+=p.value*p.value;}
  const p95=quantile(positive,.95);
  return {count:points.length,durationSeconds,rateHz:durationSeconds>0&&nonIncreasing===0?(points.length-1)/durationSeconds:null,medianIntervalMs:median===null?null:median/1000,p95IntervalMs:p95===null?null:p95/1000,longIntervals:median===null?0:positive.filter(dt=>dt>median*C.maxGapRatio).length,nonIncreasing,mean:count?mean:null,std:count?Math.sqrt(Math.max(0,m2/count)):null,rms:count?Math.sqrt(energy/count):null};
}
function unavailable(reason:string):SyncResult{
  return {status:"insufficient",reason,stepUs:null,pairs:0,lagMs:null,candidateLagMs:null,correlation:null,zeroCorrelation:null,alternativeCorrelation:null,curve:[],overlay:[]};
}
function interpolate(points:readonly SyncPoint[],start:number,step:number,count:number,maxGap:number):number[]|null{
  let j=0;const out:number[]=[];
  for(let i=0;i<count;i++){
    const t=start+i*step;
    while(j+1<points.length&&points[j+1]!.t_us<t)j++;
    const a=points[j]!,b=points[j+1];
    if(a.t_us===t){out.push(a.value);continue;}
    if(b?.t_us===t){out.push(b.value);continue;}
    if(!b||t<a.t_us||t>b.t_us||b.t_us-a.t_us>maxGap)return null;
    out.push(a.value+(b.value-a.value)*(t-a.t_us)/(b.t_us-a.t_us));
  }
  return out.every(Number.isFinite)?out:null;
}
function pearson(a:readonly number[],b:readonly number[],offset:number):number|null{
  let ma=0,mb=0,va=0,vb=0,cov=0;
  for(let i=0;i<a.length;i++){
    const x=a[i]!,y=b[i+offset]!,n=i+1,dx=x-ma,dy=y-mb;
    ma+=dx/n;mb+=dy/n;va+=dx*(x-ma);vb+=dy*(y-mb);cov+=dx*(y-mb);
  }
  if(Math.sqrt(va/a.length)<C.minDeviation||Math.sqrt(vb/a.length)<C.minDeviation)return null;
  const result=cov/Math.sqrt(va*vb);
  return Number.isFinite(result)?Math.max(-1,Math.min(1,result)):null;
}
/** Positive lag means B contains the same feature later than A in mapped session time. */
export function compareTiming(a:readonly SyncPoint[],b:readonly SyncPoint[]):SyncResult {
  if(a.length<2||b.length<2)return unavailable("Both devices need recorded samples.");
  for(const points of [a,b]){
    if(points.some((p,i)=>!Number.isFinite(p.value)||!Number.isSafeInteger(p.t_us)||p.t_us<0||(i>0&&p.t_us<=points[i-1]!.t_us)))return unavailable("Non-finite or non-increasing samples; timing comparison refused.");
  }
  const end=Math.min(a.at(-1)!.t_us,b.at(-1)!.t_us);
  const start=Math.max(a[0]!.t_us,b[0]!.t_us,end-C.maxWindowUs);
  if(end<=start)return unavailable("No common mapped-time interval.");
  const recent=(p:readonly SyncPoint[])=>p.filter(v=>v.t_us>=start&&v.t_us<=end);
  const ra=recent(a),rb=recent(b);
  const da=intervals(ra).median,db=intervals(rb).median;
  if(!da||!db)return unavailable("Not enough shared sampling intervals.");
  const step=Math.max(C.minGridUs,Math.ceil(da),Math.ceil(db));
  const maxLag=Math.floor(C.maxLagUs/step);
  const count=Math.floor((end-start)/step)+1;
  if(maxLag<1||count-2*maxLag<C.minPairs)return unavailable("Common interval too short for the lag search.");
  // Reject any gap inside the sampled interval, even if the coarse grid happens to skip it.
  const hasGap=(points:readonly SyncPoint[],median:number)=>points.some((v,i)=>i>0&&v.t_us-points[i-1]!.t_us>median*C.maxGapRatio);
  if(hasGap(ra,da)||hasGap(rb,db))return unavailable("Gap or irregular interval inside the comparison window.");
  const av=interpolate(a,start,step,count,da*C.maxGapRatio),bv=interpolate(b,start,step,count,db*C.maxGapRatio);
  if(!av||!bv)return unavailable("Cannot align samples without bridging a gap.");
  const core=av.slice(maxLag,count-maxLag);
  const curve:SyncResult["curve"]=[];
  for(let k=-maxLag;k<=maxLag;k++){
    const correlation=pearson(core,bv,maxLag+k);
    if(correlation!==null)curve.push({lagMs:k*step/1000,correlation});
  }
  if(!curve.length)return unavailable("Insufficient signal variation; move both sensors together.");
  const best=curve.reduce((p,q)=>q.correlation>p.correlation?q:p);
  const alt=curve.filter(p=>Math.abs(p.lagMs-best.lagMs)>=Math.max(C.competingPeakUs/1000,2*step/1000)).reduce<number|null>((v,p)=>v===null||p.correlation>v?p.correlation:v,null);
  const zero=curve.find(p=>p.lagMs===0)?.correlation??null;
  const status=best.correlation<C.minCorrelation?"weak":Math.abs(best.lagMs)>=maxLag*step/1000?"boundary":alt!==null&&best.correlation-alt<C.ambiguityMargin?"ambiguous":"estimated";
  const reasons={weak:"Weak correlation; no reliable lag estimate.",boundary:"Peak at the search boundary; lag may exceed the search range.",ambiguous:"Competing lag candidates; repeat with distinctive non-periodic movement.",estimated:"Advisory feature lag, not a hardware clock or latency measurement."};
  return {status,reason:reasons[status],stepUs:step,pairs:core.length,lagMs:status==="estimated"?best.lagMs:null,candidateLagMs:best.lagMs,correlation:best.correlation,zeroCorrelation:zero,alternativeCorrelation:alt,curve,overlay:av.map((v,i)=>({t_us:start+i*step,a:v,b:bv[i]!}))};
}
