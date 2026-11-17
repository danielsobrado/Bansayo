import { useEffect, useMemo, useState } from "react";
import type {
  MotionClip,
  RepTrace,
  RepetitionResult,
  SystemState,
  TrainingSessionSummary,
} from "@contracts";
import { useSkeleton } from "../../engine/hooks";
import { useQuery } from "../../engine/queries";
import { paths } from "../../engine/trainingActions";
import { Button } from "../../ui/Button";
import { Chart } from "../../ui/chart/Chart";
import { CueBanner } from "../../ui/CueBanner";
import { EmptyState } from "../../ui/EmptyState";
import { MeasurementValue } from "../../ui/MeasurementValue";
import { OriginBadge } from "../../ui/OriginBadge";
import { PhaseTimeline } from "../../ui/PhaseTimeline";
import { ScoreBars } from "../../ui/ScoreBars";
import { Select } from "../../ui/Select";
import { ClipPlayer } from "../../viewport/ClipPlayer";
import { ViewportCanvas } from "../../viewport/ViewportCanvas";
import { formatUtc, nameOf, SESSION_ID, useContentIndex, useTexts } from "../train/common";
import styles from "../train/train.module.css";

/** `#/technique` and `#/technique/<session_id>/<rep_index>` (10 §3, §6.3). */
export function Technique({ state, rest }: { state: SystemState | null; rest: string[] }) {
  const [sid, rep] = rest;
  if (sid !== undefined) {
    const n = Number(rep);
    return SESSION_ID.test(sid) && Number.isInteger(n) && n > 0 ? (
      <Attempt id={sid} rep={n} />
    ) : (
      <EmptyState title="No such attempt" />
    );
  }
  return <Browser state={state} />;
}

function Heading({ title, children }: { title: string; children?: React.ReactNode }) {
  return (
    <header className="workspace-heading">
      <div>
        <span className="eyebrow">Workspace / Technique</span>
        <h1>{title}</h1>
      </div>
      <div className="workspace-tools">{children}</div>
    </header>
  );
}

