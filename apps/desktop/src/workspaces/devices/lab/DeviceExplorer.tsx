import { useState } from "react";
import type { DeviceStatus, DeviceKind, SensorCapability } from "@contracts";
import { Icon, type IconName } from "../../../ui/Icon";
import { StatePill } from "../../../ui/StatePill";
import { CHANNELS, type ChannelId } from "./trace";
import type { SignalFocus, StudioView } from "./studioConfig";

const GROUPS: {kind:DeviceKind;label:string;icon:IconName}[] = [
  {kind:"imu_tracker",label:"IMU trackers",icon:"usb"},
  {kind:"camera",label:"Cameras",icon:"camera"},
  {kind:"xr_tracked_device",label:"OpenXR devices",icon:"layers"},
];
const CHANNEL_CAPABILITY: Partial<Record<ChannelId,SensorCapability>> = {
  orientation:"orientation",gyro:"angular_velocity",accel:"acceleration",position:"position",
  pose_rotation:"rigid_pose",pose_position:"rigid_pose",distance:"distance",contact:"contact",
};
export function deviceChannels(device: DeviceStatus): ChannelId[] {
  const ids = Object.entries(CHANNEL_CAPABILITY).filter(([,cap])=>device.descriptor.capabilities.includes(cap)).map(([id])=>id as ChannelId);
  if(device.descriptor.kind==="imu_tracker" && device.descriptor.source_id==="mosaico") ids.push("mag0","mag1");
  return ids;
}
export function DeviceExplorer({devices,selected,focus,view,onSelect}:{
  devices:DeviceStatus[];selected:string;focus:SignalFocus;view:StudioView;onSelect:(id:string,component?:SignalFocus)=>void;
}) {
  const [filter,setFilter]=useState("");
  const [collapsed,setCollapsed]=useState<string[]>([]);
  const [expanded,setExpanded]=useState<string[]>([]);
  function toggle(list:string[],id:string) {return list.includes(id)?list.filter(x=>x!==id):[...list,id];}
  const query=filter.trim().toLowerCase();
  return <aside className="studio-explorer" aria-label="Device explorer">
    <div className="studio-panel-title"><Icon name="folder"/><strong>DEVICE EXPLORER</strong></div>
    <input className="studio-search" aria-label="Filter devices" placeholder="Filter devices…" value={filter} onChange={e=>setFilter(e.target.value)}/>
    {GROUPS.map(group=>{
      const all=devices.filter(d=>d.descriptor.kind===group.kind);
      const items=all.filter(d=>[d.descriptor.id,d.descriptor.model??"",...deviceChannels(d).map(id=>CHANNELS[id].title)].join(" ").toLowerCase().includes(query));
      if(!items.length)return null;
      const open=!!query||!collapsed.includes(group.kind);
      return <section key={group.kind} className="studio-tree-group">
        <button className="studio-tree-heading" aria-expanded={open} onClick={()=>setCollapsed(s=>toggle(s,group.kind))}><span className={open?"studio-chevron":"studio-chevron closed"}><Icon name="chevron-down"/></span><Icon name={group.icon}/><span>{group.label}</span><small>{items.length}</small></button>
        {open&&items.map(d=>{
          const id=d.descriptor.id,active=selected===id;
          const children=deviceChannels(d);
          const opened=!!query||expanded.includes(id);
          return <div key={id}>
            <div className="studio-tree-device" data-selected={active&&view!=="compare"}>
              <button aria-label={`Expand ${id}`} aria-expanded={opened} disabled={!children.length} onClick={()=>setExpanded(s=>toggle(s,id))}><span className={opened?"studio-chevron":"studio-chevron closed"}><Icon name="chevron-down" size={14}/></span></button>
              <button className="studio-device-name" aria-pressed={active&&view!=="compare"&&focus==="all"} onClick={()=>{onSelect(id);if(!expanded.includes(id))setExpanded(s=>[...s,id]);}} title={id}><span>{id}</span><StatePill state={d.state}/></button>
            </div>
            {opened&&children.map(channel=><button className="studio-tree-channel" key={channel} aria-pressed={active&&view==="signals"&&focus===channel} onClick={()=>onSelect(id,channel)}><Icon name={channel==="orientation"||channel==="pose_rotation"?"rotate":"chart"} size={14}/><span>{CHANNELS[channel].title}</span></button>)}
          </div>;
        })}
      </section>;
    })}
    {!devices.some(d=>[d.descriptor.id,d.descriptor.model??"",...deviceChannels(d).map(id=>CHANNELS[id].title)].join(" ").toLowerCase().includes(query))&&<p className="subtle studio-no-match">No matching devices.</p>}
  </aside>;
}
