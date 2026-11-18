import type { SystemState } from "@contracts";
import { actions } from "../engine/actions";
import { useAction } from "../engine/hooks";
import { useState } from "react";
import { formatDurationUs, formatSpeed } from "../lib/format";
import { useShortcuts } from "../lib/shortcuts";
import { Icon } from "../ui/Icon";

const SPEEDS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 4];
export function TransportBar({
  state,
  onRecord,
}: {
  state: SystemState | null;
  onRecord: () => void;
}) {
  const simulation = state?.simulation;
  const playback = state?.playback;
  const [simulationAction, simulationStatus] = useAction(
    actions.updateSimulation,
  );
  const [playbackAction, playbackStatus] = useAction(actions.updatePlayback);
  const [stopPlayback, stopStatus] = useAction(actions.stopPlayback);
  const [stopRecording, recordStatus] = useAction(actions.stopRecording);
  const [seek, setSeek] = useState<number | null>(null);
  const recordingFeature = state?.features.find((f) => f.id === "recording");
  const recordingReason =
    recordingFeature?.reasons.map((r) => r.message).join(" ") ||
    "Record source observations (R)";
  const recordDisabled =
    !state || !!playback || recordingFeature?.state === "unavailable";
  const pause = () => {
    if (playback) void playbackAction({ paused: !playback.paused });
    else if (simulation) void simulationAction({ paused: !simulation.paused });
  };
  const setSpeed = (speed: number) => {
    if (playback) void playbackAction({ speed });
    else if (simulation) void simulationAction({ speed });
  };
  const record = () => {
    if (state?.recording) void stopRecording();
    else if (!recordDisabled) onRecord();
  };
  useShortcuts({
    " ": pause,
    r: record,
    "[": () =>
      setSpeed(
        Math.max(0.1, (playback?.speed ?? simulation?.speed ?? 1) - 0.25),
      ),
    "]": () =>
      setSpeed(Math.min(4, (playback?.speed ?? simulation?.speed ?? 1) + 0.25)),
  });
  const error =
    simulationStatus.error ||
    playbackStatus.error ||
    stopStatus.error ||
    recordStatus.error;
  function commitSeek() {
    if (seek === null) return;
    void playbackAction({ position_us: seek });
    setSeek(null);
  }
  return (
    <footer className={`transport ${playback ? "replay-transport" : ""}`}>
      {playback ? (
        <>
          <button
            disabled={stopStatus.pending}
            onClick={() => void stopPlayback()}
          >
            <Icon name="back" size={13} /> Exit replay
          </button>
          <button
            disabled={playbackStatus.pending}
            title="Play / pause (Space)"
            aria-keyshortcuts="Space"
            onClick={pause}
          >
            <Icon name={playback.paused ? "play" : "pause"} size={13} />
            {playback.paused ? "Play" : "Pause"}
          </button>
          <label className="speed-label">
            Speed{" "}
            <select
              aria-label="Playback speed"
              value={playback.speed}
              onChange={(e) => setSpeed(Number(e.target.value))}
            >
              {[...new Set([...SPEEDS, playback.speed])]
                .sort((a, b) => a - b)
                .map((speed) => (
                  <option key={speed} value={speed}>
                    {formatSpeed(speed)}
                  </option>
                ))}
            </select>
          </label>
          <label className="scrubber">
            <span className="mono">
              {formatDurationUs(seek ?? playback.position_us)}
            </span>
            <input
              type="range"
              aria-label="Playback position"
              aria-valuetext={formatDurationUs(seek ?? playback.position_us)}
              min="0"
              max={playback.session.duration_us}
              step="1000"
              value={seek ?? playback.position_us}
              onChange={(e) => setSeek(Number(e.target.value))}
              onPointerUp={commitSeek}
              onKeyUp={commitSeek}
              onBlur={commitSeek}
            />
            <span className="mono">
              {formatDurationUs(playback.session.duration_us)}
            </span>
          </label>
          <span className="replay-loop">
            {playback.looping && <Icon name="rotate" size={13} />}
            {playback.looping ? "Looping" : "Single play"}
          </span>
        </>
      ) : (
        <>
          <span className="transport-label">SIMULATION</span>
          <label>
            Motion{" "}
            <select
              aria-label="Motion"
              disabled={!simulation || simulationStatus.pending}
              value={simulation?.motion ?? ""}
              onChange={(e) =>
                void simulationAction({ motion: e.target.value })
              }
            >
              {!simulation && <option value="">No simulated source</option>}
              {simulation?.available_motions.map((motion) => (
                <option key={motion.id} value={motion.id}>
                  {motion.label}
                </option>
              ))}
            </select>
          </label>
          <label className="speed-label">
            Speed{" "}
            <select
              aria-label="Simulation speed"
              disabled={!simulation || simulationStatus.pending}
              value={simulation?.speed ?? 1}
              onChange={(e) => setSpeed(Number(e.target.value))}
            >
              {[...new Set([...SPEEDS, simulation?.speed ?? 1])]
                .sort((a, b) => a - b)
                .map((speed) => (
                  <option value={speed} key={speed}>
                    {formatSpeed(speed)}
                  </option>
                ))}
            </select>
          </label>
          <button
            title="Play / pause (Space)"
            aria-keyshortcuts="Space"
            disabled={!simulation || simulationStatus.pending}
            onClick={pause}
          >
            <Icon name={simulation?.paused ? "play" : "pause"} size={13} />
            {simulation?.paused ? "Play" : "Pause"}
          </button>
          <span className="transport-divider" />
          <button
            className={state?.recording ? "recording-button" : ""}
            title={recordingReason}
            aria-keyshortcuts="R"
            disabled={
              recordStatus.pending || (!state?.recording && recordDisabled)
            }
            onClick={record}
          >
            <Icon name={state?.recording ? "stop" : "record"} size={13} />
            {state?.recording ? "Stop recording" : "Record"}
          </button>
          <span className="transport-end">
            {state ? (
              <>
                <span className="live-mark" aria-hidden="true">
                  ●
                </span>{" "}
                Live stream
              </>
            ) : (
              "Stream unavailable"
            )}
          </span>
        </>
      )}
      {error && (
        <span className="error transport-error" role="alert">
          {error}
        </span>
      )}
    </footer>
  );
}
