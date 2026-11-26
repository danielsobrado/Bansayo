import { AmbientLight,ArrowHelper,BoxGeometry,Color,DirectionalLight,GridHelper,Group,Mesh,MeshStandardMaterial,PerspectiveCamera,Quaternion,Scene,Vector3,WebGLRenderer } from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { readPalette } from "../../../viewport/palette";
import { AXIS_COLORS } from "./trace";

export type SensorView = "3D"|"Front"|"Side"|"Top";
const POSITIONS:Record<SensorView,[number,number,number]>={"3D":[3.2,2.7,3.7],Front:[0,0,5],Side:[5,0,0],Top:[0,5,0.001]};
const AXES=[[1,0,0],[0,1,0],[0,0,1]] as const;
export class SensorScene {
  private readonly renderer:WebGLRenderer;
  private readonly scene=new Scene();
  private readonly camera=new PerspectiveCamera(42,1,.1,30);
  private readonly body=new Group();
  private readonly controls:OrbitControls;
  private readonly resize:ResizeObserver;
  private readonly theme:MutationObserver;
  private readonly grid:GridHelper;
  private readonly force:ArrowHelper;
  private readonly gravity:ArrowHelper;
  private readonly modelMaterial:MeshStandardMaterial;
  private readonly chipMaterial:MeshStandardMaterial;
  private readonly rotation=new Quaternion();
  private readonly direction=new Vector3();
  private readonly draw=()=>{if(!document.hidden)this.renderer.render(this.scene,this.camera);};

  constructor(host:HTMLElement) {
    const palette=readPalette();
    this.renderer=new WebGLRenderer({antialias:true,alpha:false});
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio,1.5));
    this.renderer.domElement.setAttribute("role","img");
    this.renderer.domElement.setAttribute("aria-label","Interactive sensor orientation, local axes and measured vectors");
    host.append(this.renderer.domElement);
    this.scene.background=palette.surface0;
    this.scene.add(new AmbientLight(0xffffff,2));
    const light=new DirectionalLight(0xffffff,2.4);light.position.set(3,5,4);this.scene.add(light);
    this.grid=new GridHelper(5,10,palette.ink3,palette.line);this.grid.position.y=-1.5;this.scene.add(this.grid);
    this.modelMaterial=new MeshStandardMaterial({color:palette.accent,roughness:.62,metalness:.15});
    this.chipMaterial=new MeshStandardMaterial({color:palette.ink3,roughness:.8});
    const board=new Mesh(new BoxGeometry(1.6,.18,1.25),this.modelMaterial);
    const chip=new Mesh(new BoxGeometry(.62,.15,.54),this.chipMaterial);chip.position.y=.165;
    this.body.add(board,chip);
    AXES.forEach((axis,i)=>this.body.add(new ArrowHelper(new Vector3(...axis),new Vector3(),1.35,new Color(AXIS_COLORS[i]!),.22,.12)));
    this.scene.add(this.body);
    this.force=new ArrowHelper(new Vector3(0,1,0),new Vector3(),1.7,0xd6aa63,.24,.14);
    this.gravity=new ArrowHelper(new Vector3(0,1,0),new Vector3(-1.45,-.9,0),1,0x9caab8,.2,.1);
    this.force.visible=false;
    this.scene.add(this.force,this.gravity);
    this.controls=new OrbitControls(this.camera,this.renderer.domElement);
    this.controls.enableDamping=false;this.controls.minDistance=2;this.controls.maxDistance=10;
    this.controls.addEventListener("change",this.draw);
    this.setView("3D");
    const measure=()=>{const w=Math.max(1,host.clientWidth),h=Math.max(1,host.clientHeight);this.camera.aspect=w/h;this.camera.updateProjectionMatrix();this.renderer.setSize(w,h,false);this.draw();};
    this.resize=new ResizeObserver(measure);this.resize.observe(host);measure();
    this.theme=new MutationObserver(()=>{const colors=readPalette();this.scene.background=colors.surface0;this.modelMaterial.color.copy(colors.accent);this.chipMaterial.color.copy(colors.ink3);this.draw();});
    this.theme.observe(document.documentElement,{attributeFilter:["data-theme"]});
  }
  setView(view:SensorView) {this.camera.position.set(...POSITIONS[view]);this.controls.target.set(0,0,0);this.controls.update();this.draw();}
  setReading(q:number[],accel:number[]|undefined,vectors:boolean) {
    const norm=Math.hypot(...q);
    if(q.length!==4||!Number.isFinite(norm)||norm<1e-6){this.body.visible=false;this.force.visible=false;this.draw();return;}
    this.body.visible=true;
    this.rotation.set(q[0]!,q[1]!,q[2]!,q[3]!).normalize();
    this.body.quaternion.copy(this.rotation);
    this.gravity.visible=vectors;
    this.updateVector(this.force,accel,vectors);
    this.draw();
  }
  private updateVector(arrow:ArrowHelper,values:number[]|undefined,enabled:boolean) {
    arrow.visible=false;
    if(!enabled||!values||values.length!==3||!values.every(Number.isFinite)||Math.hypot(...values)<1e-6)return;
    this.direction.set(values[0]!,values[1]!,values[2]!).normalize().applyQuaternion(this.rotation);
    arrow.setDirection(this.direction);arrow.visible=true;
  }
  dispose() {
    this.resize.disconnect();this.theme.disconnect();
    this.controls.removeEventListener("change",this.draw);this.controls.dispose();
    this.scene.traverse(object=>{if(object instanceof Mesh){object.geometry.dispose();const materials=Array.isArray(object.material)?object.material:[object.material];materials.forEach(m=>m.dispose());}});
    this.grid.dispose();
    for(const arrow of [this.force,this.gravity,...this.body.children.filter((o):o is ArrowHelper=>o instanceof ArrowHelper)])arrow.dispose();
    this.renderer.dispose();this.renderer.forceContextLoss();this.renderer.domElement.remove();
  }
}
