import { AXIS_COLORS, CHANNELS, rotate, type Channel, type ChannelId } from "./trace";

export function TracePlot({ id, channel, axes }: {
  id: ChannelId; channel: Channel | undefined; axes: number[];
}) {
  const config = CHANNELS[id];
  const points = channel?.points ?? [];
  const last = points.at(-1);
  const end = last?.t_us ?? 0;
  const start = end - 5_000_000;
  const visible = points.filter(p => p.t_us >= start);
  const values = visible.flatMap(p => axes.map(i => p.values[i] ?? 0));
  const low = Math.min(0, ...values), high = Math.max(0, ...values);
  const span = Math.max(high-low, 0.001);
  const y = (v: number) => 88 - (v-low)/span*76;
  return (
    <section className="lab-card">
      <header><h3>{config.title}</h3><span className="subtle">{config.unit}</span></header>
      {!last ? <p className="subtle">No measurements received.</p> : <>
        <div className="lab-values mono">
          {axes.map(i => <span key={i} style={{ color: AXIS_COLORS[i] }}>
            {config.axes[i]} {last.values[i]?.toFixed(3)}
          </span>)}
        </div>
        <svg className="lab-chart" viewBox="0 0 320 110" role="img" aria-label={`${config.title}, five second history`}>
          {[0, 1, 2, 3, 4, 5].map(i => <line key={i} x1={i*64} x2={i*64} y1="12" y2="88" stroke="var(--line)" />)}
          <line x1="0" x2="320" y1={y(0)} y2={y(0)} stroke="var(--ink-3)" strokeDasharray="3 3"/>
          {axes.map(i => {
            let path = "";
            let previous = -Infinity;
            for (const p of visible) {
              path += `${p.t_us-previous>150_000 ? "M" : "L"}${((p.t_us-start)/5_000_000*320).toFixed(2)},${y(p.values[i] ?? 0).toFixed(2)} `;
              previous = p.t_us;
            }
            return <path key={i} d={path} fill="none" stroke={AXIS_COLORS[i]} strokeWidth="1.7" vectorEffect="non-scaling-stroke"/>;
          })}
          <text x="2" y="10" fill="var(--ink-3)" fontSize="9">{high.toFixed(2)}</text>
          <text x="2" y="99" fill="var(--ink-3)" fontSize="9">{low.toFixed(2)}</text>
          <text x="2" y="109" fill="var(--ink-3)" fontSize="9">−5 s</text>
          <text x="318" y="109" textAnchor="end" fill="var(--ink-3)" fontSize="9">latest</text>
        </svg>
        <p className="subtle mono">{points.length} displayed samples · seq {last.seq} · age {channel?.age_ms} ms at capture</p>
      </>}
    </section>
  );
}

const vertices = [[-1,-1,-1],[1,-1,-1],[1,1,-1],[-1,1,-1],[-1,-1,1],[1,-1,1],[1,1,1],[-1,1,1]];
const edges = [[0,1],[1,2],[2,3],[3,0],[4,5],[5,6],[6,7],[7,4],[0,4],[1,5],[2,6],[3,7]];
const project = (v: number[]) => {
  const [x,y,z] = v as [number,number,number];
  return [140 + 39*(x+0.45*z), 100 - 39*(y-0.3*z)] as const;
};

export function OrientationView({ q }: { q: number[] | undefined }) {
  const points = q ? vertices.map(v => project(rotate(v,q))) : [];
  const center = project([0,0,0]);
  return <section className="lab-card">
    <header><h3>Isolated orientation</h3><span className="subtle">Sensor reference</span></header>
    {!q ? <p className="subtle">Waiting for orientation.</p> : <svg viewBox="0 0 280 200" className="lab-orientation" role="img" aria-label="Live quaternion wireframe and sensor axes">
      {[1,2,3,4,5,6,7].map(i => <line key={i} x1={i*35} y1="0" x2={i*35} y2="200" stroke="var(--line)" opacity=".4"/>)}
      {[1,2,3,4,5].map(i => <line key={i} x1="0" y1={i*35} x2="280" y2={i*35} stroke="var(--line)" opacity=".4"/>)}
      {edges.map(([a,b],i) => <line key={i} x1={points[a!]![0]} y1={points[a!]![1]} x2={points[b!]![0]} y2={points[b!]![1]} stroke="var(--ink-2)" strokeWidth="1.5"/>)}
      {[[1.8,0,0],[0,1.8,0],[0,0,1.8]].map((v,i) => {
        const p = project(rotate(v,q));
        return <g key={i}><line x1={center[0]} y1={center[1]} x2={p[0]} y2={p[1]} stroke={AXIS_COLORS[i]} strokeWidth="3"/><text x={p[0]+4} y={p[1]-4} fill={AXIS_COLORS[i]} fontSize="12">{"XYZ"[i]}</text></g>;
      })}
    </svg>}
    <p className="subtle">Unmounted observation; not the fused body segment.</p>
  </section>;
}
