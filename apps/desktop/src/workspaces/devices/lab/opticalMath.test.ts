import {describe,it,expect} from "vitest";
import {projectWorld,imageRay,nearestOnRay} from "./opticalMath";
import type {CameraPose} from "../../../engine/cameraCalibration";
import type {CameraIntrinsicsData} from "@contracts";
const camera:CameraPose={id:"c",q:[0,0,0,1],t:[0,0,0],fovY:1,aspect:1};
const i:CameraIntrinsicsData={image_size_px:[640,480],fx_px:500,fy_px:500,cx_px:320,cy_px:240,distortion:[.1,-.01,.001,.002,0]};
describe("optical instrumentation",()=>{it("round trips a distorted projected ray and rejects behind-camera points",()=>{const p:[number,number,number]=[.2,.3,2];const px=projectWorld(p,camera,i)!;const ray=imageRay(px,camera,i)!;const nearest=nearestOnRay(camera.t,ray,p);expect(nearest[0]).toBeCloseTo(p[0],4);expect(nearest[1]).toBeCloseTo(p[1],4);expect(projectWorld([0,0,-1],camera,i)).toBeNull();});});
