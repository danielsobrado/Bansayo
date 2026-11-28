import type {CalibrationRef} from "@contracts";
import {FACE_TOLERANCE,ROTATION_TOLERANCE_DEG} from "./benchMath";
import {CALIBRATION_SCORE as C} from "./reliabilityConfig";
export function profileReadiness(profiles:readonly CalibrationRef[]){
  const valid=profiles.filter(p=>p.state==="valid").length;
  return {valid,total:profiles.length,percent:profiles.length?Math.round(valid/profiles.length*100):null};
}
export function benchQuality(faces:Partial<Record<number,{error:number}>>,turns:Partial<Record<string,{angle:number;axisError:number}>>,drift:{angle:number;seconds:number}|null){
  const gates:{name:string;pass:boolean|null}[]=[];
  for(let i=0;i<6;i++){const f=faces[i];gates.push({name:"Face "+(i+1),pass:f&&Number.isFinite(f.error)&&f.error>=0?f.error<=FACE_TOLERANCE:null});}
  for(const axis of ["X","Y","Z"]){const t=turns[axis];gates.push({name:axis+" rotation",pass:t&&Number.isFinite(t.angle)&&Number.isFinite(t.axisError)&&t.axisError>=0?Math.abs(t.angle-90)<=ROTATION_TOLERANCE_DEG&&t.axisError<=ROTATION_TOLERANCE_DEG:null});}
  gates.push({name:"Endpoint drift",pass:drift&&Number.isFinite(drift.angle)&&drift.angle>=0&&Number.isFinite(drift.seconds)&&drift.seconds>=C.minimumDriftSeconds?drift.angle<=C.maxDriftDegrees:null});
  const completed=gates.filter(g=>g.pass!==null).length,passing=gates.filter(g=>g.pass===true).length;
  return {gates,completed,passing,score:completed===gates.length?Math.round(100*passing/gates.length):null};
}
