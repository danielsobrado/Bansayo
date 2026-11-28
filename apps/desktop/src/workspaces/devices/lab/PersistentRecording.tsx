import {useEffect,useState} from "react";
import type {DeviceStatus} from "@contracts";
import {request} from "../../../engine/http";

interface Status {running:boolean;file:string|null;observations:number;lost:number;bytes:number;reason:string;window_start_us?:number|null;window_end_us?:number|null}
const DURATIONS=[60,300,600,1800,3600];
const PATH="/inspection/recording";
export function PersistentRecording({devices,live,active}:{devices:DeviceStatus[];live:boolean;active:boolean}){
  const [status,setStatus]=useState<Status|null>(null);
  const [ids,setIds]=useState<string[]>([]);
  const [duration,setDuration]=useState(600);
  const [error,setError]=useState("");
  const [commandError,setCommandError]=useState("");
  const [pending,setPending]=useState(false);
  useEffect(()=>{
    if(pending||!active)return;
    const controller=new AbortController();
    let disposed=false,timer=0;
    async function poll(){
      const res=await request<Status>("GET",PATH,undefined,(url,init)=>fetch(url,{...init,cache:"no-store",signal:AbortSignal.any([controller.signal,AbortSignal.timeout(3000)])}));
      if(disposed)return;
      if(res.ok){setStatus(res.data);setError("");}else setError(res.error.message);
      timer=window.setTimeout(()=>void poll(),1000);
    }
    void poll();
    return()=>{disposed=true;controller.abort();clearTimeout(timer);};
  },[pending,active]);
  async function command(stop:boolean){
    setPending(true);
    setCommandError("");
    const res=await request<Status>("POST",stop?PATH+"/stop":PATH,stop?undefined:{device_ids:selected,duration_s:duration},(url,init)=>fetch(url,{...init,signal:AbortSignal.timeout(5000)}));
    if(res.ok){setStatus(res.data);setError("");}else setCommandError(res.error.message);
    setPending(false);
  }
  const eligible=devices.filter(d=>d.descriptor.kind!=="camera");
  const selected=ids.filter(id=>eligible.some(d=>d.descriptor.id===id));
  return <section className="lab-card"><header><h3>Persistent engine recording</h3><span className={status?.running?"lab-result-warn":"subtle"}>{status?.reason||"Idle"}</span></header>
    <p className="subtle">Continues after the browser closes. Files are stored under the engine data directory. One recording at a time, up to 4 devices, 1 hour or 256 MiB.</p>
    <div className="lab-bench-controls">{eligible.map(d=><label key={d.descriptor.id}><input type="checkbox" aria-label={"Persistent "+d.descriptor.id} checked={selected.includes(d.descriptor.id)} disabled={!!status?.running||pending||(!selected.includes(d.descriptor.id)&&selected.length>=4)} onChange={e=>setIds(v=>e.target.checked?[...v,d.descriptor.id]:v.filter(id=>id!==d.descriptor.id))}/>{d.descriptor.id}</label>)}</div>
    <div className="studio-controls"><label>Duration <select value={duration} disabled={!!status?.running||pending} onChange={e=>setDuration(Number(e.target.value))}>{DURATIONS.map(v=><option key={v} value={v}>{v/60} min</option>)}</select></label>
    <button disabled={!live||!status||!!status.running||pending||!selected.length} onClick={()=>void command(false)}>Start engine recording</button>
    <button disabled={!status?.running||pending} onClick={()=>void command(true)}>Stop engine recording</button></div>
    {status?.file&&<p className="mono" style={{overflowWrap:"anywhere"}}>{status.file}</p>}
    {status?.window_start_us!=null&&status.window_end_us!=null&&<p className="mono">Common engine-clock window: {((status.window_end_us-status.window_start_us)/1e6).toFixed(3)} s {status.running?"(planned / stopping)":""}</p>}
    {status&&<p className="mono">{status.observations.toLocaleString()} observations · {status.lost} journal losses · {(status.bytes/1048576).toFixed(2)} MiB</p>}
    {(commandError||error)&&<p className="lab-result-warn" role="status">{commandError||error}</p>}
  </section>;
}
