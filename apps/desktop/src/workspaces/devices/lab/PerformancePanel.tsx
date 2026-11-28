import {useDiagnosticEndpoint} from "./useDiagnosticEndpoint";
import {parsePerformance,type ResourceSample} from "./performanceTypes";
import {downloadText} from "./signalMath";
const fmt=(v:number|null)=>v===null?"—":v.toFixed(1);
function Graph({samples,metric,label,scale=1}:{samples:ResourceSample[];metric:"cpu_percent_one_core"|"resident_bytes";label:string;scale?:number}){
  const values=samples.map(s=>s[metric]===null?null:s[metric]!/scale),valid=values.filter((v):v is number=>v!==null);
  if(!valid.length)return <p>{label}: unavailable or collecting baseline.</p>;
  const max=Math.max(1,...valid),first=samples[0]!.t_us,span=Math.max(1,samples.at(-1)!.t_us-first);
  const path=samples.map((s,i)=>values[i]===null?"":(i&&values[i-1]!==null&&s.t_us-samples[i-1]!.t_us<=6_000_000?"L":"M")+`${(s.t_us-first)/span*320},${95-values[i]!/max*85}`).join(" ");
  return <div><h4>{label}</h4><p className="mono">{fmt(values.at(-1)??null)} · max {fmt(max)}</p><svg viewBox="0 0 320 105" className="lab-chart" role="img" aria-label={label+" history"}><path d="M0 95H320 M0 10H320" stroke="var(--line)"/><path d={path} stroke="var(--accent)" strokeWidth="1.5" fill="none"/></svg></div>;
}
export function PerformancePanel({live}:{live:boolean}){
  const {data,error}=useDiagnosticEndpoint("/inspection/performance",live,parsePerformance);
  return <section className="lab-card"><header><h3>Engine performance</h3><button disabled={!data?.samples.length} onClick={()=>data&&downloadText("bansayo-performance.json",JSON.stringify({format:"bansayo-performance-v1",...data},null,2),"application/json")}>Export performance report</button></header>
    {!live?<p>Live engine required.</p>:error?<p role="alert">{error}</p>:!data?<p>Waiting for engine diagnostics.</p>:!data.enabled?<p>Enable diagnostics.resources in schema version 2 and restart the engine. Enable diagnostics.timing for stage timings.</p>:<>
      <p className="subtle">Process CPU and resident memory, sampled every two seconds. CPU is relative to one logical core and may exceed 100%. Not system-wide load.</p>
      <div className="lab-grid"><Graph samples={data.samples} metric="cpu_percent_one_core" label="Engine CPU (%)"/><Graph samples={data.samples} metric="resident_bytes" label="Engine resident memory (MiB)" scale={1048576}/></div>
      <h4>Pipeline stage latency · last 10 seconds</h4>
      {!data.timing_enabled?<p>Stage timing disabled in configuration.</p>:!data.stages.length?<p>No recent stage measurements.</p>:<div className="studio-stats"><table><thead><tr><th>Stage</th><th>P50 ms</th><th>P95 ms</th><th>P99 ms</th><th>Max ms</th><th>Budget ms</th><th>Samples</th></tr></thead><tbody>{data.stages.map(s=><tr key={s.stage}><td>{s.stage}</td><td>{fmt(s.p50_us/1000)}</td><td className={s.p95_us>s.budget_us?"lab-result-warn":undefined}>{fmt(s.p95_us/1000)}</td><td>{fmt(s.p99_us/1000)}</td><td>{fmt(s.max_us/1000)}</td><td>{fmt(s.budget_us/1000)}</td><td>{s.count}</td></tr>)}</tbody></table></div>}
      <p className="subtle">GPU utilization and VRAM are not measured by this endpoint; inference timing is not a GPU counter.</p>
    </>}
  </section>;
}
