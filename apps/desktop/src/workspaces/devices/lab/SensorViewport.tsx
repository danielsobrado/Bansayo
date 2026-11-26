import { useEffect,useRef,useState,type ReactNode } from "react";
import { Icon } from "../../../ui/Icon";
import { SensorScene,type SensorView } from "./SensorScene";
import { AXIS_CSS } from "./studioConfig";

export function SensorViewport({q,accel,frozen,fallback}:{q:number[];accel?:number[]|undefined; frozen:boolean;fallback:ReactNode}) {
  const host=useRef<HTMLDivElement>(null),scene=useRef<SensorScene|null>(null);
  const [failed,setFailed]=useState(false);
  const [view,setView]=useState<SensorView>("3D");
  const [vectors,setVectors]=useState(true);
  useEffect(()=>{
    if(!host.current)return;
    try {scene.current=new SensorScene(host.current);setFailed(false);} catch {setFailed(true);}
    return()=>{scene.current?.dispose();scene.current=null;};
  },[]);
  useEffect(()=>{scene.current?.setReading(q,accel,vectors);},[q,accel,vectors]);
  function select(next:SensorView){setView(next);scene.current?.setView(next);}
  return <div className="studio-sensor-viewport">
    <div className="studio-sensor-controls">{(["3D","Front","Side","Top"] as const).map(v=><button key={v} disabled={failed} aria-pressed={v===view} onClick={()=>select(v)}>{v}</button>)}<button disabled={failed} title="Toggle measured vectors" aria-pressed={vectors} onClick={()=>setVectors(v=>!v)}><Icon name="layers"/> Vectors</button><small className="subtle">{frozen?"Frozen":"Sampled"}</small></div>
    <div ref={host} className="studio-sensor-canvas" hidden={failed}/>
    {failed&&<><p className="subtle">WebGL unavailable — wireframe fallback.</p>{fallback}</>}
    <div className="studio-vector-legend"><span style={{color:AXIS_CSS[0]}}>X</span><span style={{color:AXIS_CSS[1]}}>Y</span><span style={{color:AXIS_CSS[2]}}>Z</span>{vectors&&<><span>Reference up ↑</span><span style={{color:"var(--studio-force)"}}>Specific force</span> </>}</div>
    <p className="mono subtle">Q XYZW · {q.map(v=>v.toFixed(3)).join(" / ")}</p>
  </div>;
}
