import { useState } from "react";
import type { SeriesResponse, SystemState } from "@contracts";
import { contentCache, useQuery } from "../../engine/queries";
import { paths } from "../../engine/trainingActions";
import { formatValue } from "../../lib/training";
import { Chart, SERIES_COLORS, type Marker, type Series } from "../../ui/chart/Chart";
import { EmptyState } from "../../ui/EmptyState";
import { OriginBadge } from "../../ui/OriginBadge";
import { Select } from "../../ui/Select";
import { CONTENT_ID, formatUtc, nameOf, progressOrigin, useContentIndex } from "../train/common";
import styles from "../train/train.module.css";

const CHANGE: Record<string, string> = {
  increased: "increased",
  decreased: "decreased",
  no_clear_change: "no clear change",
};

/** `#/progress` and `#/progress/<technique_id>/<metric_id>` (10 §3, §6.5). */
export function Progress({ state, rest }: { state: SystemState | null; rest: string[] }) {
  const index = useContentIndex();
  const athlete = state?.athlete?.id ?? null;
  const origin = progressOrigin(state);
  const techniques = index?.entries.filter((e) => e.kind === "techniques" && e.valid) ?? [];
  const routeTechnique = rest[0] && CONTENT_ID.test(rest[0]) ? rest[0] : null;
  const routeMetric = rest[1] && CONTENT_ID.test(rest[1]) ? rest[1] : null;
  const technique = routeTechnique ?? techniques[0]?.id ?? null;
  const definition = useQuery<{ metrics?: { use: string }[] }>(
    technique ? `/content/techniques/${technique}` : null,
    contentCache,
  );
  const metrics = definition.data?.metrics?.map((m) => m.use) ?? [];
  const metric = routeMetric ?? metrics[0] ?? null;
  const [side, setSide] = useState("right");
  const series = useQuery<SeriesResponse>(
    athlete && technique && metric ? paths.series({ athlete_id: athlete, technique, metric, side, origin }) : null,
  );
  const go = (t: string | null, m: string | null) => {
    location.hash = `#/progress/${t ?? ""}${m ? `/${m}` : ""}`;
  };
  const data = series.data;
  const unit = data?.unit ?? "";
  const lines: Series[] = [];
  const markers: Marker[] = [];
  data?.classes.forEach((c, i) => {
    const color = SERIES_COLORS[i % SERIES_COLORS.length];
    const label = `${c.method_class} · ${c.detector_id}`;
    for (const basis of ["A", "B"] as const) {
      const pts = c.points.filter((p) => p.basis === basis);
      if (pts.length)
        lines.push({
          id: `${c.comparability_class}-${basis}`,
          label: `${label}${basis === "B" ? " (grade B)" : ""}`,
          color,
          secondary: basis === "B",
          points: pts.map((p) => [Date.parse(p.date_utc), Number(formatValue(p.value, unit).text)] as const),
        });
    }
    for (const b of c.breaks) {
      const p = c.points.find((x) => x.session_id === b.session_id);
      if (p) markers.push({ x: Date.parse(p.date_utc), label: `Series break: ${b.reason}`, kind: "break" });
    }
  });
  const display = formatValue(1, unit).unit;
  return (
    <div className="workspace-page">
      <header className="workspace-heading">
        <div>
          <span className="eyebrow">Workspace / Progress</span>
          <h1>Progress</h1>
        </div>
        <div className="workspace-tools">{origin && <OriginBadge label={origin} />}</div>
      </header>
      <div className={styles.page}>
      {!athlete ? (
        <EmptyState title="No active athlete">
          <a href="#/train">Choose an athlete in Train</a>
        </EmptyState>
      ) : (
        <div className={styles.stack}>
          <div className={styles.row}>
            <Select label="Technique" value={technique ?? ""} options={techniques.map((t) => ({ value: t.id, label: t.names.display }))} onChange={(e) => go(e.target.value, null)} />
            <Select label="Metric" value={metric ?? ""} options={metrics.map((m) => ({ value: m, label: nameOf(index, m) }))} onChange={(e) => go(technique, e.target.value)} />
            <Select
              label="Side"
              value={side}
              options={[
                { value: "right", label: "Right" },
                { value: "left", label: "Left" },
              ]}
              onChange={(e) => setSide(e.target.value)}
            />
          </div>
          {origin && <p className={styles.muted}>This rig is simulated: simulated sessions are included and marked.</p>}
          {series.error && <p className="error" role="alert">{series.error}</p>}
          {data && data.classes.length === 0 && (
            <EmptyState title="No sessions for this metric yet">
              <p>Values appear after a session with grade A or B measurements of this metric.</p>
            </EmptyState>
          )}
          {data && data.classes.length > 0 && (
            <>
              <section className={styles.card} aria-label="Series">
                <h2>
                  {nameOf(index, metric)} · {display} · one line per comparability class
                </h2>
                <Chart
                  label={`${nameOf(index, metric)} per session`}
                  series={lines}
                  markers={markers}
                  height={220}
                  xFormat={(v) => new Date(v).toISOString().slice(5, 16).replace("T", " ")}
                  yFormat={(v) => v.toFixed(1)}
                />
              </section>
              {data.classes.map((c) => (
                <section key={c.comparability_class} className={styles.card} aria-label={`Class ${c.method_class} ${c.detector_id}`} data-testid="series-class">
                  <h2>
                    {c.method_class} · {c.detector_id}
                  </h2>
                  <p>
                    {c.classification
                      ? `Trend: ${CHANGE[c.classification.result] ?? c.classification.result} (change ${formatValue(c.classification.delta, unit).text} ${display}, noise ${formatValue(c.classification.noise, unit).text} ${display})`
                      : "Trend: not enough sessions to judge."}
                  </p>
                  <table className={styles.table}>
                    <thead>
                      <tr>
                        <th>Session</th>
                        <th>Median</th>
                        <th>Spread</th>
                        <th>Values</th>
                        <th>Basis</th>
                        <th>Tracking</th>
                      </tr>
                    </thead>
                    <tbody>
                      {c.points.map((p) => (
                        <tr key={p.session_id + p.basis}>
                          <td>
                            <a href={`#/train/summary/${p.session_id}`}>{formatUtc(p.date_utc)}</a> {p.origin !== "live" && <OriginBadge label={p.origin} />}
                          </td>
                          <td className="mono">
                            {formatValue(p.value, unit).text} {display}
                          </td>
                          <td className="mono">{formatValue(p.spread, unit).text}</td>
                          <td className="mono">{p.count}</td>
                          <td>{p.basis === "A" ? "Observed (A)" : "Estimated (B)"}</td>
                          <td>
                            {p.capability_profile} · QA {p.calibration_qa}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </section>
              ))}
            </>
          )}
        </div>
      )}
      </div>
    </div>
  );
}
