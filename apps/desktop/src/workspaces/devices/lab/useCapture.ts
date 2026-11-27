import { useEffect,useRef,useState } from "react";
import { request,seg } from "../../../engine/http";
import { BATCH_MS,MAX_ROWS,MAX_DEVICES,parseBatch,continuesBatch,type Capture,type Sample } from "./captureTypes";

export function useCapture(live:boolean) {
  const [running,setRunning]=useState(false),[elapsed,setElapsed]=useState(0),[revision,setRevision]=useState(0);
  const data=useRef<Capture|null>(null);
  const controller=useRef<AbortController|null>(null);
  const latest=useRef(0);
  const stopRef=useRef<(reason:string)=>void>(()=>{});
  function stop(reason="Stopped by operator"){stopRef.current(reason);}
  useEffect(()=>{if(!live)stopRef.current("Engine left live mode");},[live]);
  useEffect(()=>()=>{controller.current?.abort();},[]);
  function start(ids:string[],seconds:number) {
    if(!live||running||!ids.length||ids.length>MAX_DEVICES||new Set(ids).size!==ids.length||(controller.current&&!controller.current.signal.aborted))return;
    const ctl=new AbortController();controller.current=ctl;
    const capture:Capture={samples:[],markers:[],lost:0,resets:0,started:new Date().toISOString(),reason:"Capturing"};
    data.current=capture;latest.current=0;setRunning(true);setElapsed(0);setRevision(v=>v+1);
    const started=performance.now();
    const cursors=new Map<string,{epoch:number;next:number}>();
    let timer=0;
    ctl.signal.addEventListener("abort",()=>window.clearTimeout(timer),{once:true});
    const finish=(reason:string)=>{
      if(ctl.signal.aborted)return;
      ctl.abort();window.clearTimeout(timer);capture.reason=reason;setRunning(false);setRevision(v=>v+1);
    };
    stopRef.current=finish;
    async function poll(){
      if(ctl.signal.aborted)return;
      const results=await Promise.all(ids.map(async id=>{
        const cursor=cursors.get(id);
        const suffix=cursor?`?epoch=${cursor.epoch}&after=${cursor.next}`:"";
        const res=await request<unknown>("GET",`/devices/${seg(id)}/inspection/batch${suffix}`,undefined,(url,init)=>fetch(url,{...init,signal:AbortSignal.any([ctl.signal,AbortSignal.timeout(3000)])}));
        return {id,batch:res.ok?parseBatch(res.data):null,error:res.ok?"Invalid diagnostic response":res.error.message};
      }));
      if(ctl.signal.aborted)return;
      for(const {id,batch,error} of results){
        if(!batch){finish(id+": "+error);return;}
        const previous=cursors.get(id);
        if(previous&&!continuesBatch(previous,batch)){
          if(batch.reset||batch.epoch!==previous.epoch){capture.resets++;capture.markers.push({t_us:latest.current,label:"Stream reference / lease reset",device:id});finish("Stream reset; restart capture for a continuous time base");}
          else finish(id+": inconsistent journal cursor; recording stopped");
          return;
        }
        if(!previous&&(batch.reset||batch.entries.length||batch.lost)){finish(id+": invalid initial journal cursor");return;}
        if(batch.lost){capture.lost+=batch.lost;capture.markers.push({t_us:batch.entries[0]?.point.t_us??latest.current,label:`Journal overrun: ${batch.lost} observations`,device:id});}
        cursors.set(id,{epoch:batch.epoch,next:batch.next});
        for(const entry of batch.entries){
          if(capture.samples.length>=MAX_ROWS){finish("Browser capture capacity reached");return;}
          const sample:Sample={...entry,device:id,epoch:batch.epoch};
          capture.samples.push(sample);latest.current=Math.max(latest.current,entry.point.t_us);
        }
      }
      const age=(performance.now()-started)/1000;setElapsed(age);setRevision(v=>v+1);
      if(age>=seconds){finish("Duration completed");return;}
      timer=window.setTimeout(()=>void poll(),BATCH_MS);
    }
    void poll();
  }
  function mark(label:string){if(running&&data.current&&latest.current>0){data.current.markers.push({t_us:latest.current,label});setRevision(v=>v+1);}}
  return {capture:data.current,running,elapsed,revision,start,stop,mark};
}
