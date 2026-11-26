import { useEffect,useRef,useState } from "react";
import type { DeviceStatus } from "@contracts";
import { downloadText } from "./signalMath";

const MAX_EVENTS=300;
interface Entry {id:number; observed_at:string; device:string; property:string; value:string}
function stateFields(device:DeviceStatus):Record<string,string> {
  return {state:device.state,assignment:device.assignment??"Unassigned",
    mounting:device.calibration.find(c=>c.kind==="mounting")?.state??"Not reported",
    clock_resets:String(device.time_sync?.resets??"—"),
    clock_settled:device.time_sync?String(device.time_sync.settled):"—",
  };
}
export function useStudioEvents(devices:DeviceStatus[],live:boolean) {
  const previous=useRef(new Map<string,Record<string,string>>());
  const priorLive=useRef<boolean|null>(null);
  const seq=useRef(0);
  const [events,setEvents]=useState<Entry[]>([]);
  useEffect(()=>{
    const now=new Date().toISOString(), next=new Map<string,Record<string,string>>(), entries:Entry[]=[];
    const add=(device:string,property:string,value:string)=>entries.push({id:++seq.current,observed_at:now,device,property,value});
    if(priorLive.current!==live){add("Engine","mode",live?"Live":"Offline / replay");priorLive.current=live;}
    for(const device of devices) {
      const id=device.descriptor.id,fields=stateFields(device);
      next.set(id,fields);
      const before=previous.current.get(id);
      for(const [property,value] of Object.entries(fields))if(before?.[property]!==value)add(id,property,value);
    }
    for(const id of previous.current.keys())if(!next.has(id))add(id,"state","Removed from state");
    previous.current=next;
    if(entries.length)setEvents(old=>[...old,...entries].slice(-MAX_EVENTS));
  },[devices,live]);
  return events;
}
export function StudioEvents({events}:{events:Entry[]}) {
  const [filter,setFilter]=useState("");
  const query=filter.toLowerCase();
  const visible=events.filter(e=>[e.device,e.property,e.value].join(" ").toLowerCase().includes(query)).toReversed();
  return <div className="lab-main">
    <section className="lab-card">
      <header><div><h3>Observed device events</h3><small className="subtle">Browser-observed state changes while Sensor Studio is open · not engine logs or sensor sample time</small></div><button disabled={!visible.length} onClick={()=>downloadText("bansayo-studio-events.json",JSON.stringify({source:"browser_observed_state",events:visible},null,2),"application/json")}>Export JSON</button></header>
      <div className="studio-subbar"><label>Filter <input aria-label="Filter events" placeholder="Device, property, value…" value={filter} onChange={e=>setFilter(e.target.value)}/></label><small className="subtle">{visible.length} / {events.length} events · latest {MAX_EVENTS} retained</small></div>
      <div className="lab-bench-results"><table><thead><tr><th>Browser time (UTC)</th><th>Device</th><th>Property</th><th>Value</th></tr></thead><tbody>{visible.map(e=><tr key={e.id}><td className="mono">{e.observed_at.slice(11,23)}</td><td className="mono">{e.device}</td><td>{e.property}</td><td>{e.value}</td></tr>)}</tbody></table></div>
      {!visible.length&&<p className="subtle">No matching events.</p>}
    </section>
  </div>;
}
