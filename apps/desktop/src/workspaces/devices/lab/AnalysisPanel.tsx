import { useMemo,useState } from "react";
import { CHANNELS,type ChannelId } from "./trace";
import type { Capture } from "./captureTypes";
import { allan,spectralAnalysis,uniformRun } from "./analysisMath";
import { MagneticFitPanel } from "./MagneticFitPanel";
import {StationaryPanel} from "./StationaryPanel";
import {OpticalSyncPanel} from "./OpticalSyncPanel";
import { SynchronizationPanel } from "./SynchronizationPanel";
function Plot({data,label,log=false}:{data:{x:number;y:number}[];label:string;log?:boolean}) {
  const valid=data.filter(p=>Number.isFinite(p.y)&&(!log||(p.x>0&&p.y>0)));
  if(!valid.length)return <p>No positive finite analysis points.</p>;
  const points=valid.map(p=>({x:log?Math.log10(p.x):p.x,y:log?Math.log10(p.y):p.y}));
  const minX=Math.min(...points.map(p=>p.x)),maxX=Math.max(...points.map(p=>p.x)),minY=log?Math.min(...points.map(p=>p.y)):0,maxY=Math.max(...points.map(p=>p.y));
  return <div><svg viewBox="0 0 360 140" className="lab-chart" role="img" aria-label={label}>
    {[20,45,70,95,120].map(y=><line key={y} x1="35" x2="350" y1={y} y2={y} stroke="var(--line)"/>)}
    <polyline points={points.map(p=>`${35+(p.x-minX)/Math.max(1e-9,maxX-minX)*315},${120-(p.y-minY)/Math.max(1e-9,maxY-minY)*100}`).join(" ")} fill="none" stroke="var(--accent)" strokeWidth="1.5"/>
    <text x="35" y="136" fontSize="10" fill="var(--ink-3)">{valid[0]!.x.toPrecision(3)}</text><text x="350" y="136" fontSize="10" textAnchor="end" fill="var(--ink-3)">{valid.at(-1)!.x.toPrecision(3)}</text>
    <text x="30" y="23" fontSize="9" textAnchor="end" fill="var(--ink-3)">{(log?10**maxY:maxY).toPrecision(2)}</text><text x="30" y="120" fontSize="9" textAnchor="end" fill="var(--ink-3)">{(log?10**minY:minY).toPrecision(2)}</text>
  </svg><small className="subtle">{log?"Log–log scale":"Linear scale"}</small></div>;
}
export function AnalysisPanel({capture}:{capture:Capture}) {
  const [device,setDevice]=useState(""),[channel,setChannel]=useState<ChannelId>("gyro"),[axis,setAxis]=useState(0);
  const devices=useMemo(()=>[...new Set(capture.samples.map(s=>s.device))],[capture]);
  const id=devices.includes(device)?device:devices[0]??"";
  const points=useMemo(()=>capture.samples.filter(s=>s.device===id&&s.channel===channel).map(s=>s.point),[capture,id,channel]);
  const run=useMemo(()=>uniformRun(points,axis),[points,axis]);
  const spectral=useMemo(()=>run?spectralAnalysis(run):null,[run]);
  const fft=spectral?.amplitude??[];
  const av=useMemo(()=>run?allan(run):[],[run]);
  const peak=fft.slice(1).reduce<{x:number;y:number}|null>((best,p)=>!best||p.y>best.y?p:best,null);
  return <><section className="lab-card"><header><h3>Signal analysis</h3></header><div className="studio-subbar"><label>Device <select value={id} onChange={e=>setDevice(e.target.value)}>{devices.map(d=><option key={d}>{d}</option>)}</select></label><label>Channel <select value={channel} onChange={e=>{setChannel(e.target.value as ChannelId);setAxis(0);}}>{(["gyro","accel"] as const).map(c=><option key={c} value={c}>{CHANNELS[c].title}</option>)}</select></label><label>Axis <select value={axis} onChange={e=>setAxis(Number(e.target.value))}>{["X","Y","Z"].map((v,i)=><option key={v} value={i}>{v}</option>)}</select></label></div>
    {run?<><p className="mono">{run.values.length} contiguous samples · {run.hz.toFixed(2)} Hz estimated · {peak?`Peak bin ${peak.x.toFixed(2)} Hz`:"No spectral peak"}</p><div className="lab-grid"><div><h3>FFT amplitude · Hz</h3><Plot data={fft} label="FFT amplitude spectrum"/></div><div><h3>Power spectral density · ({CHANNELS[channel].unit})²/Hz</h3><Plot data={spectral?.density??[]} label="Power spectral density"/></div><div><h3>Overlapping Allan deviation · seconds</h3><Plot data={av} label="Allan deviation" log/></div></div><p className="subtle">Hann-windowed, mean removed, up to 4096 latest samples. Single-window one-sided periodogram, not Welch averaging. Frequency resolution {spectral?.resolutionHz.toPrecision(3)} Hz. Allan deviation uses up to 32768 contiguous samples; stationary data only for noise characterization. Timestamp variation above 20% breaks the run. Advisory, not metrology.</p></>:<p>Need at least 32 approximately evenly spaced contiguous samples.</p>}
  </section><MagneticFitPanel capture={capture} device={id}/><SynchronizationPanel capture={capture}/><StationaryPanel capture={capture}/><OpticalSyncPanel capture={capture}/></>;
}
