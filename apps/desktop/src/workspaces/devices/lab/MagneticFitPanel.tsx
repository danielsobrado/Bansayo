import { useMemo,useState } from "react";
import type { Capture } from "./captureTypes";
import { fitMagnetic } from "./magneticFit";
import { downloadText } from "./signalMath";
export function MagneticFitPanel({capture,device}:{capture:Capture;device:string}) {
  const [sensor,setSensor]=useState<"mag0"|"mag1">("mag0");
  const points=useMemo(()=>capture.samples.filter(s=>s.device===device&&s.channel===sensor).map(s=>s.point.values),[capture,device,sensor]);
  const result=useMemo(()=>fitMagnetic(points),[points]);
  return <section className="lab-card"><header><h3>Magnetic ellipsoid calibration</h3><select aria-label="Magnetometer" value={sensor} onChange={e=>setSensor(e.target.value as typeof sensor)}><option value="mag0">BMM150 #0</option><option value="mag1">BMM150 #1</option></select></header><p className="subtle">Rotate the device through many orientations away from metal. Fits a full symmetric quadratic ellipsoid in reported BSP units; correction is diagnostic and never written to firmware.</p>
    {result?<><p className="mono">Hard-iron offset: {result.center.map(v=>v.toFixed(3)).join(" / ")}</p><p className="mono">Normalized radius RMS error: {(result.rms*100).toFixed(2)}% · {result.count} samples</p><table className="studio-stats"><thead><tr><th colSpan={3}>Unit-sphere correction matrix</th></tr></thead><tbody>{result.matrix.map((row,i)=><tr key={i}>{row.map((v,j)=><td key={j}>{v.toFixed(5)}</td>)}</tr>)}</tbody></table>
      <div className="studio-scatter-grid">{[[0,1],[0,2],[1,2]].map(([a,b])=><svg key={a+":"+b} viewBox="0 0 120 120" role="img" aria-label="Corrected magnetic projection"><circle cx="60" cy="60" r="45" stroke="var(--line)" fill="none"/>{result.corrected.map((p,i)=><circle key={i} cx={60+p[a!]!*45} cy={60-p[b!]!*45} r="1.3" fill="var(--accent)" opacity=".6"/>)}</svg>)}</div>
      <button onClick={()=>downloadText(device+"-"+sensor+"-fit.json",JSON.stringify({device,sensor,...result,advisory:true},null,2),"application/json")}>Export fit</button></>:<p>Fit unavailable: need sufficient diverse 3D measurements and a positive-definite, well-conditioned ellipsoid.</p>}
  </section>;
}
