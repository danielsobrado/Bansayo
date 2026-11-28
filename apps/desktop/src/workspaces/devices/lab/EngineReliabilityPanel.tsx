import {useState} from "react";
import {useDiagnosticEndpoint} from "./useDiagnosticEndpoint";
import {parseEngineReliability} from "./performanceTypes";
import {downloadText} from "./signalMath";
export function EngineReliabilityPanel({live}:{live:boolean}){
  const [before,setBefore]=useState<number|null>(null);
  const {data,error}=useDiagnosticEndpoint("/inspection/reliability"+(before===null?"":"?before_ms="+before),live,parseEngineReliability,before===null?10000:0);
  const [choice,setChoice]=useState("");
  const ids=[...new Set(data?.rows.flatMap(r=>r.devices.map(d=>String(d.device)))??[])];
  const id=ids.includes(choice)?choice:ids[0]??"";
  return <section className="lab-card"><header><h3>Engine-owned reliability archive</h3></header>
    <p className="subtle">Opt-in engine snapshots independent of Sensor Studio. Five-second cadence, up to 20,000 snapshots or seven days; the latest 120 snapshots per page. No motion or camera image data.</p>
    {!live?<p>Connect to the engine to read the archive.</p>:error?<p role="alert">{error}</p>:!data?<p>Loading archive…</p>:<>
      {!data.enabled&&<p>Recording disabled. Enable diagnostics.reliability in schema version 2 to collect new evidence.</p>}
      <div className="studio-controls"><label>Archive device <select value={id} onChange={e=>setChoice(e.target.value)}>{ids.map(d=><option key={d}>{d}</option>)}</select></label><button disabled={!data.rows.length} onClick={()=>downloadText("bansayo-engine-reliability.json",JSON.stringify({format:"bansayo-engine-reliability-v1",...data},null,2),"application/json")}>Export archive page</button><button disabled={!data.rows.length} onClick={()=>setBefore(data.rows[0]!.at_ms)}>Older snapshots</button><button disabled={before===null} onClick={()=>setBefore(null)}>Latest snapshots</button></div>
      <p>{data.rows.length} snapshots on this page</p>
      <div className="studio-stats"><table><thead><tr><th>UTC</th><th>Origin</th><th>State</th><th>Rate Hz</th><th>Received</th><th>Dropped</th></tr></thead><tbody>{data.rows.toReversed().flatMap((r,i)=>r.devices.filter(d=>d.device===id).map(d=>{const stream=d.stream as Record<string,number>;return <tr key={r.epoch+":"+r.at_ms+":"+i}><td>{new Date(r.at_ms).toISOString()}</td><td>{String(d.origin)}</td><td>{String(d.state)}</td><td>{stream.measured_rate_hz?.toFixed(1)}</td><td>{stream.received}</td><td>{stream.dropped}</td></tr>;}))}</tbody></table></div>
    </>}
  </section>;
}
