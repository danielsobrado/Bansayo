import { useEffect, useState } from "react";
import type { RecordingSession, SystemState } from "@contracts";
import { actions } from "../../engine/actions";
import { useAction } from "../../engine/hooks";
import { formatDurationUs } from "../../lib/format";
import { EmptyState } from "../../ui/EmptyState";
import { OriginBadge } from "../../ui/OriginBadge";

export function Sessions({
  state,
  onRecord,
}: {
  state: SystemState | null;
  onRecord: () => void;
}) {
  const [sessions, setSessions] = useState<RecordingSession[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [looping, setLooping] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const [play, status] = useAction(actions.startPlayback);
  const recordingId = state?.recording?.session_id;
  const connected = state !== null;
  useEffect(() => {
    let active = true;
    if (!connected) {
      setSessions([]);
      return;
    }
    void actions.sessions().then((response) => {
      if (!active) return;
      if (response.ok) {
        setSessions(response.data);
        setLoadError(null);
      } else setLoadError(response.error.message);
    });
    return () => {
      active = false;
    };
  }, [connected, recordingId, refresh]);
  async function replay(session: RecordingSession) {
    const response = await play({ session_id: session.id, speed: 1, looping });
    if (response.ok) location.hash = "#/studio";
  }
  const feature = state?.features.find((f) => f.id === "playback");
  return (
    <div className="workspace-page">
      <header className="workspace-heading">
        <div>
          <span className="eyebrow">Workspace / Sessions</span>
          <h1>
            Recorded sessions{" "}
            <span className="heading-count mono">{sessions.length}</span>
          </h1>
        </div>
        <div className="workspace-tools">
          <label className="loop-choice">
            <input
              type="checkbox"
              checked={looping}
              onChange={(e) => setLooping(e.target.checked)}
            />{" "}
            Loop replay
          </label>
          <button
            onClick={() => setRefresh((v) => v + 1)}
            disabled={!connected}
          >
            Refresh
          </button>
          <button
            onClick={onRecord}
            disabled={!connected || !!state.recording || !!state.playback}
          >
            ● Record session
          </button>
        </div>
      </header>
      {(loadError || status.error) && (
        <p className="error page-error" role="alert">
          {loadError || status.error}
        </p>
      )}
      {sessions.length ? (
        <div className="sessions-scroll">
          <table className="sessions-table">
            <thead>
              <tr>
                <th>Name</th>
                <th>Created (UTC)</th>
                <th>Duration</th>
                <th>Devices</th>
                <th>Origin</th>
                <th>Size</th>
                <th>Replay</th>
              </tr>
            </thead>
            <tbody>
              {sessions.map((session) => (
                <tr key={session.id}>
                  <td>
                    <strong>{session.name || "Untitled session"}</strong>
                    <span className="session-id mono">{session.id}</span>
                  </td>
                  <td className="mono">
                    {session.created_utc
                      .replace("T", " ")
                      .replace(/\.\d+Z$/, "Z")}
                  </td>
                  <td className="mono">
                    {formatDurationUs(session.duration_us)}
                  </td>
                  <td className="mono">{session.device_count}</td>
                  <td>
                    {session.origins
                      .filter((o) => o !== "live")
                      .map((o) => (
                        <OriginBadge key={o} label={o} />
                      ))}
                  </td>
                  <td className="mono">
                    {(session.size_bytes / 1024).toFixed(1)} KiB
                  </td>
                  <td>
                    <button
                      disabled={
                        status.pending ||
                        !!state?.recording ||
                        feature?.state === "unavailable"
                      }
                      title={feature?.reasons.map((r) => r.message).join(" ")}
                      onClick={() => void replay(session)}
                      aria-label={`Replay ${session.name}`}
                    >
                      ▶
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <EmptyState
          title={
            connected
              ? "No sessions recorded"
              : "Connect to the engine to list sessions"
          }
        >
          <p>
            Record incoming source observations, then replay them through the
            same tracking pipeline.
          </p>
          {connected && (
            <button
              onClick={onRecord}
              disabled={!!state.recording || !!state.playback}
            >
              Record a session
            </button>
          )}
        </EmptyState>
      )}
    </div>
  );
}
