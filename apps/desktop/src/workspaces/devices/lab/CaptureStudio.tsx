import { useState } from "react";
import type { DeviceStatus } from "@contracts";
import { CAPTURE_SECONDS,MAX_DEVICES,MAX_ROWS } from "./captureTypes";
import { PersistentRecording } from "./PersistentRecording";
import { useCapture } from "./useCapture";
import { Timeline } from "./Timeline";
import { AnalysisPanel } from "./AnalysisPanel";
import { downloadText } from "./signalMath";
export function CaptureStudio({devices,live}:{devices:DeviceStatus[];live:boolean}) {
  const [ids,setIds]=useState<string[]>([]),[seconds,setSeconds]=useState<number>(60),[label,setLabel]=useState("Impact");
  const capture=useCapture(live);
  const eligible=devices.filter(d=>d.descriptor.kind!=="camera");
  const selected=ids.filter(id=>eligible.some(d=>d.descriptor.id===id));
  const record=capture.capture;
  return <div className="lab-main"><PersistentRecording devices={devices} live={live}/><section className="lab-card"><header><h3>Full-rate diagnostic capture</h3><span className={capture.running?"lab-result-warn":"subtle"}>{capture.running?"Recording":record?.reason??"Idle"}</span></header>
    <p className="subtle">Every accepted supported sensor observation, not raw hardware FIFO data. Up to {MAX_DEVICES} devices and {MAX_ROWS.toLocaleString()} observations in browser memory. Keep the page open; loss and reference resets are reported.</p>
    <div className="lab-bench-controls">{eligible.map(d=><label key={d.descriptor.id}><input type="checkbox" checked={selected.includes(d.descriptor.id)} disabled={capture.running||(!selected.includes(d.descriptor.id)&&selected.length>=MAX_DEVICES)} onChange={e=>setIds(v=>e.target.checked?[...v,d.descriptor.id]:v.filter(id=>id!==d.descriptor.id))}/>{d.descriptor.id}</label>)}</div>
    <div className="studio-controls"><label>Duration <select value={seconds} disabled={capture.running} onChange={e=>setSeconds(Number(e.target.value))}>{CAPTURE_SECONDS.map(v=><option key={v} value={v}>{v} s</option>)}</select></label><button disabled={!live||capture.running||!selected.length} onClick={()=>capture.start(selected,seconds)}>Start full-rate capture</button><button disabled={!capture.running} onClick={()=>capture.stop()}>Stop capture</button><button disabled={!record||capture.running} onClick={()=>record&&downloadText("bansayo-full-rate.json",JSON.stringify({format:"bansayo-diagnostics-v1",...record}),"application/json")}>Export recording</button></div>
    {record&&<><progress max={seconds} value={Math.min(seconds,capture.elapsed)}/><p className="mono">{record.samples.length.toLocaleString()} observations · {record.lost} journal losses · {record.resets} resets · {capture.elapsed.toFixed(1)} s</p></>}
    <div className="studio-controls"><input aria-label="Event marker label" maxLength={80} value={label} onChange={e=>setLabel(e.target.value)}/><button disabled={!capture.running||!label.trim()} onClick={()=>capture.mark(label.trim())}>Add event marker</button></div>
  </section>{record&&<><Timeline capture={record}/>{!capture.running&&<AnalysisPanel capture={record}/>}</>}</div>;
}
