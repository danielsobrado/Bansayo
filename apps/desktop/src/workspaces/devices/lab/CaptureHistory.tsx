import { useEffect,useRef,useState } from "react";

export interface CapturePoint { t_us:number;value:number;skew_ms:number; }
interface RecordData { samples:CapturePoint[]; gaps:number; started:string; duration_s:number; }
const MAX_SECONDS=600;
const MAX_SAMPLES=3000;
const SAMPLE_INTERVAL_US=200_000;

export function CaptureHistory({label,enabled,point}:{label:string;enabled:boolean;point:CapturePoint|null}) {
  const [seconds,setSeconds]=useState(60);
  const [record,setRecord]=useState<RecordData|null>(null);
  const [running,setRunning]=useState(false);
  const [elapsed,setElapsed]=useState(0);
  const current=useRef(point);current.current=point;
  useEffect(()=>{
    if(!running)return;
    const started=performance.now();
    let last=-Infinity;
    const timer=window.setInterval(()=>{
      const age=(performance.now()-started)/1000;
      setElapsed(Math.min(age,seconds));
      if(age>=seconds){setRunning(false);return;}
      const p=current.current;
      if(document.hidden||!enabled||!p||p.t_us<=last){setRecord(r=>r?{...r,gaps:r.gaps+1}:r);return;}
      if(p.t_us-last<SAMPLE_INTERVAL_US)return;
      last=p.t_us;
      setRecord(r=>r?{...r,samples:[...r.samples,p].slice(-MAX_SAMPLES)}:r);
    },500);
    return()=>window.clearInterval(timer);
  },[running,seconds,enabled]);
  useEffect(()=>{
    if(!enabled&&running)setRunning(false);
  },[enabled,running]);
  const samples=record?.samples??[];
  const first=samples[0],last=samples.at(-1);
  const span=Math.max(1,(last?.t_us??0)-(first?.t_us??0));
  const peak=Math.max(1,...samples.map(p=>p.value));
  function exportReport(){
    if(!record)return;
    const blob=new Blob([JSON.stringify({label,...record,elapsed_s:elapsed,unit:"degrees",sampling:"browser 2 Hz, not full-rate",completed:elapsed>=record.duration_s},null,2)],{type:"application/json"});
    const url=URL.createObjectURL(blob),a=document.createElement("a");a.href=url;a.download="bansayo-comparison.json";a.click();setTimeout(()=>URL.revokeObjectURL(url),1000);
  }
  return <section className="lab-card">
    <header><h3>Long-run disagreement capture</h3><span className="mono">{label}</span></header>
    <div className="lab-bench-controls">
      <label>Duration <select disabled={running} value={seconds} onChange={e=>setSeconds(Math.min(MAX_SECONDS,Number(e.target.value)))}><option value="60">1 min</option><option value="300">5 min</option><option value="600">10 min</option></select></label>
      <button disabled={!enabled||!point||running} onClick={()=>{setElapsed(0);setRecord({samples:[],gaps:0,started:new Date().toISOString(),duration_s:seconds});setRunning(true);}}>Start capture</button>
      <button disabled={!running} onClick={()=>setRunning(false)}>Stop</button>
      <button disabled={!record||running} onClick={exportReport}>Export report</button>
    </div>
    {record&&<>
      <progress max={record.duration_s} value={elapsed}/><p className="mono">{elapsed.toFixed(1)} s · {samples.length} pairs · {record.gaps} unavailable/repeated polls</p>
      {samples.length>0&&<><svg viewBox="0 0 320 110" className="lab-chart" role="img" aria-label="Recorded angular disagreement"><polyline points={samples.map(p=>`${(p.t_us-(first?.t_us??p.t_us))/span*320},${100-p.value/peak*90}`).join(" ")} fill="none" stroke="var(--accent)" strokeWidth="1.5"/></svg>
      <p className="mono">Mean {(samples.reduce((s,p)=>s+p.value,0)/samples.length).toFixed(2)}° · max {Math.max(...samples.map(p=>p.value)).toFixed(2)}° · chart 0–{peak.toFixed(1)}°</p></>}
    </>}
    <p className="subtle">Keep this workspace visible during capture. Records only valid paired comparison samples at 2 Hz, including missing-poll count; not the full-rate recorder. Changing device or segment resets the capture.</p>
  </section>;
}
