import {completeCapture,type Capture} from "./captureTypes";
import type {Point} from "./trace";
import {GRAVITY,quaternionAngle} from "./benchMath";
import {scalarPoints,signalHealth} from "./syncMath";
import {validLimits,type StationaryLimits} from "./stationaryConfig";

const channels=["gyro","accel","orientation"] as const;
type Channel=typeof channels[number];
export interface Gate {name:string;value:number|null;limit:number;direction:"min"|"max";pass:boolean}
function axisStats(points:readonly Point[],axis:0|1|2){
  const h=signalHealth(scalarPoints(points,axis));
  const t0=points[0]?.t_us??0;
  let mt=0,mv=0,vt=0,cov=0;
  points.forEach((p,i)=>{const t=(p.t_us-t0)/60e6,v=p.values[axis]!,n=i+1,dt=t-mt,dv=v-mv;mt+=dt/n;mv+=dv/n;vt+=dt*(t-mt);cov+=dt*(v-mv);});
  return {mean:h.mean,std:h.std,rms:h.rms,slopePerMinute:vt>0?cov/vt:null};
}
export function stationaryReport(capture:Capture,device:string,limits:StationaryLimits,confirmed:boolean){
  const grouped:Record<Channel,Point[]>={gyro:[],accel:[],orientation:[]},epochs=new Set<number>();
  for(const s of capture.samples){
    if(s.device!==device||!channels.includes(s.channel as Channel))continue;
    if(capture.window&&(s.point.t_us<capture.window.start_us||s.point.t_us>=capture.window.end_us))continue;
    grouped[s.channel as Channel].push(s.point);epochs.add(s.epoch);
  }
  const health=channels.map(channel=>({channel,...signalHealth(scalarPoints(grouped[channel],"magnitude"))}));
  const start=Math.max(...channels.map(c=>grouped[c][0]?.t_us??Infinity));
  const end=Math.min(...channels.map(c=>grouped[c].at(-1)?.t_us??-Infinity));
  const commonSeconds=Number.isFinite(end-start)?Math.max(0,(end-start)/1e6):0;
  const common=(channel:Channel)=>grouped[channel].filter(p=>p.t_us>=start&&p.t_us<=end);
  const gyro=common("gyro"),accel=common("accel"),orientation=common("orientation");
  const finite=channels.every(c=>grouped[c].length>=2&&grouped[c].every(p=>p.values.length===(c==="orientation"?4:3)&&p.values.every(Number.isFinite)&&(c!=="orientation"||Math.hypot(...p.values)>1e-6)));
  const gyroAxes=([0,1,2] as const).map(i=>axisStats(gyro,i)),accelAxes=([0,1,2] as const).map(i=>axisStats(accel,i));
  let gyroPeak=0,gravityError=0,orientationMax=0;
  for(const p of gyro)gyroPeak=Math.max(gyroPeak,Math.hypot(...p.values));
  for(const p of accel)gravityError=Math.max(gravityError,Math.abs(Math.hypot(...p.values)-GRAVITY));
  const first=orientation[0]?.values;
  if(first)for(const p of orientation)orientationMax=Math.max(orientationMax,quaternionAngle(first,p.values)??Infinity);
  const finalAngle=first&&orientation.length>1?quaternionAngle(first,orientation.at(-1)!.values):null;
  const gates:Gate[]=[];
  function gate(name:string,value:number|null,limit:number,direction:"min"|"max"){
    const pass=value!==null&&Number.isFinite(value)&&(direction==="min"?value>=limit:value<=limit);
    gates.push({name,value:value!==null&&Number.isFinite(value)?value:null,limit,direction,pass});
  }
  gate("Common duration (s)",commonSeconds,limits.minSeconds,"min");
  for(const h of health)gate(h.channel+" rate (Hz)",h.rateHz,limits.minRateHz,"min");
  gate("Gyro peak (rad/s)",gyro.length?gyroPeak:null,limits.maxGyroPeak,"max");
  gate("Gravity error (m/s²)",accel.length?gravityError:null,limits.maxGravityError,"max");
  gate("Gyro bias magnitude (rad/s)",gyro.length?Math.hypot(...gyroAxes.map(a=>a.mean??NaN)):null,limits.maxGyroBias,"max");
  gate("Worst gyro axis std dev (rad/s)",gyro.length?Math.max(...gyroAxes.map(a=>a.std??NaN)):null,limits.maxGyroStd,"max");
  gate("Worst accel axis std dev (m/s²)",accel.length?Math.max(...accelAxes.map(a=>a.std??NaN)):null,limits.maxAccelStd,"max");
  gate("Orientation excursion (°)",first?orientationMax:null,limits.maxOrientationDriftDeg,"max");
  const reasons:string[]=[];
  if(!confirmed)reasons.push("Confirm the tracker was rigidly stationary.");
  if(!validLimits(limits))reasons.push("Invalid acceptance limits.");
  if(!completeCapture(capture)||epochs.size!==1)reasons.push("Incomplete or incompatible capture evidence.");
  if(!finite||[gyro,accel,orientation].some(p=>p.length<2)||health.some(h=>h.nonIncreasing||h.longIntervals))reasons.push("Missing, invalid or irregular channel samples.");
  if(commonSeconds<limits.minSeconds)reasons.push("Insufficient common duration.");
  if(gyroPeak>limits.maxGyroPeak||gravityError>limits.maxGravityError)reasons.push("Stationary motion screen failed; noise is not isolated.");
  const status=reasons.length?"inconclusive":gates.every(g=>g.pass)?"pass":"fail";
  return {device,status,reasons,limits,health,gates,gyroAxes,accelAxes,finalAngle,orientationMax:first&&Number.isFinite(orientationMax)?orientationMax:null,commonSeconds};
}
