import {describe,it,expect} from "vitest";
import {adjustWindow,constrainWindow,initialWindow,parseWindowGeometry} from "./dockGeometry";
const viewport={width:1200,height:800},rect={x:100,y:100,width:400,height:300};
describe("floating instrument geometry",()=>{
  it("rejects invalid saved geometry",()=>{
    expect(parseWindowGeometry(null)).toBeUndefined();
    expect(parseWindowGeometry({...rect,x:Infinity})).toBeUndefined();
    expect(parseWindowGeometry({...rect,width:-1})).toBeUndefined();
    expect(parseWindowGeometry(rect)).toEqual(rect);
  });
  it("clamps movement and resizing",()=>{
    expect(adjustWindow(rect,viewport,"move",-1000,1000)).toEqual({...rect,x:8,y:492});
    expect(adjustWindow(rect,viewport,"resize",3000,3000)).toEqual({...rect,width:1092,height:692});
    expect(adjustWindow(rect,viewport,"resize",-3000,-3000)).toEqual({...rect,width:280,height:180});
  });
  it("recovers geometry on smaller displays",()=>{
    expect(constrainWindow(rect,{width:200,height:150})).toEqual({x:8,y:8,width:184,height:134});
    const g=initialWindow(viewport,2);
    expect(g.x+g.width).toBeLessThanOrEqual(1192);
    expect(g.y+g.height).toBeLessThanOrEqual(792);
  });
});
