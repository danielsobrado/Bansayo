export interface WindowGeometry { x:number; y:number; width:number; height:number }
export interface ViewportSize { width:number; height:number }
export type WindowAction="move"|"resize";
const MARGIN=8, MIN_WIDTH=280, MIN_HEIGHT=180, DEFAULT_WIDTH=460, DEFAULT_HEIGHT=360;
export const WINDOW_KEY_STEP=20;
const clamp=(v:number,min:number,max:number)=>Math.max(min,Math.min(max,v));
export function parseWindowGeometry(value:unknown):WindowGeometry|undefined {
  if(!value||typeof value!=="object")return;
  const g=value as WindowGeometry;
  if(![g.x,g.y,g.width,g.height].every(Number.isFinite)||g.width<=0||g.height<=0)return;
  return {x:g.x,y:g.y,width:g.width,height:g.height};
}
export function constrainWindow(g:WindowGeometry,v:ViewportSize):WindowGeometry {
  const aw=Math.max(1,v.width-2*MARGIN),ah=Math.max(1,v.height-2*MARGIN);
  const width=clamp(g.width,Math.min(MIN_WIDTH,aw),aw),height=clamp(g.height,Math.min(MIN_HEIGHT,ah),ah);
  return {width,height,x:clamp(g.x,MARGIN,Math.max(MARGIN,v.width-MARGIN-width)),y:clamp(g.y,MARGIN,Math.max(MARGIN,v.height-MARGIN-height))};
}
export function initialWindow(v:ViewportSize,index:number):WindowGeometry {
  return constrainWindow({x:v.width-DEFAULT_WIDTH-24-index*28,y:100+index*28,width:DEFAULT_WIDTH,height:DEFAULT_HEIGHT},v);
}
export function adjustWindow(g:WindowGeometry,v:ViewportSize,action:WindowAction,dx:number,dy:number):WindowGeometry {
  const s=constrainWindow(g,v);
  if(action==="move")return constrainWindow({...s,x:s.x+dx,y:s.y+dy},v);
  return constrainWindow({...s,width:Math.min(s.width+dx,v.width-MARGIN-s.x),height:Math.min(s.height+dy,v.height-MARGIN-s.y)},v);
}
