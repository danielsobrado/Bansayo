import { useEffect, useRef, useState } from "react";
import type { TimedWorkoutDef, TrainingState } from "@contracts";
import { useAction, useConnection } from "../../engine/hooks";
import { contentCache, useQuery } from "../../engine/queries";
import { trainingActions } from "../../engine/trainingActions";
import { formatDurationUs } from "../../lib/format";
import { Button } from "../../ui/Button";
import backingLoop from "./full-body-loop.wav";
import styles from "./train.module.css";

export function FollowAlong({ training }: { training: TrainingState }) {
  const [command, status] = useAction(trainingActions.command);
  const [music, setMusic] = useState(true);
  const connected = useConnection().kind === "open";
  const audio = useRef<HTMLAudioElement>(null);
  const { phase, current, position, totals } = training;
  const [id, version] = (training.plan.ref ?? "").split("@");
  const definition = useQuery<TimedWorkoutDef>(
    id && version ? `/content/workouts/${encodeURIComponent(id)}?version=${encodeURIComponent(version)}` : null,
    contentCache,
  );
  const blocks = definition.data?.blocks ?? [];
  const blockIndex = position.block ?? 0;
  const itemIndex = position.item ?? 0;
  const block = blocks[blockIndex];
  const item = block?.items[itemIndex];
  const next = block?.items[itemIndex + 1] ?? blocks[blockIndex + 1]?.items[0];
  const durationMs = current?.timer?.duration_ms ?? 0;
  const remainingMs = Math.max(0, durationMs - (current?.timer?.elapsed_ms ?? 0));
  const finished = phase === "completed" || phase === "aborted";
  const previewMs = ((current?.beat?.period_us ?? 500_000) * 8) / 1000;
  const starting = phase === "ready" || phase === "countdown";

  useEffect(() => {
    const track = audio.current;
    return () => track?.pause();
  }, []);

  useEffect(() => {
    const track = audio.current;
    if (!track) return;
    if (!connected || !music || phase !== "active") {
      track.pause();
      return;
    }
    let current = true;
    const sync = () => {
      if (!Number.isFinite(track.duration) || track.duration <= 0) return;
      const offset = (totals.active_ms / 1000) % track.duration;
      if (track.paused || Math.abs(track.currentTime - offset) > 0.65) track.currentTime = offset;
    };
    track.volume = 0.45;
    track.addEventListener("loadedmetadata", sync);
    sync();
    if (track.paused) void track.play().catch(() => {
      if (current) setMusic(false);
    });
    return () => {
      current = false;
      track.removeEventListener("loadedmetadata", sync);
    };
  }, [connected, music, phase, totals.active_ms]);

  return (
    <div className={styles.page}>
      <audio ref={audio} src={backingLoop} loop preload="auto" />
      {finished ? (
        <>
          <WorkoutResults activeMs={totals.active_ms} blocks={phase === "completed" ? blocks.length : blockIndex} completed={phase === "completed"} />
          <a href={`#/train/summary/${training.session_id}`}>Open the session summary</a>
        </>
      ) : (
        <div className={styles.grid}>
          <section className={`${styles.card} ${styles.followAlong}`} aria-label="Current movement">
            <div aria-live="polite" aria-atomic="true">
              <h2>{block?.title ?? "Follow along"}</h2>
              <h3>{phase === "countdown" ? "Get ready" : item?.title ?? current?.title}</h3>
              <p>{starting ? "Clear space around you and keep a stable wall within reach." : item?.instruction}</p>
              {item?.easier && !starting && <p className={styles.muted}>Easier option · {item.easier}</p>}
            </div>
            <div className={styles.classTimer} aria-label="Time remaining" data-testid="class-timer">
              {formatDurationUs(Math.ceil(remainingMs / 1000) * 1_000_000)}
            </div>
            <progress aria-label="Current timer progress" max={durationMs || 1} value={durationMs - remainingMs} />
            <p className={styles.nextMove} aria-live="polite">
              {phase === "countdown" && item ? `First · ${item.title}` : null}
              {phase === "active" && remainingMs <= previewMs && next ? `Next · ${next.title}` : null}
            </p>
            <div className={styles.transport}>
              {phase === "ready" && <Button variant="primary" disabled={!connected || status.pending || !definition.data} onClick={() => void command(training.session_id, { command: "start" })}>Start class</Button>}
              {phase === "active" && <Button disabled={!connected || status.pending} onClick={() => void command(training.session_id, { command: "pause" })}>Pause class</Button>}
              {phase === "paused" && <Button variant="primary" disabled={!connected || status.pending} onClick={() => void command(training.session_id, { command: "resume" })}>Resume class</Button>}
              <Button onClick={() => {
                if (connected && !music && phase === "active") void audio.current?.play().catch(() => {});
                setMusic(!music);
              }}>{music ? "Mute music" : "Play music"}</Button>
              <Button disabled={!connected || status.pending} onClick={() => void command(training.session_id, { command: "end" })}>End class</Button>
            </div>
            {phase === "paused" && <p role="status">Paused</p>}
            {(status.error || definition.error) && <p className="error" role="alert">{status.error ?? definition.error}</p>}
          </section>
          <section className={styles.card} aria-label="Class outline">
            <h2>Class outline</h2>
            <ol className={styles.classOutline}>
              {blocks.map((b, i) => <li key={b.id} aria-current={i === blockIndex ? "step" : undefined}>{b.title}</li>)}
            </ol>
            <p className={styles.muted}>Class time · {formatDurationUs(totals.active_ms * 1000)}</p>
          </section>
        </div>
      )}
    </div>
  );
}

export function WorkoutResults({ activeMs, blocks, completed }: { activeMs: number | null; blocks: number | null; completed: boolean }) {
  return (
    <section className={styles.card} aria-label="Class results">
      <h2>{completed ? "Class complete" : "Class ended early"}</h2>
      <dl className={styles.gauges}>
        <div><dt>Class time</dt><dd data-testid="class-active-time">{activeMs !== null ? formatDurationUs(activeMs * 1000) : "Unavailable"}</dd></div>
        {blocks !== null && <div><dt>Blocks completed</dt><dd>{blocks}</dd></div>}
      </dl>
    </section>
  );
}
