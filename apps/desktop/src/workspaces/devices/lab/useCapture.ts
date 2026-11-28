import { useEffect,useRef,useState } from "react";
import {startOpticalCapture} from "./opticalCapture";
import type {OpticalSelection} from "./opticalSyncConfig";
import { request,seg } from "../../../engine/http";
import { BATCH_MS,CAPTURE_DRAIN_MS,MAX_ROWS,MAX_DEVICES,parseBatch,continuesBatch,parseClock,type Capture,type Sample,type EngineClock } from "./captureTypes";

export function useCapture(live:boolean) {
  const [running,setRunning]=useState(false),[stopping,setStopping]=useState(false),[elapsed,setElapsed]=useState(0),[revision,setRevision]=useState(0);
  const data=useRef<Capture|null>(null);
  const controller=useRef<AbortController|null>(null);
  const latest=useRef(0);
  const stopRef=useRef<(reason:string)=>void>(()=>{});
  function stop(reason="Stopped by operator"){stopRef.current(reason);}
  useEffect(()=>{if(!live)stopRef.current("Engine left live mode");},[live]);
  useEffect(()=>()=>{controller.current?.abort();},[]);
  function start(ids:string[],seconds:number,optical?:OpticalSelection) {
    if(!live||running||!ids.length||ids.length>MAX_DEVICES||new Set(ids).size!==ids.length||(controller.current&&!controller.current.signal.aborted))return;
    const ctl=new AbortController();controller.current=ctl;
    const capture:Capture={samples:[],markers:[],lost:0,resets:0,started:new Date().toISOString(),reason:"Capturing"};
    if(optical)capture.optical=startOpticalCapture(optical,ctl.signal);
    data.current=capture;latest.current=0;setRunning(true);setStopping(false);setElapsed(0);setRevision(v=>v+1);
    let clockStart:EngineClock|null=null,cutoff:number|null=null,lastClock:number|null=null;
    let excluded=0;
    const cursors=new Map<string,{epoch:number;next:number}>();
    let timer=0,inFlight=false,operatorStop=false;
    ctl.signal.addEventListener("abort",()=>window.clearTimeout(timer),{once:true});
    const finish=(reason:string)=>{
      if(ctl.signal.aborted)return;
      if(clockStart&&lastClock!==null){
        const end=Math.max(clockStart.t_us,Math.min(cutoff??lastClock,lastClock));
        const count=capture.samples.length;
        capture.samples=capture.samples.filter(s=>s.point.t_us>=clockStart!.t_us&&s.point.t_us<end);
        capture.window={start_us:clockStart.t_us,end_us:end,excluded:excluded+count-capture.samples.length};
        if(capture.optical)capture.optical.frames=capture.optical.frames.filter(p=>p.t_us>=clockStart!.t_us&&p.t_us<end);
      }
      ctl.abort();window.clearTimeout(timer);capture.reason=reason;setRunning(false);setStopping(false);setRevision(v=>v+1);
    };
    async function readClock(){
      const res=await request<unknown>("GET","/inspection/clock",undefined,(url,init)=>fetch(url,{...init,cache:"no-store",signal:AbortSignal.any([ctl.signal,AbortSignal.timeout(3000)])}));
      if(ctl.signal.aborted)return null;
      const value=res.ok?parseClock(res.data):null;
      if(!value){finish(res.ok?"Invalid engine clock":res.error.message);return null;}
      if(clockStart&&(clockStart.session_epoch_utc!==value.session_epoch_utc||(lastClock!==null&&value.t_us<lastClock))){
        capture.resets++;finish("Engine clock changed; restart capture");return null;
      }
      lastClock=value.t_us;
      return value;
    }
    stopRef.current=(reason)=>{
      if(reason!=="Stopped by operator"){finish(reason);return;}
      if(ctl.signal.aborted||operatorStop)return;
      operatorStop=true;setStopping(true);
      if(!inFlight){window.clearTimeout(timer);void poll();}
    };
    async function poll(){
      if(ctl.signal.aborted||inFlight)return;
      inFlight=true;
      const initial=cursors.size===0;
      const finalPoll=cutoff!==null;
      const results=await Promise.all(ids.map(async id=>{
        const cursor=cursors.get(id);
        const suffix=cursor?`?epoch=${cursor.epoch}&after=${cursor.next}`:"";
        const res=await request<unknown>("GET",`/devices/${seg(id)}/inspection/batch${suffix}`,undefined,(url,init)=>fetch(url,{...init,cache:"no-store",signal:AbortSignal.any([ctl.signal,AbortSignal.timeout(3000)])}));
        return {id,batch:res.ok?parseBatch(res.data):null,error:res.ok?"Invalid diagnostic response":res.error.message};
      }));
      if(ctl.signal.aborted)return;
      let backlog=false;
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
        backlog ||= batch.entries.length===4096;
        for(const entry of batch.entries){
          if(clockStart&&entry.point.t_us<clockStart.t_us){excluded++;continue;}
          if(cutoff!==null&&entry.point.t_us>=cutoff){excluded++;continue;}
          if(capture.samples.length>=MAX_ROWS){finish("Browser capture capacity reached");return;}
          const sample:Sample={...entry,device:id,epoch:batch.epoch};
          capture.samples.push(sample);latest.current=Math.max(latest.current,entry.point.t_us);
        }
      }
      const clock=await readClock();
      inFlight=false;
      if(!clock||ctl.signal.aborted)return;
      if(initial)clockStart=clock;
      if(!clockStart){finish("Missing capture clock");return;}
      const scheduled=clockStart.t_us+seconds*1e6;
      if(cutoff===null&&(operatorStop||clock.t_us>=scheduled)){
        cutoff=Math.min(scheduled,clock.t_us);setStopping(true);
      }
      const age=(Math.min(clock.t_us,cutoff??scheduled)-clockStart.t_us)/1e6;
      setElapsed(age);setRevision(v=>v+1);
      if(finalPoll&&cutoff!==null&&clock.t_us>=cutoff+CAPTURE_DRAIN_MS*1000){
        const reason=operatorStop?"Stopped by operator":"Duration completed";
        finish(backlog?reason+" (journal backlog may remain)":reason);return;
      }
      timer=window.setTimeout(()=>void poll(),backlog?0:BATCH_MS);
    }
    void poll();
  }
  function mark(label:string){if(running&&data.current&&latest.current>0){data.current.markers.push({t_us:latest.current,label});setRevision(v=>v+1);}}
  return {capture:data.current,running,stopping,elapsed,revision,start,stop,mark};
}
