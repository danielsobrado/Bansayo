import { useEffect, useRef, useState } from "react";
import type { CalibrationProfileSummary } from "@contracts";
import { actions } from "../engine/actions";
import { useAction, useSystemState } from "../engine/hooks";
import { announce } from "../lib/announcer";

const COUNTDOWN_SECONDS = 3;
export function ReferencePoseFlow({ onClose }: { onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const mounted = useRef(true);
  const simulation = useSystemState((s) => s?.simulation ?? null);
  const [capture, status] = useAction(actions.captureReferencePose);
  const [reference, referenceStatus] = useAction(actions.updateSimulation);
  const [countdown, setCountdown] = useState<number | null>(null);
  const [result, setResult] = useState<CalibrationProfileSummary[] | null>(
    null,
  );
  const busy = countdown !== null || status.pending || referenceStatus.pending;
  useEffect(() => {
    mounted.current = true;
    dialog.current?.showModal();
    return () => {
      mounted.current = false;
      dialog.current?.close();
    };
  }, []);
  async function run() {
    setResult(null);
    for (let value = COUNTDOWN_SECONDS; value > 0; value--) {
      setCountdown(value);
      await new Promise((resolve) => setTimeout(resolve, 1000));
      if (!mounted.current) return;
    }
    setCountdown(null);
    const response = await capture({ device_ids: null });
    if (response.ok && mounted.current) {
      setResult(response.data);
      announce(
        `Mounting calibration captured for ${response.data.length} trackers.`,
      );
    }
  }
  return (
    <dialog
      ref={dialog}
      onCancel={(event) => {
        if (busy) event.preventDefault();
        else onClose();
      }}
      aria-labelledby="calibration-title"
    >
      <header className="dialog-heading">
        <span className="eyebrow">Mounting calibration</span>
        <button
          onClick={onClose}
          aria-label="Close calibration"
          disabled={busy}
        >
          ×
        </button>
      </header>
      <h2 id="calibration-title">Capture a reference pose</h2>
      <p>
        Stand upright, face forward and extend both arms into a T-pose. Hold
        still during capture.
      </p>
      <p className="subtle">
        For simulated trackers, select the reference motion first. This records
        the rotation between each tracker and its assigned body segment.
      </p>
      {simulation && (
        <button
          onClick={() => void reference({ motion: "reference_pose" })}
          disabled={busy}
        >
          Use reference motion
        </button>
      )}
      <p className="calibration-note">
        Heading remains assumed. Joint measurements have not been validated
        against a reference instrument.
      </p>
      {countdown !== null && (
        <div className="capture-progress" role="status">
          <p>Reference pose in {countdown}…</p>
          <progress
            max={COUNTDOWN_SECONDS}
            value={COUNTDOWN_SECONDS - countdown}
          />
        </div>
      )}
      {status.pending && (
        <div className="capture-progress" role="status">
          <p>Capturing · hold still…</p>
          <progress aria-label="Capturing reference pose" />
        </div>
      )}
      {(status.error || referenceStatus.error) && (
        <p className="error" role="alert">
          {status.error || referenceStatus.error}
        </p>
      )}
      {result !== null && (
        <>
          <p className="success" role="status">
            Mounting captured for {result.length} trackers.
          </p>
          <ul className="capture-results">
            {result.map((profile) => (
              <li key={profile.id}>
                <span className="mono">{profile.device_id ?? profile.id}</span>
                <span>✓ {profile.state}</span>
              </li>
            ))}
          </ul>
        </>
      )}
      <footer className="dialog-actions">
        <button onClick={onClose} disabled={busy}>
          Close
        </button>
        <button className="primary" onClick={() => void run()} disabled={busy}>
          {busy
            ? "Hold still…"
            : result
              ? "Recapture mounting"
              : "Capture mounting"}
        </button>
      </footer>
    </dialog>
  );
}
