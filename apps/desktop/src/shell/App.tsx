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
import { useHashRoute } from "../lib/hashRoute";
import { OriginBadge } from "../ui/OriginBadge";
import { StatePill } from "../ui/StatePill";
import { ReferencePoseFlow } from "../calibration/ReferencePoseFlow";
import { Studio } from "../workspaces/studio/Studio";
import { Overview } from "../workspaces/overview/Overview";
import { Devices } from "../workspaces/devices/Devices";
import { Cameras } from "../workspaces/cameras/Cameras";
import { Sessions } from "../workspaces/sessions/Sessions";
import { Train } from "../workspaces/train/Train";
import { Technique } from "../workspaces/technique/Technique";
import { Progress } from "../workspaces/progress/Progress";
import { CapabilityBadge } from "../ui/CapabilityBadge";
import { TransportBar } from "./TransportBar";
import { WorkbenchMenu } from "./WorkbenchMenu";
import { usePreferences } from "./preferences";
import { Icon } from "../ui/Icon";
import { RecordingDialog } from "./RecordingDialog";

// Keys follow martial-arts 10 §2; Workouts (3) and Calibration (9) appear once implemented.
const NAV = [
  { id: "train", label: "Train", icon: "play", key: "1" },
  { id: "technique", label: "Technique", icon: "target", key: "2" },
  { id: "progress", label: "Progress", icon: "chart", key: "4" },
  { id: "overview", label: "Overview", icon: "monitor", key: "5" },
  { id: "studio", label: "Studio", icon: "crosshair", key: "6" },
  { id: "devices", label: "Devices", icon: "usb", key: "7" },
  { id: "cameras", label: "Cameras", icon: "camera", key: "8" },
  { id: "sessions", label: "Sessions", icon: "folder", key: "0" },
] as const;
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
  const { preferences, update: updatePreferences } = usePreferences();
  const { workspace: route, rest } = useHashRoute(WORKSPACES, "train");
  useEffect(() => {
    if (!location.hash) location.hash = "#/train";
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
    <div className={`app-shell ${preferences.rail === "expanded" ? "rail-expanded" : ""}`}>
      <WorkbenchMenu items={NAV} active={route} connected={connected}
        preferences={preferences} onChange={updatePreferences} onCalibrate={calibrate} />
      <nav className="navigation" aria-label="Workspaces">
        <div className="brand">
          <span className="brand-mark">B</span>
          <span className="brand-name">
            bansayo<span>motion platform</span>
          </span>
        </div>
        <button type="button" className="rail-toggle"
          title={preferences.rail === "compact" ? "Expand navigation" : "Collapse navigation"}
          aria-label={preferences.rail === "compact" ? "Expand navigation" : "Collapse navigation"}
          onClick={() => updatePreferences({ rail: preferences.rail === "compact" ? "expanded" : "compact" })}>
          <Icon name="panel-left" />
        </button>
        <span className="rail-label">Workspace</span>
        {NAV.map((item) => (
          <a
            key={item.id}
            href={`#/${item.id}`}
            aria-current={route === item.id ? "page" : undefined}
            title={`${item.label} (${item.key})`}
          >
            <span className="nav-icon" aria-hidden="true">
              <Icon name={item.icon} />
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
        {state?.athlete && (
          <a className="mono" href="#/train" title="Active athlete">
            {state.athlete.display_name}
          </a>
        )}
        <CapabilityBadge report={state?.capability_report} />
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
            {route === "train" && <Train state={state} rest={rest} />}
            {route === "technique" && <Technique state={state} rest={rest} />}
            {route === "progress" && <Progress state={state} rest={rest} />}
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
