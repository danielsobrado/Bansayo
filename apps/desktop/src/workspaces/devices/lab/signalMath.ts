import { CHANNELS, type ChannelId, type Point, type Snapshot } from "./trace";
import { CURSOR_TOLERANCE_US } from "./studioConfig";

export function sharedEnd(channels: Snapshot["channels"]): number {
  return Math.max(0,...Object.values(channels).map(c=>c?.points.at(-1)?.t_us??0));
}
export function windowPoints(points: readonly Point[], end: number, seconds:number): Point[] {
  return points.filter(p=>p.t_us>=end-seconds*1_000_000&&p.t_us<=end);
}
export function nearestPoint(points:readonly Point[], target:number,tolerance=CURSOR_TOLERANCE_US): Point | undefined {
  let best:Point|undefined;
  for(const point of points) {
    if(!best||Math.abs(point.t_us-target)<Math.abs(best.t_us-target))best=point;
  }
  return best&&Math.abs(best.t_us-target)<=tolerance?best:undefined;
}
export function stats(values:readonly number[]) {
  if(!values.length)return null;
  const mean=values.reduce((s,v)=>s+v,0)/values.length;
  return {min:Math.min(...values),max:Math.max(...values),mean,rms:Math.sqrt(values.reduce((s,v)=>s+v*v,0)/values.length),std:Math.sqrt(values.reduce((s,v)=>s+(v-mean)**2,0)/values.length)};
}
export function signalCsv(channels:Snapshot["channels"],ids:readonly ChannelId[],end:number,seconds:number) {
  const rows=["channel,unit,session_time_us,sequence,device_time_us,axis,value"];
  for(const id of ids)for(const p of windowPoints(channels[id]?.points??[],end,seconds)) {
    p.values.forEach((value,i)=>rows.push([id,CHANNELS[id].unit,p.t_us,p.seq,p.device_time_us??"",CHANNELS[id].axes[i],value].join(",")));
  }
  return rows.join("\n")+"\n";
}
export function downloadText(filename:string,content:string,type:string) {
  const url=URL.createObjectURL(new Blob([content],{type}));
  const a=document.createElement("a");
  a.href=url;a.download=filename;a.click();
  window.setTimeout(()=>URL.revokeObjectURL(url),1000);
}