function Browser({ state }: { state: SystemState | null }) {
  const athlete = state?.athlete?.id ?? null;
  const sessions = useQuery<TrainingSessionSummary[]>(athlete ? paths.sessions(athlete, 50) : null);
  const [picked, setPicked] = useState<string | null>(null);
  const sid = picked ?? sessions.data?.[0]?.id ?? null;
  const reps = useQuery<RepetitionResult[]>(sid ? paths.reps(sid) : null);
  const [outcome, setOutcome] = useState("all");
  const shown = (reps.data ?? []).filter((r) => outcome === "all" || r.repetition.outcome === outcome);
  return (
    <div className="workspace-page">
      <Heading title="Attempts" />
      <div className={styles.page}>
      {!athlete ? (
        <EmptyState title="No active athlete">
          <a href="#/train">Choose an athlete in Train</a>
        </EmptyState>
      ) : !sessions.data?.length ? (
        <EmptyState title="No stored sessions">
          <p>Run a session in Train; its repetitions appear here.</p>
        </EmptyState>
      ) : (
        <div className={styles.grid}>
          <section className={styles.card} aria-label="Repetitions">
            <div className={styles.row}>
              <Select
                label="Session"
                value={sid ?? ""}
                options={sessions.data.map((s) => ({ value: s.id, label: `${s.title} · ${formatUtc(s.started_utc)}` }))}
                onChange={(e) => setPicked(e.target.value)}
              />
              <Select
                label="Outcome"
                value={outcome}
                options={["all", "completed", "incomplete", "wrong_side", "rejected_quality"].map((o) => ({ value: o, label: o.replace("_", " ") }))}
                onChange={(e) => setOutcome(e.target.value)}
              />
            </div>
            <table className={styles.table}>
              <tbody>
                {shown.map((r) => (
                  <tr key={r.repetition.rep_index}>
                    <td>
                      <a href={`#/technique/${sid}/${r.repetition.rep_index}`}>Attempt {r.repetition.rep_index}</a>
                    </td>
                    <td>{r.repetition.side}</td>
                    <td>{r.repetition.outcome.replace("_", " ")}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        </div>
      )}
      </div>
    </div>
  );
}

const SPEEDS = [0.25, 0.5, 1];

function Attempt({ id, rep }: { id: string; rep: number }) {
  const index = useContentIndex();
  const texts = useTexts();
  const skeleton = useSkeleton();
  const result = useQuery<RepetitionResult>(paths.rep(id, rep));
  const trace = useQuery<RepTrace>(paths.trace(id, rep));
  const clip = useQuery<MotionClip>(paths.motion(id, rep));
  const session = useQuery<{ session: TrainingSessionSummary }>(paths.session(id));
  const player = useMemo(() => (clip.data && clip.data.samples.length > 0 ? new ClipPlayer(clip.data) : null), [clip.data]);
  const poseSource = useMemo(() => (player ? (now: number) => player.advance(now) : null), [player]);
  const [cursor, setCursor] = useState<number | null>(null);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(1);
  useEffect(() => {
    if (!player) return;
    // The cursor follows the player at a low rate; the viewport reads it every frame itself.
    const t = setInterval(() => {
      setCursor(player.timeUs);
      setPlaying(player.playing);
    }, 100);
    return () => clearInterval(t);
  }, [player]);
  const r = result.data;
  if (result.error) return <EmptyState title="Attempt not found"><p>{result.error}</p></EmptyState>;
  if (!r) return <div className="workspace-page"><Heading title={`Attempt ${rep}`} /></div>;
  const rp = r.repetition;
  const origin = session.data?.session.origin;
  const t0 = rp.t_start_us;
  return (
    <div className="workspace-page">
      <Heading title={`Attempt ${rep} · ${nameOf(index, rp.technique)} · ${rp.side}`}>
        {origin && origin !== "live" && <OriginBadge label={origin} />}
        <a href={`#/train/summary/${id}`}>Session summary</a>
        {rep > 1 && <a href={`#/technique/${id}/${rep - 1}`}>← Previous</a>}
        <a href={`#/technique/${id}/${rep + 1}`}>Next →</a>
      </Heading>
      <div className={styles.page}>
      <p className={styles.muted}>
        {rp.outcome.replace("_", " ")} · recognition confidence {rp.recognition_confidence.toFixed(2)} · tracking {rp.capability_profile}
      </p>
      <div className={styles.grid}>
        <div className={styles.stack}>
          <div className={styles.viewport} aria-label="Attempt clip">
            {poseSource ? (
              <ViewportCanvas skeleton={skeleton} selected={-1} onSelect={() => {}} view="3D" skeletonVisible floorVisible poseSource={poseSource} />
            ) : (
              <EmptyState title="No motion stored for this attempt">{clip.error && <p>{clip.error}</p>}</EmptyState>
            )}
          </div>
          {player && (
            <div className={styles.transport} role="toolbar" aria-label="Clip player">
              <Button onClick={() => (player.playing ? player.pause() : player.play())}>{playing ? "⏸ Pause" : "▶ Play"}</Button>
              <Select
                label="Speed"
                value={String(speed)}
                options={SPEEDS.map((s) => ({ value: String(s), label: `${s}×` }))}
                onChange={(e) => {
                  setSpeed(Number(e.target.value));
                  player.speed = Number(e.target.value);
                }}
              />
              <label>
                <input type="checkbox" defaultChecked onChange={(e) => (player.loop = e.target.checked)} /> Loop
              </label>
              <input
                type="range"
                aria-label="Clip position"
                min={player.startUs}
                max={player.endUs}
                value={cursor ?? player.startUs}
                onChange={(e) => {
                  player.seek(Number(e.target.value));
                  setCursor(player.timeUs);
                }}
              />
            </div>
          )}
          <section className={styles.card} aria-label="Phases">
            <h2>Phases</h2>
            <PhaseTimeline
              phases={rp.phases}
              strikeUs={rp.strike_t_us}
              startUs={rp.t_start_us}
              endUs={rp.t_end_us}
              cursorUs={cursor}
              onScrub={(t) => {
                player?.seek(t);
                setCursor(t);
              }}
            />
            <table className={styles.table}>
              <tbody>
                {rp.phases.map((p) => (
                  <tr key={p.phase + p.t_start_us}>
                    <th scope="row">{p.phase}</th>
                    <td className="mono">{Math.round((p.t_end_us - p.t_start_us) / 1000)} ms</td>
                    <td className={styles.muted}>{p.evidence.provenance}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
          <section className={styles.card} aria-label="Traces">
            <h2>Traces</h2>
            {trace.data ? (
              <Chart
                label="Observables over the attempt"
                xFormat={(v) => `${Math.round(v / 1000)} ms`}
                yFormat={(v) => v.toFixed(1)}
                series={trace.data.series.map((s) => ({
                  id: s.id,
                  label: s.id.replaceAll("_", " "),
                  points: trace.data!.t_us.map((t, i) => [t - t0, s.values[i] ?? null] as const),
                }))}
                markers={rp.strike_t_us !== null ? [{ x: rp.strike_t_us - t0, label: "Strike", kind: "strike" }] : []}
                cursor={cursor === null ? null : cursor - t0}
                under={(x, top, bottom) =>
                  rp.phases.map((p, i) => (
                    <rect key={p.phase + p.t_start_us} x={x(p.t_start_us - t0)} y={top} width={Math.max(0, x(p.t_end_us - t0) - x(p.t_start_us - t0))} height={bottom - top} fill={i % 2 ? "var(--surface-2)" : "var(--surface-1)"} />
                  ))
                }
              />
            ) : (
              <p className={styles.muted}>{trace.error ?? "Loading…"}</p>
            )}
          </section>
        </div>
        <div className={styles.stack}>
          {r.cue && <CueBanner cue={r.cue} texts={texts} />}
          <section className={styles.card} aria-label="Metrics">
            <h2>Metrics</h2>
            <table className={styles.table} data-testid="metric-table">
              <tbody>
                {r.measurements.map((m) => (
                  <tr key={m.metric}>
                    <th scope="row">{nameOf(index, m.metric)}</th>
                    <td>
                      <MeasurementValue m={m} />
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
          {r.score && (
            <section className={styles.card} aria-label="Scores">
              <h2>Scores</h2>
              <ScoreBars score={r.score} texts={texts} />
            </section>
          )}
          {r.targets.length > 0 && (
            <section className={styles.card} aria-label="Targets">
              <h2>Targets</h2>
              <ul className={styles.gates}>
                {r.targets.map((t) => (
                  <li key={t.id}>{t.id}: {t.reached === null ? "not assessed" : t.reached ? "reached" : "not reached"}</li>
                ))}
              </ul>
            </section>
          )}
        </div>
      </div>
      </div>
    </div>
  );
}
