import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import type {
  AthleteProfile,
  PlanSide,
  SystemState,
  TrainingEvent,
  TrainingSessionSummary,
  TrainingState,
} from "@contracts";
import { useAction, useSkeleton } from "../../engine/hooks";
import { contentCache, queryCache, useQuery } from "../../engine/queries";
import { observablesStore, trainingEvents, trainingStore } from "../../engine/training";
import { paths, trainingActions } from "../../engine/trainingActions";
import { formatDurationUs } from "../../lib/format";
import { contentId, formatValue, resolveText } from "../../lib/training";
import { Button } from "../../ui/Button";
import { CapabilityBadge } from "../../ui/CapabilityBadge";
import { CueBanner } from "../../ui/CueBanner";
import { EmptyState } from "../../ui/EmptyState";
import { Field } from "../../ui/Field";
import { MeasurementValue } from "../../ui/MeasurementValue";
import { OriginBadge } from "../../ui/OriginBadge";
import { Select } from "../../ui/Select";
import { ViewportCanvas } from "../../viewport/ViewportCanvas";
import { formatUtc, nameOf, SESSION_ID, useContentIndex, useTexts } from "./common";
import styles from "./train.module.css";

export function useTraining(): TrainingState | null {
  return useSyncExternalStore(trainingStore.subscribe, trainingStore.get);
}

/** `#/train`, `#/train/live`, `#/train/summary/<session_id>` (10 §3). */
export function Train({ state, rest }: { state: SystemState | null; rest: string[] }) {
  const training = useTraining();
  if (rest[0] === "summary") {
    const id = rest[1] ?? "";
    return SESSION_ID.test(id) ? <Summary id={id} state={state} /> : <EmptyState title="No such session" />;
  }
  if (rest[0] === "live" && training) return <LiveSession training={training} state={state} />;
  return <Dashboard state={state} training={training} />;
}

function Heading({ title, children }: { title: string; children?: React.ReactNode }) {
  return (
    <header className="workspace-heading">
      <div>
        <span className="eyebrow">Workspace / Train</span>
        <h1>{title}</h1>
      </div>
      <div className="workspace-tools">{children}</div>
    </header>
  );
}

// ---- dashboard ---------------------------------------------------------------

function Dashboard({ state, training }: { state: SystemState | null; training: TrainingState | null }) {
  const athlete = state?.athlete ?? null;
  const running = training && training.phase !== "completed" && training.phase !== "aborted";
  return (
    <div className="workspace-page">
      <Heading title="Train">
        <CapabilityBadge report={state?.capability_report} />
      </Heading>
      <div className={styles.page}>
      <div className={styles.grid}>
        <div className={styles.stack}>
          <AthletePanel state={state} />
          {running ? (
            <section className={styles.card} aria-label="Session in progress">
              <h2>Session in progress</h2>
              <p>{training.plan.title}</p>
              <a href="#/train/live">Open the live session</a>
            </section>
          ) : (
            athlete && <PlanPicker />
          )}
        </div>
        <div className={styles.stack}>{athlete && <RecentSessions athleteId={athlete.id} />}</div>
      </div>
      </div>
    </div>
  );
}

