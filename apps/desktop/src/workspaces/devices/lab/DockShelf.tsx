import {useEffect,useState,type ReactNode} from "react";
import type {DeviceStatus} from "@contracts";
import {Icon} from "../../../ui/Icon";
import {useInspection} from "./useInspection";
import {freshChannels,CHANNELS,type ChannelId,CHANNEL_IDS} from "./trace";
import {SignalWorkspace} from "./SignalWorkspace";
import {OrientationView} from "./Plots";
import {TimingPanel} from "./SignalPanels";
import {OpticalLab} from "./OpticalLab";
import {CameraPreview} from "../../cameras/CameraPreview";
const STORAGE="bansayo.studio.docks.v1",MAX=3;
type Kind="signals"|"orientation"|"timing"|"camera"|"optical";
type Dock={kind:Kind;device:string;channel:ChannelId|"all"};
const KINDS:Kind[]=["signals","orientation","timing","camera","optical"];
function load():Dock[]{
  try{const v:unknown=JSON.parse(localStorage.getItem(STORAGE)??"null");return Array.isArray(v)?v.filter((x):x is Dock=>!!x&&KINDS.includes(x.kind)&&typeof x.device==="string"&&(x.channel==="all"||CHANNEL_IDS.includes(x.channel))).slice(0,MAX):[];}catch{return [];}
}
function SignalDock({dock,device,live}:{dock:Dock;device:DeviceStatus;live:boolean}){
  const reading=useInspection(device.descriptor.id,live,false);
  const raw=reading.snapshot?.channels??{};
  const channels=freshChannels(raw,performance.now()-reading.receivedAt,live&&device.state!=="disconnected"&&device.state!=="unavailable");
  const [focus,setFocus]=useState(dock.channel);
  if(dock.kind==="orientation")return <OrientationView q={(channels.orientation??channels.pose_rotation)?.points.at(-1)?.values}/>;
  if(dock.kind==="timing")return <TimingPanel timing={channels.orientation?reading.snapshot?.timing:undefined}/>;
  return <SignalWorkspace device={device} channels={channels} known={raw} focus={focus} onFocus={setFocus} frozen={false}/>;
}
function content(dock:Dock,device:DeviceStatus|undefined,live:boolean):ReactNode{
  if(dock.kind==="optical")return <OpticalLab live={live}/>;
  if(!device)return <p className="subtle">Device no longer configured.</p>;
  if(dock.kind==="camera")return live&&device.state!=="disconnected"&&device.state!=="unavailable"?<CameraPreview device={device} showAprilTags/>:<p>Camera unavailable.</p>;
  return <SignalDock dock={dock} device={device} live={live}/>;
}
export function DockShelf({devices,live,children}:{devices:DeviceStatus[];live:boolean;children:ReactNode}){
  const [docks,setDocks]=useState(load),[device,setDevice]=useState(""),[kind,setKind]=useState<Kind>("signals"),[channel,setChannel]=useState<ChannelId|"all">("all"),[collapsed,setCollapsed]=useState(false),[drag,setDrag]=useState<number|null>(null);
  const choices=devices.filter(d=>kind==="camera"?d.descriptor.kind==="camera":d.descriptor.kind!=="camera");
  const id=choices.some(d=>d.descriptor.id===device)?device:choices[0]?.descriptor.id??"";
  useEffect(()=>{try{localStorage.setItem(STORAGE,JSON.stringify(docks));}catch{/* storage disabled */}},[docks]);
  function move(from:number,to:number){setDocks(old=>{const next=[...old],item=next.splice(from,1)[0];if(item)next.splice(to,0,item);return next;});}
  function add(){const dock:Dock={kind,device:kind==="optical"?"":id,channel:kind==="signals"?channel:"all"};if(docks.length<MAX&&(dock.device||kind==="optical")&&!docks.some(d=>JSON.stringify(d)===JSON.stringify(dock))){setDocks(v=>[...v,dock]);setCollapsed(false);}}
  return <div className="studio-dock-shell" data-docked={!collapsed&&docks.length>0}>
    <div className="studio-primary">{children}</div>
    <aside className="studio-shelf" aria-label="Pinned instruments">
      <div className="studio-subbar"><button aria-pressed={!collapsed} onClick={()=>setCollapsed(v=>!v)}><Icon name="layers"/> Pinned instruments ({docks.length}/{MAX})</button>
      <select aria-label="Dock instrument" value={kind} onChange={e=>setKind(e.target.value as Kind)}>{KINDS.map(k=><option key={k}>{k}</option>)}</select>
      {kind!=="optical"&&<select aria-label="Dock device" value={id} onChange={e=>setDevice(e.target.value)}>{choices.map(d=><option key={d.descriptor.id}>{d.descriptor.id}</option>)}</select>}
      {kind==="signals"&&<select aria-label="Dock channel" value={channel} onChange={e=>setChannel(e.target.value as ChannelId|"all")}><option value="all">All</option>{CHANNEL_IDS.map(c=><option key={c} value={c}>{CHANNELS[c].title}</option>)}</select>}
      <button disabled={docks.length>=MAX||(!id&&kind!=="optical")} onClick={add}>Pin</button></div>
      {!collapsed&&docks.map((dock,i)=><section className="studio-pinned" key={JSON.stringify(dock)} onDragOver={e=>e.preventDefault()} onDrop={e=>{e.preventDefault();if(drag!==null)move(drag,i);setDrag(null);}}>
        <header className="studio-subbar"><span draggable onDragStart={e=>{setDrag(i);e.dataTransfer.setData("text/plain",String(i));}} onDragEnd={()=>setDrag(null)}>⠿</span><strong>{dock.device||"Room"} · {dock.kind}</strong><button disabled={i===0} aria-label="Move pinned instrument up" onClick={()=>move(i,i-1)}>↑</button><button disabled={i===docks.length-1} aria-label="Move pinned instrument down" onClick={()=>move(i,i+1)}>↓</button><button aria-label={"Unpin "+dock.kind} onClick={()=>setDocks(v=>v.filter((_,j)=>i!==j))}>×</button></header>
        {content(dock,devices.find(d=>d.descriptor.id===dock.device),live)}
      </section>)}
    </aside>
  </div>;
}
