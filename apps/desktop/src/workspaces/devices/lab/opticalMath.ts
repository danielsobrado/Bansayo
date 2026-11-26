import type {CameraIntrinsicsData} from "@contracts";
import type {CameraPose} from "../../../engine/cameraCalibration";
import {rotateVec3,type Vec3,type Quat} from "../../../lib/quat";
function distort(x:number,y:number,i:CameraIntrinsicsData):[number,number] {
  const [k1,k2,p1,p2,k3]=i.distortion,r2=x*x+y*y,radial=1+k1*r2+k2*r2*r2+k3*r2*r2*r2;
  return [x*radial+2*p1*x*y+p2*(r2+2*x*x),y*radial+p1*(r2+2*y*y)+2*p2*x*y];
}
export function projectWorld(p:Vec3,c:CameraPose,i:CameraIntrinsicsData):[number,number]|null {
  const inv:Quat=[-c.q[0],-c.q[1],-c.q[2],c.q[3]];
  const [x,y,z]=rotateVec3(inv,[p[0]-c.t[0],p[1]-c.t[1],p[2]-c.t[2]],[0,0,0]);
  if(z<=1e-6)return null;
  const [u,v]=distort(x/z,y/z,i),px:[number,number]=[u*i.fx_px+i.cx_px,v*i.fy_px+i.cy_px];
  return px.every(Number.isFinite)?px:null;
}
export function imageRay(px:[number,number],c:CameraPose,i:CameraIntrinsicsData):Vec3|null {
  const u=(px[0]-i.cx_px)/i.fx_px,v=(px[1]-i.cy_px)/i.fy_px;
  let x=u,y=v;
  for(let j=0;j<20;j++){const [a,b]=distort(x,y,i);x+=u-a;y+=v-b;if(!Number.isFinite(x+y)||Math.abs(x)+Math.abs(y)>100)return null;}
  const [a,b]=distort(x,y,i);
  if(Math.hypot((a-u)*i.fx_px,(b-v)*i.fy_px)>.25)return null;
  const n=Math.hypot(x,y,1);return rotateVec3(c.q,[x/n,y/n,1/n],[0,0,0]);
}
export function nearestOnRay(origin:Vec3,direction:Vec3,p:Vec3):Vec3 {
  const t=Math.max(0,p.reduce((s,v,i)=>s+(v-origin[i]!)*direction[i]!,0));
  return origin.map((v,i)=>v+t*direction[i]!) as Vec3;
}