function AthletePanel({ state }: { state: SystemState | null }) {
  const athletes = useQuery<AthleteProfile[]>(state ? paths.athletes : null);
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [height, setHeight] = useState("1.75");
  const [consent, setConsent] = useState(true);
  const [select, selectStatus] = useAction(trainingActions.setActive);
  const [error, setError] = useState<string | null>(null);
  const active = state?.athlete ?? null;
  async function create() {
    setError(null);
    const id = name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "") || "athlete";
    const created = await trainingActions.createAthlete({ id, display_name: name.trim(), height_m: Number(height) });
    if (!created.ok) return setError(created.error.message);
    if (consent) {
      const c = await trainingActions.consent(id, true);
      if (!c.ok) return setError(c.error.message);
    }
    const a = await trainingActions.setActive(id);
    if (!a.ok) return setError(a.error.message);
    queryCache.invalidate(paths.athletes);
    setCreating(false);
  }
  const list = athletes.data ?? [];
  return (
    <section className={styles.card} aria-label="Athlete">
      <h2>Athlete</h2>
      {active && !creating ? (
        <div className={styles.row}>
          <Select
            label="Active athlete"
            value={active.id}
            options={list.map((a) => ({ value: a.id, label: a.display_name }))}
            disabled={selectStatus.pending}
            onChange={(e) => void select(e.target.value)}
          />
          <Button onClick={() => setCreating(true)}>New athlete</Button>
          {list.find((a) => a.id === active.id)?.consent.motion_data_utc ? null : (
            <span className={styles.notice}>Motion data is not saved for this athlete: no consent given.</span>
          )}
        </div>
      ) : (
        <form
          className={styles.stack}
          onSubmit={(e) => {
            e.preventDefault();
            void create();
          }}
        >
          {!active && <p className={styles.muted}>Create an athlete to start training.</p>}
          <Field label="Name" value={name} required maxLength={64} onChange={(e) => setName(e.target.value)} />
          <Field label="Height (m)" type="number" min={1} max={2.5} step={0.01} value={height} onChange={(e) => setHeight(e.target.value)} />
          <label>
            <input type="checkbox" checked={consent} onChange={(e) => setConsent(e.target.checked)} /> Store my motion data on
            this computer
          </label>
          <div className={styles.row}>
            <Button type="submit" variant="primary" disabled={!name.trim()}>
              Create athlete
            </Button>
            {active && <Button onClick={() => setCreating(false)}>Cancel</Button>}
          </div>
          {error && <p className="error" role="alert">{error}</p>}
        </form>
      )}
    </section>
  );
}

function PlanPicker() {
  const index = useContentIndex();
  const disciplines = index?.entries.filter((e) => e.kind === "disciplines" && e.valid) ?? [];
  const [discipline, setDiscipline] = useState<string>("");
  const d = discipline || disciplines[0]?.id || "";
  const techniques = index?.entries.filter((e) => e.kind === "techniques" && e.valid && e.discipline === d) ?? [];
  const [technique, setTechnique] = useState("");
  const t = technique || techniques[0]?.id || "";
  const [side, setSide] = useState<PlanSide>("right");
  const [sets, setSets] = useState("3");
  const [reps, setReps] = useState("10");
  const [rest, setRest] = useState("30");
  const [create, status] = useAction(trainingActions.createSession);
  async function prepare() {
    const r = await create({
      plan: { kind: "technique", technique: t, side, sets: Number(sets), reps: Number(reps), rest_s: Number(rest) },
    });
    if (r.ok) location.hash = "#/train/live";
  }
  if (!index) return <section className={styles.card}><h2>Plan</h2><p className={styles.muted}>Loading content…</p></section>;
  return (
    <section className={styles.card} aria-label="Plan">
      <h2>Plan</h2>
      <div className={styles.row}>
        <Select label="Discipline" value={d} options={disciplines.map((e) => ({ value: e.id, label: e.names.display }))} onChange={(e) => setDiscipline(e.target.value)} />
        <Select label="Technique" value={t} options={techniques.map((e) => ({ value: e.id, label: e.names.native ? `${e.names.display} (${e.names.native})` : e.names.display }))} onChange={(e) => setTechnique(e.target.value)} />
        <Select
          label="Side"
          value={side}
          options={[
            { value: "right", label: "Right" },
            { value: "left", label: "Left" },
            { value: "alternate", label: "Alternate" },
          ]}
          onChange={(e) => setSide(e.target.value as PlanSide)}
        />
      </div>
      <div className={styles.row}>
        <Field label="Sets" type="number" min={1} max={8} value={sets} onChange={(e) => setSets(e.target.value)} />
        <Field label="Repetitions" type="number" min={1} max={20} value={reps} onChange={(e) => setReps(e.target.value)} />
        <Field label="Rest (s)" type="number" min={0} max={600} value={rest} onChange={(e) => setRest(e.target.value)} />
      </div>
      <div className={styles.row}>
        <Button variant="primary" disabled={!t || status.pending} onClick={() => void prepare()}>
          Prepare session
        </Button>
      </div>
      {status.error && <p className="error" role="alert">{status.error}</p>}
    </section>
  );
}

