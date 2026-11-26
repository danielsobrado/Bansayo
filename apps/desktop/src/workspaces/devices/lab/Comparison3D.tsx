import {useEffect,useRef,useState} from "react";
import {AmbientLight,DirectionalLight,GridHelper,PerspectiveCamera,Scene,WebGLRenderer} from "three";
import {OrbitControls} from "three/addons/controls/OrbitControls.js";
import type {SkeletonDefinition,SkeletonPose} from "@contracts";
import {SkeletonRig} from "../../../viewport/SkeletonRig";
import {readPalette,tokenColor} from "../../../viewport/palette";

const TOKENS=["--accent","--prov-inferred","--state-tracking"];
class ComparisonScene {
  readonly scene=new Scene();
  readonly renderer:WebGLRenderer;
  readonly camera=new PerspectiveCamera(45,1,.05,100);
  readonly controls:OrbitControls;
  readonly rigs:SkeletonRig[];
  readonly resize:ResizeObserver;
  readonly theme:MutationObserver;
  readonly grid:GridHelper;
  readonly draw=()=>{if(!document.hidden)this.renderer.render(this.scene,this.camera);};
  constructor(host:HTMLElement,readonly skeleton:SkeletonDefinition){
    const palette=readPalette();
    this.renderer=new WebGLRenderer({antialias:true});
    this.renderer.setPixelRatio(Math.min(1.5,devicePixelRatio));
    host.append(this.renderer.domElement);
    this.renderer.domElement.setAttribute("role","img");
    this.renderer.domElement.setAttribute("aria-label","Orbitable 3D solver comparison");
    this.scene.background=palette.surface0;
    this.grid=new GridHelper(12,24,palette.ink3,palette.line);
    this.scene.add(this.grid,new AmbientLight(0xffffff,2));
    const light=new DirectionalLight(0xffffff,2);light.position.set(2,5,4);this.scene.add(light);
    this.rigs=TOKENS.map(token=>{
      const color=tokenColor(token);
      const rig=new SkeletonRig(skeleton,{...palette,measured:color,derived:color,inferred:color});
      this.scene.add(rig.root);return rig;
    });
    this.controls=new OrbitControls(this.camera,this.renderer.domElement);
    this.camera.position.set(3,2.5,6);this.controls.target.set(0,skeleton.height_m/2,0);this.controls.update();
    this.controls.addEventListener("change",this.draw);
    const measure=()=>{const w=Math.max(1,host.clientWidth),h=Math.max(1,host.clientHeight);this.camera.aspect=w/h;this.camera.updateProjectionMatrix();this.renderer.setSize(w,h,false);this.draw();};
    this.resize=new ResizeObserver(measure);this.resize.observe(host);measure();
    this.theme=new MutationObserver(()=>{const p=readPalette();this.scene.background=p.surface0;this.rigs.forEach((rig,i)=>{const color=tokenColor(TOKENS[i]!);rig.setPalette({...p,measured:color,derived:color,inferred:color});});this.draw();});
    this.theme.observe(document.documentElement,{attributeFilter:["data-theme"]});
  }
  update(poses:(SkeletonPose|null)[],selected:string,overlay:boolean){
    const segment=this.skeleton.segments.findIndex(s=>s.id===selected);
    this.rigs.forEach((rig,i)=>{
      const pose=poses[i];rig.root.visible=!!pose;
      if(!pose)return;
      rig.update(pose);rig.setSelected(segment,-1);
      const root=pose.segments[0]?.position_m??[0,0,0];
      if(overlay)rig.root.position.set(-root[0],this.skeleton.height_m/2-root[1],-root[2]);
      else rig.root.position.set((i-1)*this.skeleton.height_m*1.1,0,0);
    });this.draw();
  }
  dispose(){this.resize.disconnect();this.theme.disconnect();this.controls.removeEventListener("change",this.draw);this.controls.dispose();this.rigs.forEach(r=>r.dispose());this.grid.dispose();this.renderer.dispose();this.renderer.forceContextLoss();this.renderer.domElement.remove();}
}
export function Comparison3D({skeleton,poses,selected,overlay}:{skeleton:SkeletonDefinition;poses:(SkeletonPose|null)[];selected:string;overlay:boolean}){
  const host=useRef<HTMLDivElement>(null),scene=useRef<ComparisonScene|null>(null);
  const [failed,setFailed]=useState(false);
  // Solver previews deserialize a new definition each time; recreate only on actual definition changes.
  const definitionKey=JSON.stringify(skeleton);
  useEffect(()=>{if(!host.current)return;try{scene.current=new ComparisonScene(host.current,JSON.parse(definitionKey) as SkeletonDefinition);setFailed(false);}catch{setFailed(true);}return()=>{scene.current?.dispose();scene.current=null;};},[definitionKey]);
  useEffect(()=>scene.current?.update(poses,selected,overlay),[poses,selected,overlay,definitionKey]);
  return <div><div ref={host} className="studio-optical-canvas" hidden={failed}/>{failed&&<p>WebGL unavailable. Select an orthographic view.</p>}<small className="subtle">One shared orbit camera. Side-by-side offsets are for display only; overlay aligns roots.</small></div>;
}
