import type { SkeletonDefinition, SystemState } from "@contracts";
import type { readouts } from "../../engine/readouts";
import { attentionReasons } from "../../engine/presentation";
import { calibrationReadiness } from "../../engine/selectors";
import { BodyMap } from "../../ui/BodyMap";
import { Reasons } from "../../ui/Reasons";
import { StatePill } from "../../ui/StatePill";
import { humanizeId } from "../../lib/format";

export function Overview({
  state,
  skeleton,
  sampled,
  onCalibrate,
}: {
  state: SystemState | null;
  skeleton: SkeletonDefinition | null;
  sampled: ReturnType<typeof readouts>;
  onCalibrate: () => void;
}) {
  const trackers =
    state?.devices.filter((d) => d.descriptor.kind !== "camera") ?? [];
  const cameras =
    state?.devices.filter((d) => d.descriptor.kind === "camera") ?? [];
  const mounting = trackers.filter((d) =>
    d.calibration.some((c) => c.kind === "mounting" && c.state === "valid"),
  ).length;
  const attention = attentionReasons(state);
  return (
    <div className="workspace-page">
      <header className="workspace-heading">
        <div>
          <span className="eyebrow">Workspace / Overview</span>
          <h1>System overview</h1>
        </div>
        <button disabled={!state} onClick={onCalibrate}>
          Calibrate mounting
        </button>
      </header>
      <div className="overview-grid">
        <section className="overview-system dock">
          <div className="panel-heading">SYSTEM</div>
          <div className="page-panel-content">
            <h2>
              {state
                ? state.devices.length === 0
                  ? "No inputs configured"
                  : `Tracking ${sampled.coverage} segments`
                : "Waiting for engine"}
            </h2>
            <p className="subtle">
              {trackers.length} trackers · {cameras.length} cameras ·{" "}
              {state?.engine.mode ?? "—"} mode
            </p>
            {state && <StatePill state={state.engine.state} />}
            <div className="section-divider" />
            <h3 className="eyebrow">Needs attention</h3>
            {attention.length ? (
              <Reasons reasons={attention} />
            ) : (
              <p className="subtle">
                {state
                  ? "No input problems reported."
                  : "Connect to the local engine to inspect its state."}
              </p>
            )}
            {calibrationReadiness(state) === "action_needed" && (
              <button className="primary" onClick={onCalibrate}>
                Capture reference pose
              </button>
            )}
          </div>
        </section>
        <section className="overview-body dock">
          <div className="panel-heading">
            BODY COVERAGE <span className="mono">{sampled.coverage}</span>
          </div>
          <BodyMap skeleton={skeleton} values={sampled.segments} />
          <div className="coverage-legend">
            <span data-prov="derived">━━ Derived</span>
            <span data-prov="inferred">┄┄ Inferred</span>
            <span>Reference view</span>
          </div>
        </section>
        <section className="overview-calibration dock">
          <div className="panel-heading">CALIBRATION</div>
          <div className="page-panel-content">
            <dl className="detail-list">
              <dt>Mounting</dt>
              <dd className="mono">
                {state ? `${mounting} / ${trackers.length}` : "—"}
              </dd>
              <dt>Readiness</dt>
              <dd>
                {calibrationReadiness(state)?.replaceAll("_", " ") ?? "—"}
              </dd>
              <dt>Body proportions</dt>
              <dd>{skeleton?.proportions ?? "—"}</dd>
            </dl>
            <p className="subtle">
              Assumed values carry that label in the engine’s state.
            </p>
          </div>
        </section>
        <section className="overview-features dock">
          <div className="panel-heading">FEATURES</div>
          <div className="feature-list">
            {state?.features.map((feature) => (
              <div className="feature-row" key={feature.id}>
                <div>
                  <span
                    className={`feature-state ${feature.state}`}
                    aria-hidden="true"
                  >
                    {feature.state === "available"
                      ? "●"
                      : feature.state === "degraded"
                        ? "◐"
                        : "○"}
                  </span>
                  <span>{humanizeId(feature.id)}</span>
                  <span className="feature-word">{feature.state}</span>
                </div>
                <Reasons reasons={feature.reasons} />
              </div>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}