function RecentSessions({ athleteId }: { athleteId: string }) {
  const sessions = useQuery<TrainingSessionSummary[]>(paths.sessions(athleteId, 5));
  return (
    <section className={styles.card} aria-label="Recent sessions">
      <h2>Recent sessions</h2>
      {sessions.data?.length ? (
        <table className={styles.table}>
          <tbody>
            {sessions.data.map((s) => (
              <tr key={s.id}>
                <td>
                  <a href={`#/train/summary/${s.id}`}>{s.title}</a>
                  <div className={styles.muted}>{formatUtc(s.started_utc)}</div>
                </td>
                <td className="mono">{s.reps_completed} reps</td>
                <td>{s.origin !== "live" && <OriginBadge label={s.origin} />}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <p className={styles.muted}>No stored sessions yet.</p>
      )}
    </section>
  );
}

// ---- live session ---------------------------------------------------------------

/** Live observable values written straight into the DOM each animation frame, never React state. */
function Gauges({ ids }: { ids: readonly string[] }) {
  const refs = useRef(new Map<string, HTMLElement>());
  useEffect(() => {
    let frame = 0;
    const draw = () => {
      frame = requestAnimationFrame(draw);
      for (const [id, el] of refs.current) {
        const v = observablesStore.values.get(id);
        // shortcut: unit from the id's name, as `ObservableValue` carries none; upgrade when the
        // observables topic or content index states units.
        const unit = id.includes("flexion") || id.includes("elevation") || id.includes("yaw") || id.includes("lean") ? "rad" : id.includes("speed") ? "m_s" : id.includes("rate") ? "rad_s" : "m";
        const f = formatValue(v?.value ?? null, unit);
        const text = `${f.text}${f.unit ? ` ${f.unit}` : ""}`;
        if (el.textContent !== text) el.textContent = text;
      }
    };
    draw();
    return () => cancelAnimationFrame(frame);
  }, []);
  if (ids.length === 0) return null;
  return (
    <dl className={styles.gauges} aria-label="Live values">
      {ids.map((id) => (
        <div key={id}>
          <dt>{id.replaceAll("_", " ")}</dt>
          <dd
            ref={(el) => {
              if (el) refs.current.set(id, el);
              else refs.current.delete(id);
            }}
          >
            —
          </dd>
        </div>
      ))}
    </dl>
  );
}

function LiveSession({ training, state }: { training: TrainingState; state: SystemState | null }) {
  const skeleton = useSkeleton();
  const texts = useTexts();
  const index = useContentIndex();
  const events = useSyncExternalStore(trainingEvents.store.subscribe, trainingEvents.store.get);
  const [command, status] = useAction(trainingActions.command);
  const cur = training.current;
  const ended = training.phase === "completed" || training.phase === "aborted";
  const safety = training.readiness.find((g) => g.gate === "safety_ack" && g.state === "fail");
  const lastSet = [...events].reverse().find((e) => e.type === "set_completed");
  const lost = cur?.sub_state === "tracking_lost";
  const last = training.last_rep;
  return (
    <div className="workspace-page">
      <Heading title={`${training.plan.title}${cur?.exercise.side ? ` · ${cur.exercise.side}` : ""}`}>
        <CapabilityBadge report={state?.capability_report} />
        {state?.devices.some((d) => d.descriptor.origin === "simulated") && <OriginBadge label="simulated" />}
      </Heading>
      <div className={styles.page}>
      <div className={styles.grid}>
        <div className={styles.stack}>
          <div className={styles.viewport} aria-label="Live avatar">
            <ViewportCanvas skeleton={skeleton} selected={-1} onSelect={() => {}} view="3D" skeletonVisible floorVisible />
          </div>
          {last?.cue && <CueBanner cue={last.cue} texts={texts} />}
          <Gauges ids={training.live_observables.slice(0, 8)} />
          <div className={styles.transport} role="toolbar" aria-label="Session controls">
            {(training.phase === "preparing" || training.phase === "ready") && (
              <Button variant="primary" disabled={training.phase !== "ready" || status.pending} onClick={() => void command({ command: "start" })}>
                ▶ Start
              </Button>
            )}
            {training.phase === "paused" ? (
              <Button onClick={() => void command({ command: "resume" })}>▶ Resume</Button>
            ) : (
              !ended && training.phase !== "preparing" && training.phase !== "ready" && <Button onClick={() => void command({ command: "pause" })}>⏸ Pause</Button>
            )}
            {training.phase === "resting" && <Button onClick={() => void command({ command: "skip_rest" })}>⏭ Skip rest</Button>}
            {!ended && training.phase !== "preparing" && training.phase !== "ready" && <Button onClick={() => void command({ command: "repeat_item" })}>↻ Repeat</Button>}
            {!ended && (
              <Button variant="danger" onClick={() => void command({ command: "end" })}>
                ■ End
              </Button>
            )}
            <span className="mono" aria-label="Elapsed">{formatDurationUs(training.totals.elapsed_ms * 1000)}</span>
            {status.error && <span className="error" role="alert">{status.error}</span>}
          </div>
        </div>
        <div className={styles.stack}>
          <section className={styles.card} aria-label="Session state">
            <h2>{training.phase.replace("_", " ")}</h2>
            {cur && (
              <>
                <div className={styles.row}>
                  <span className={styles.big} data-testid="set-count">Set {training.position.set ?? cur.sets.done} / {cur.sets.target}</span>
                </div>
                <div className={styles.row}>
                  <span className={styles.big} data-testid="rep-count">
                    Rep {cur.counts.done} / {cur.counts.target ?? "—"}
                  </span>
                </div>
                <p>Phase: <strong>{cur.phase_id ?? "—"}</strong>{lost && " · tracking lost"}</p>
              </>
            )}
            <p className={styles.muted}>
              {training.totals.reps_completed} counted · {training.totals.reps_other} other
            </p>
          </section>
          {(training.phase === "preparing" || training.phase === "ready") && (
            <section className={styles.card} aria-label="Readiness">
              <h2>Readiness</h2>
              <ul className={styles.gates}>
                {training.readiness.map((g) => (
                  <li key={g.gate} className={styles.gate} data-state={g.state}>
                    {g.state === "pass" ? "✓" : g.state === "warn" ? "!" : "✗"} {g.gate.replaceAll("_", " ")}
                    {g.reasons.length > 0 && <span className={styles.muted}> — {g.reasons.map((r) => r.message).join(" ")}</span>}
                  </li>
                ))}
              </ul>
              {safety && cur && (
                <>
                  <SafetyNotes technique={contentId(cur.exercise.ref)} texts={texts} />
                  <Button onClick={() => void command({ command: "ack_safety", args: { content_refs: [cur.exercise.ref] } })}>
                    I have read the safety notes
                  </Button>
                </>
              )}
              <h2>Metrics on this setup</h2>
              <ul className={styles.gates}>
                {training.metric_availability.map((m) => (
                  <li key={m.metric}>
                    {nameOf(index, m.metric)}: <strong>{m.best_grade === "X" ? "not assessed" : `best grade ${m.best_grade}`}</strong>
                    {m.best_grade === "X" && <span className={styles.muted}> — {m.reasons.map((r) => r.message).join(" ")}</span>}
                  </li>
                ))}
              </ul>
            </section>
          )}
          {lost && (
            <section className={styles.card} aria-label="Tracking lost">
              <h2>Tracking lost</h2>
              <ul className={styles.gates}>
                {state?.capability_report?.observables
                  .filter((o) => o.state === "unavailable" && !o.id.startsWith("input."))
                  .map((o) => <li key={o.id}>{o.id}: {o.reasons.map((r) => r.message).join(" ")}</li>)}
              </ul>
            </section>
          )}
          {training.phase === "resting" && lastSet && <SetSummary event={lastSet} texts={texts} />}
          {last && (
            <section className={styles.card} aria-label="Last repetition">
              <h2>Last repetition · #{last.repetition.rep_index} · {last.repetition.outcome.replace("_", " ")}</h2>
              <table className={styles.table}>
                <tbody>
                  {last.measurements.slice(0, 6).map((m) => (
                    <tr key={m.metric}>
                      <th scope="row">{nameOf(index, m.metric)}</th>
                      <td><MeasurementValue m={m} /></td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </section>
          )}
          {ended && (
            <section className={styles.card} aria-label="Session finished">
              <h2>Session {training.phase}</h2>
              <a href={`#/train/summary/${training.session_id}`}>Open the session summary</a>
            </section>
          )}
          <p className={styles.muted}>
            Recording: {training.recording.inputs ? "inputs" : "no inputs"}, {training.recording.motion ? "motion" : "no motion"} ·
            storage {training.storage}
          </p>
        </div>
      </div>
      </div>
    </div>
  );
}

/** The technique's safety notes, shown before the acknowledgement (17 §4.3). */
function SafetyNotes({ technique, texts }: { technique: string; texts: Record<string, string> | null }) {
  const def = useQuery<{ safety?: { notes?: string[] } }>(`/content/techniques/${technique}`, contentCache);
  const notes = def.data?.safety?.notes ?? [];
  return (
    <ul className={styles.gates} aria-label="Safety notes">
      {notes.map((k) => (
        <li key={k}>{resolveText(k, {}, texts)}</li>
      ))}
    </ul>
  );
}

function SetSummary({ event, texts }: { event: TrainingEvent; texts: Record<string, string> | null }) {
  const cues = (event.data.cues as { observed: { key: string; params: Record<string, never> } }[] | undefined) ?? [];
  return (
    <section className={styles.card} aria-label="Set summary">
      <h2>Set {event.position.set ?? ""} complete · rest</h2>
      {cues.map((c, i) => (
        <p key={i}>{resolveText(c.observed.key, c.observed.params, texts)}</p>
      ))}
    </section>
  );
}

// ---- summary ----------------------------------------------------------------------

function Summary({ id, state }: { id: string; state: SystemState | null }) {
  const index = useContentIndex();
  const detail = useQuery<{ session: TrainingSessionSummary }>(paths.session(id));
  const reps = useQuery<import("@contracts").RepetitionResult[]>(paths.reps(id));
  const s = detail.data?.session;
  const metricIds = [...new Set((reps.data ?? []).flatMap((r) => r.measurements.map((m) => m.metric)))].slice(0, 4);
  return (
    <div className="workspace-page">
      <Heading title={s ? `Summary · ${s.title}` : "Session summary"}>
        {s && s.origin !== "live" && <OriginBadge label={s.origin} />}
        {s?.plan_ref && <a href={`#/progress/${contentId(s.plan_ref)}/kick.total_time`}>Progress</a>}
      </Heading>
      <div className={styles.page}>
      {detail.error && (
        <EmptyState title="This session is not stored">
          <p>{detail.error} Sessions of athletes without motion-data consent are not saved.</p>
        </EmptyState>
      )}
      {s && (
        <p className={styles.muted}>
          {formatUtc(s.started_utc)} · {s.status} · {s.reps_completed} counted, {s.reps_other} other · tracking {s.capability_profile}
          {state?.training?.session_id === id ? " · live" : ""}
        </p>
      )}
      {reps.data && (
        <table className={styles.table} aria-label="Repetitions">
          <thead>
            <tr>
              <th>#</th>
              <th>Outcome</th>
              {metricIds.map((m) => (
                <th key={m}>{nameOf(index, m)}</th>
              ))}
              <th />
            </tr>
          </thead>
          <tbody>
            {reps.data.map((r) => (
              <tr key={r.repetition.rep_index}>
                <td className="mono">{r.repetition.rep_index}</td>
                <td>{r.repetition.outcome.replace("_", " ")}</td>
                {metricIds.map((m) => {
                  const v = r.measurements.find((x) => x.metric === m);
                  return <td key={m}>{v ? <MeasurementValue m={v} showReasons={false} /> : "—"}</td>;
                })}
                <td>
                  <a href={`#/technique/${id}/${r.repetition.rep_index}`} aria-label={`Open attempt ${r.repetition.rep_index}`}>
                    Attempt
                  </a>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      </div>
    </div>
  );
}
