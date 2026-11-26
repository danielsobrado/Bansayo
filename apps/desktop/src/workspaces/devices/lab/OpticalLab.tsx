import {useState} from "react";
import {useSampled,useSystemState} from "../../../engine/hooks";
import {freshLandmarks,landmarks3dStore,STALE_FRAME_MS} from "../../../engine/stores";
import {calibratedCameras,intrinsics} from "../../../engine/cameraCalibration";
import {imageRay,projectWorld,nearestOnRay} from "./opticalMath";
import {OpticalViewport,type Ray} from "./OpticalViewport";
const MAX_SKEW_US=40_000;
export function OpticalLab({live}:{live:boolean}) {
  const state=useSystemState(s=>s);
  const frame=useSampled(4,()=>{const now=performance.now();return live&&now-landmarks3dStore.receivedAt<=STALE_FRAME_MS?{landmarks:landmarks3dStore.landmarks,images:freshLandmarks(now)}:null;});
  const [selected,setSelected]=useState(0);
  const data=frame?.landmarks,point=data?.points[selected];
  const profiles=(state?.calibration??[]).filter(p=>p.state==="valid");
  const cameras=calibratedCameras(profiles,state?.active_room??null);
  const rays:Ray[]=[];
  const rows:{id:string;skew:number;error:number;status:string}[]=[];
  if(point&&data&&frame)for(const camera of cameras){
    const image=frame.images.get(camera.id), observed=image?.points[selected];
    const profile=profiles.find(p=>p.device_id===camera.id&&p.kind==="camera_intrinsics");
    const i=intrinsics(profile?.data);
    if(!image||!observed||!i||image.schema_id!==data.schema_id||image.frame_meta.image_size_px.some((v,j)=>v!==i.image_size_px[j]))continue;
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
      <label>Landmark <select value={selected} onChange={e=>setSelected(Number(e.target.value))}>{data.points.map((_,i)=><option key={i} value={i}>Landmark {i}</option>)}</select></label>
      <OpticalViewport cameras={cameras} point={point?.m} rays={rays}/>
      {point?<><p className="mono">Solver RMS {point.reprojection_error_px.toFixed(2)} px · {point.views} views · σ {point.sigma_m==null?"—":(point.sigma_m*1000).toFixed(1)+" mm"}</p><table className="studio-stats"><thead><tr><th>Camera</th><th>Skew</th><th>Preview residual</th><th>Solver status</th></tr></thead><tbody>{rows.map(row=><tr key={row.id}><td>{row.id}</td><td>{row.skew.toFixed(1)} ms</td><td>{row.error.toFixed(2)} px</td><td>{row.status}</td></tr>)}</tbody></table></>:<p>Selected landmark unavailable.</p>}
    </>}
    <p className="subtle">Rays use latest compatible 2D detections within 40 ms and valid calibration at matching resolution. Preview residuals are not the solver's exact input residuals. Ray-to-point segments show geometric disagreement, not ground truth.</p>
  </section>;
}
