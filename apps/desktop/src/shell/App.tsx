import { useEffect, useState, useSyncExternalStore } from "react";
import { engineConnection } from "../engine/connection";
import { ENGINE_HOST } from "../engine/config";
import {
  useConnection,
  useSampledFrame,
  useSkeleton,
  useSystemState,
} from "../engine/hooks";
import { readouts } from "../engine/readouts";
import { calibrationReadiness, nonLiveOrigins } from "../engine/selectors";
import { announcementStore } from "../lib/announcer";
import { formatDurationUs } from "../lib/format";
import { useShortcuts } from "../lib/shortcuts";
import { useRoute } from "../lib/hashRoute";
import { OriginBadge } from "../ui/OriginBadge";
import { StatePill } from "../ui/StatePill";
import { ReferencePoseFlow } from "../calibration/ReferencePoseFlow";
import { Studio } from "../workspaces/studio/Studio";
import { Overview } from "../workspaces/overview/Overview";
import { Devices } from "../workspaces/devices/Devices";
import { Cameras } from "../workspaces/cameras/Cameras";
import { Sessions } from "../workspaces/sessions/Sessions";
import { TransportBar } from "./TransportBar";
import { RecordingDialog } from "./RecordingDialog";

const NAV = [
  { id: "overview", label: "Overview", icon: "▣", key: "1" },
  { id: "studio", label: "Studio", icon: "◇", key: "2" },
  { id: "devices", label: "Devices", icon: "⌬", key: "3" },
  { id: "cameras", label: "Cameras", icon: "◉", key: "4" },
  { id: "sessions", label: "Sessions", icon: "▤", key: "5" },
];
const WORKSPACES = NAV.map((item) => item.id);
export function App() {
  const connection = useConnection();
  const state = useSystemState((s) => s);
  const skeleton = useSkeleton();
  const sampled = useSampledFrame(10, readouts);
  const announcement = useSyncExternalStore(
    announcementStore.subscribe,
    announcementStore.get,
  );
  const [calibrating, setCalibrating] = useState(false);
  const [recording, setRecording] = useState(false);
  const route = useRoute(WORKSPACES, "overview");
  useEffect(() => {
    if (!location.hash) location.hash = "#/overview";
    engineConnection.start();
    return () => engineConnection.stop();
  }, []);
  const connected = connection.kind === "open";
  const readiness = calibrationReadiness(state);
  const origins = [
    ...new Set([
      ...nonLiveOrigins(sampled.origins),
      ...(state?.devices.flatMap((d) =>
        d.descriptor.origin === "live" ? [] : [d.descriptor.origin],
      ) ?? []),
      ...(state?.engine.mode === "replay" ? ["replay"] : []),
    ]),
  ];
  useShortcuts(
    Object.fromEntries(
      NAV.map((item) => [
        item.key,
        () => {
          location.hash = `#/${item.id}`;
        },
      ]),
    ),
  );
  const calibrate = () => setCalibrating(true);
  return (
    <div className="app-shell">
      <nav className="navigation" aria-label="Workspaces">
        <div className="brand">
          <span className="brand-mark">K</span>
          <span className="brand-name">
            kinexa<span>motion platform</span>
          </span>
        </div>
        <span className="rail-label">Workspace</span>
        {NAV.map((item) => (
          <a
            key={item.id}
            href={`#/${item.id}`}
            aria-current={route === item.id ? "page" : undefined}
            title={`${item.label} (${item.key})`}
          >
            <span className="nav-icon" aria-hidden="true">
              {item.icon}
            </span>
            <span className="nav-text">{item.label}</span>
            <kbd>{item.key}</kbd>
          </a>
        ))}
        <div className="rail-bottom">
          <span className="rail-label">Local engine</span>
          <span className="mono endpoint">{ENGINE_HOST}</span>
          <span className="rail-version">PoC · Local motion capture</span>
        </div>
      </nav>
      <header className={`statusbar ${state?.playback ? "is-replay" : ""}`}>
        <span className={`connection-status ${connected ? "connected" : ""}`}>
          <span aria-hidden="true">{connected ? "●" : "○"}</span>{" "}
          {connected
            ? "Engine connected"
            : connection.kind === "incompatible"
              ? "Protocol incompatible"
              : "Connecting to engine"}
        </span>
        {origins.map((origin) => (
          <OriginBadge key={origin} label={origin} />
        ))}
        {sampled.state && (
          <StatePill
            state={sampled.state}
            detail={`${sampled.coverage} segments`}
          />
        )}
        <button
          className="calibration-status"
          disabled={!connected || !!state?.playback}
          onClick={calibrate}
        >
          {readiness === "action_needed"
            ? "⚠ Calibration needed"
            : readiness === "assumptions"
              ? "◌ Heading assumed"
              : readiness === "ready"
                ? "✓ Calibration ready"
                : "Calibration —"}
        </button>
        {state?.recording && (
          <span className="recording-indicator mono">
            ● REC{" "}
            {formatDurationUs(
              sampled.timeUs === null
                ? null
                : sampled.timeUs - state.recording.started_t_us,
            )}{" "}
            <span>{state.recording.events} events</span>
            {state.recording.dropped_events > 0 && (
              <strong>⚠ {state.recording.dropped_events} dropped</strong>
            )}
          </span>
        )}
        <span className="status-protocol mono">
          v{connection.kind === "open" ? connection.engineVersion : "0.1.0"}
        </span>
      </header>
      <main>
        {connection.kind === "incompatible" ? (
          <section className="blocking-state" role="alert">
            <h1>Engine protocol is incompatible</h1>
            <p>
              The engine uses protocol {connection.engineProtocol}. This app
              supports {connection.supportedProtocol}. Update the older side to
              connect.
            </p>
          </section>
        ) : (
          <>
            {connection.kind !== "open" && (
              <div className="connection-banner" role="status">
                <span>
                  {connection.kind === "disconnected"
                    ? "Engine disconnected. Live values cleared. Retrying automatically."
                    : `Connecting to the engine at ${ENGINE_HOST}…`}
                </span>
                {connection.kind === "disconnected" && (
                  <button onClick={() => engineConnection.retryNow()}>
                    Retry now
                  </button>
                )}
              </div>
            )}
            {route === "overview" && (
              <Overview
                state={state}
                skeleton={skeleton}
                sampled={sampled}
                onCalibrate={calibrate}
              />
            )}
            {route === "studio" && (
              <Studio
                skeleton={skeleton}
                sampled={sampled}
                state={state}
                onCalibrate={calibrate}
                connected={connected}
              />
            )}
            {route === "devices" && (
              <Devices
                state={state}
                skeleton={skeleton}
                sampled={sampled}
                onCalibrate={calibrate}
              />
            )}
            {route === "cameras" && <Cameras state={state} />}
            {route === "sessions" && (
              <Sessions state={state} onRecord={() => setRecording(true)} />
            )}
          </>
        )}
      </main>
      <TransportBar state={state} onRecord={() => setRecording(true)} />
      <div
        className="visually-hidden"
        aria-live="polite"
        aria-atomic="true"
        key={announcement?.id}
      >
        {announcement?.text}
      </div>
      {calibrating && (
        <ReferencePoseFlow onClose={() => setCalibrating(false)} />
      )}
      {recording && <RecordingDialog onClose={() => setRecording(false)} />}
    </div>
  );
}
