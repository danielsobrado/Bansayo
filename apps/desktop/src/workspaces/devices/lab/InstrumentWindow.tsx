import {useEffect,useRef,useState,type DragEventHandler,type KeyboardEvent,type PointerEvent,type ReactNode} from "react";
import {adjustWindow,constrainWindow,WINDOW_KEY_STEP,type WindowAction,type WindowGeometry} from "./dockGeometry";
const viewport=()=>({width:window.innerWidth,height:window.innerHeight});
interface Gesture {pointer:number;action:WindowAction;x:number;y:number;start:WindowGeometry;last:WindowGeometry}
interface Props {
  label:string;geometry?:WindowGeometry|undefined;active:boolean;toolbar:ReactNode;children:ReactNode;
  onActivate:()=>void;onGeometry:(g:WindowGeometry)=>void;
  onDragOver:DragEventHandler<HTMLElement>;onDrop:DragEventHandler<HTMLElement>;
}
export function InstrumentWindow({label,geometry,active,toolbar,children,onActivate,onGeometry,onDragOver,onDrop}:Props){
  const [size,setSize]=useState(viewport);
  const [draft,setDraft]=useState<WindowGeometry>();
  const gesture=useRef<Gesture|null>(null);
  useEffect(()=>{const resize=()=>setSize(viewport());window.addEventListener("resize",resize);return()=>window.removeEventListener("resize",resize);},[]);
  const rect=geometry?constrainWindow(draft??geometry,size):undefined;
  function start(e:PointerEvent<HTMLButtonElement>,action:WindowAction){
    if(!rect||e.button!==0||gesture.current)return;
    e.currentTarget.setPointerCapture(e.pointerId);
    gesture.current={pointer:e.pointerId,action,x:e.clientX,y:e.clientY,start:rect,last:rect};
    onActivate();
  }
  function move(e:PointerEvent<HTMLButtonElement>){
    const g=gesture.current;if(!g||g.pointer!==e.pointerId)return;
    g.last=adjustWindow(g.start,viewport(),g.action,e.clientX-g.x,e.clientY-g.y);setDraft(g.last);
  }
  function finish(e:PointerEvent<HTMLButtonElement>,commit:boolean){
    const g=gesture.current;if(!g||g.pointer!==e.pointerId)return;
    gesture.current=null;setDraft(undefined);
    if(commit)onGeometry(g.last);
  }
  function key(e:KeyboardEvent<HTMLButtonElement>,action:WindowAction){
    if(!rect)return;
    const delta:Record<string,[number,number]>={ArrowLeft:[-1,0],ArrowRight:[1,0],ArrowUp:[0,-1],ArrowDown:[0,1]};
    const direction=delta[e.key];if(!direction)return;
    e.preventDefault();onActivate();
    onGeometry(adjustWindow(rect,size,action,direction[0]*WINDOW_KEY_STEP,direction[1]*WINDOW_KEY_STEP));
  }
  function handle(action:WindowAction,title:string,text:string){
    return <button type="button" className="studio-window-handle" aria-label={title} title={title+" · drag or use arrow keys"}
      onPointerDown={e=>start(e,action)} onPointerMove={move} onPointerUp={e=>finish(e,true)}
      onPointerCancel={e=>finish(e,false)} onLostPointerCapture={e=>finish(e,false)} onKeyDown={e=>key(e,action)}>{text}</button>;
  }
  return <section className="studio-pinned studio-instrument-window" aria-label={label} data-floating={!!geometry} data-active={active}
    style={rect?{left:rect.x,top:rect.y,width:rect.width,height:rect.height}:undefined}
    onPointerDownCapture={onActivate} onFocusCapture={onActivate} onDragOver={onDragOver} onDrop={onDrop}>
    <header className="studio-subbar studio-window-toolbar">
      {geometry&&handle("move","Move floating instrument","↔")}{toolbar}{geometry&&handle("resize","Resize floating instrument","↘")}
    </header>
    <div className="studio-window-content">{children}</div>
  </section>;
}
