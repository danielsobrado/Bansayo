import {useState} from "react";
import {useSampled,useSystemState} from "../../../engine/hooks";
import {freshLandmarks,landmarks3dStore,STALE_FRAME_MS} from "../../../engine/stores";
import {calibratedCameras,intrinsics,roomOf} from "../../../engine/cameraCalibration";
import {imageRay,projectWorld,nearestOnRay} from "./opticalMath";
import {OpticalViewport,type Ray} from "./OpticalViewport";
const MAX_SKEW_US=40_000;
export function OpticalLab({live}:{live:boolean}) {
  const state=useSystemState(s=>s);
  const frame=useSampled(4,()=>{const now=performance.now();return live&&now-landmarks3dStore.receivedAt<=STALE_FRAME_MS?{landmarks:landmarks3dStore.landmarks,images:freshLandmarks(now)}:null;});
  const [selected,setSelected]=useState(0);
  const data=live?frame?.landmarks:null;
  const index=data&&selected<data.points.length?selected:0;
  const point=data?.points[index];
  const room=state?.active_room??null;
  const profiles=(state?.calibration??[]).filter(p=>p.state==="valid"&&
    data?.calibration_refs.includes(p.id)&&
    (p.kind!=="camera_extrinsics"||roomOf(p)===room));
  const cameras=calibratedCameras(profiles,room);
  const rays:Ray[]=[];
  const rows:{id:string;skew:number|null;error:number|null;status:string}[]=[];
  const exact=point?.view_diagnostics;
  if(point&&exact){
    for(const view of exact){
      const status=view.used?"Used":"Rejected";
      const nearest=nearestOnRay(view.ray_origin_m,view.ray_direction,point.m);
      rays.push({origin:view.ray_origin_m,end:nearest,point:point.m,status});
      rows.push({id:view.device_id,skew:null,error:view.error_px,status});
    }
  }else if(point&&data&&frame)for(const camera of cameras){
    const image=frame.images.get(camera.id), observed=image?.points[index];
    const profile=profiles.find(p=>p.device_id===camera.id&&p.kind==="camera_intrinsics");
    const i=intrinsics(profile?.data);
    if(!image||!observed||observed.confidence<=0||!i||image.schema_id!==data.schema_id||image.points.length!==data.points.length||image.frame_meta.image_size_px.some((v,j)=>v!==i.image_size_px[j]))continue;
    const skew=Math.abs(image.frame_meta.t_us-data.t_us);
    if(skew>MAX_SKEW_US)continue;
    const px:[number,number]=[observed.x_px,observed.y_px],direction=imageRay(px,camera,i),projected=projectWorld(point.m,camera,i);
    if(!direction||!projected)continue;
    const nearest=nearestOnRay(camera.t,direction,point.m);
    const status=point.used_views.includes(camera.id)?"Used":point.rejected_views.includes(camera.id)?"Rejected":"Other";
    rays.push({origin:camera.t,end:nearest,point:point.m,status});
    rows.push({id:camera.id,skew:skew/1000,error:Math.hypot(px[0]-projected[0],px[1]-projected[1]),status});
  }
  return <section className="lab-card"><header><h3>Triangulation / reprojection laboratory</h3><small className="subtle">Live calibrated geometry</small></header>
    {!data?<p>Waiting for fresh triangulated landmarks.</p>:<>
      <label>Landmark <select value={index} onChange={e=>setSelected(Number(e.target.value))}>{data.points.map((_,i)=><option key={i} value={i}>Landmark {i}</option>)}</select></label>
      <OpticalViewport cameras={cameras} point={point?.m} rays={rays}/>
      {!cameras.length&&<p className="subtle">No matching active calibration profiles for this reconstruction.</p>}
      {point?<><p className="mono">Solver RMS {point.reprojection_error_px.toFixed(2)} px · {point.views} views · σ {point.sigma_m==null?"—":(point.sigma_m*1000).toFixed(1)+" mm"}</p><table className="studio-stats"><thead><tr><th>Camera</th><th>Preview skew</th><th>{exact?"Solver residual":"Preview residual"}</th><th>Solver status</th></tr></thead><tbody>{rows.map(row=><tr key={row.id}><td>{row.id}</td><td>{row.skew===null?"—":row.skew.toFixed(1)+" ms"}</td><td>{row.error===null?"Not projectable":row.error.toFixed(2)+" px"}</td><td>{row.status}</td></tr>)}</tbody></table></>:<p>Selected landmark unavailable.</p>}
    </>}
    <p className="subtle">{exact?"Exact aligned solver inputs and final-point reprojection, including rejected views. Camera frustums show matching currently available profiles; rays use the original solver geometry.":"Legacy reconstruction: rays use compatible 2D previews within 40 ms, not exact solver inputs."} Ray-to-point segments show geometric disagreement, not ground truth.</p>
  </section>;
}
