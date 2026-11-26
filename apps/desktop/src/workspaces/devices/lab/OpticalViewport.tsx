import {useEffect,useRef,useState} from "react";
import {BufferGeometry,GridHelper,Group,Line,LineBasicMaterial,Mesh,MeshBasicMaterial,PerspectiveCamera,Scene,SphereGeometry,Vector3,WebGLRenderer} from "three";
import {OrbitControls} from "three/addons/controls/OrbitControls.js";
import {CameraFrustums} from "../../../viewport/CameraFrustums";
import {readPalette} from "../../../viewport/palette";
import type {CameraPose} from "../../../engine/cameraCalibration";
import type {Vec3} from "../../../lib/quat";
export interface Ray {origin:Vec3;end:Vec3;point:Vec3;status:string}
class SceneView {
  readonly renderer:WebGLRenderer;
  readonly scene=new Scene();
  readonly camera=new PerspectiveCamera(45,1,.05,100);
  readonly controls:OrbitControls;
  readonly resize:ResizeObserver;
  readonly frustums:CameraFrustums;
  readonly grid:GridHelper;
  readonly geometry=new Group();
  readonly draw=()=>{if(!document.hidden)this.renderer.render(this.scene,this.camera);};
  constructor(host:HTMLElement){
    const palette=readPalette();this.renderer=new WebGLRenderer({antialias:true});
    this.renderer.setPixelRatio(Math.min(1.5,devicePixelRatio));host.append(this.renderer.domElement);
    this.renderer.domElement.setAttribute("role","img");this.renderer.domElement.setAttribute("aria-label","3D triangulation rays and camera frustums");
    this.scene.background=palette.surface0;this.grid=new GridHelper(10,20,palette.ink3,palette.line);this.scene.add(this.grid,this.geometry);
    this.frustums=new CameraFrustums(palette);this.scene.add(this.frustums.root);
    this.camera.position.set(4,3,5);this.controls=new OrbitControls(this.camera,this.renderer.domElement);this.controls.target.set(0,1,0);this.controls.update();this.controls.addEventListener("change",this.draw);
    const resize=()=>{const w=Math.max(1,host.clientWidth),h=Math.max(1,host.clientHeight);this.camera.aspect=w/h;this.camera.updateProjectionMatrix();this.renderer.setSize(w,h,false);this.draw();};
    this.resize=new ResizeObserver(resize);this.resize.observe(host);resize();
  }
  clear(){for(const child of [...this.geometry.children]){if(child instanceof Line||child instanceof Mesh){child.geometry.dispose();const materials=Array.isArray(child.material)?child.material:[child.material];materials.forEach(m=>m.dispose());}child.removeFromParent();}}
  update(cameras:CameraPose[],point:Vec3|undefined,rays:Ray[]){
    this.frustums.set(cameras,null);this.clear();
    const line=(a:Vec3,b:Vec3,color:number)=>this.geometry.add(new Line(new BufferGeometry().setFromPoints([new Vector3(...a),new Vector3(...b)]),new LineBasicMaterial({color})));
    for(const r of rays){line(r.origin,r.end,r.status==="Rejected"?0xe4aa62:0x59b7da);line(r.end,r.point,0xee7878);}
    if(point){const mesh=new Mesh(new SphereGeometry(.035,12,8),new MeshBasicMaterial({color:0x58bd91}));mesh.position.set(...point);this.geometry.add(mesh);}
    this.draw();
  }
  dispose(){this.resize.disconnect();this.controls.removeEventListener("change",this.draw);this.controls.dispose();this.clear();this.frustums.dispose();this.grid.dispose();this.renderer.dispose();this.renderer.forceContextLoss();this.renderer.domElement.remove();}
}
export function OpticalViewport({cameras,point,rays}:{cameras:CameraPose[];point?:Vec3; rays:Ray[]}){
  const host=useRef<HTMLDivElement>(null),scene=useRef<SceneView|null>(null);
  const [error,setError]=useState(false);
  useEffect(()=>{if(!host.current)return;try{scene.current=new SceneView(host.current);setError(false);}catch{setError(true);}return()=>{scene.current?.dispose();scene.current=null;};},[]);
  useEffect(()=>scene.current?.update(cameras,point,rays),[cameras,point,rays]);
  return <div><div ref={host} className="studio-optical-canvas" hidden={error}/>{error&&<p>WebGL unavailable. Numerical residual table remains accessible.</p>}<small className="subtle">Blue: observed ray · amber: rejected view · red: closest-ray residual · green: triangulated landmark</small></div>;
}
