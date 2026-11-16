import { useEffect, useRef, useState } from "react";
import { actions } from "../engine/actions";
import { useAction } from "../engine/hooks";
import { announce } from "../lib/announcer";
export function RecordingDialog({ onClose }: { onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [name, setName] = useState("");
  const [start, status] = useAction(actions.startRecording);
  useEffect(() => {
    dialog.current?.showModal();
  }, []);
  async function record() {
    const result = await start(name.trim() ? { name: name.trim() } : {});
    if (result.ok) {
      announce("Recording started.");
      onClose();
    }
  }
  return (
    <dialog ref={dialog} onCancel={onClose} aria-labelledby="recording-title">
      <h2 id="recording-title">Record a session</h2>
      <p>
        Store source observations and calibration context locally. Replays use
        the same tracking pipeline.
      </p>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void record();
        }}
      >
        <label className="recording-name">
          Session name{" "}
          <input
            autoFocus
            maxLength={120}
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Optional name"
          />
        </label>
        {status.error && (
          <p className="error" role="alert">
            {status.error}
          </p>
        )}
        <footer className="dialog-actions">
          <button type="button" onClick={onClose} disabled={status.pending}>
            Cancel
          </button>
          <button className="primary" type="submit" disabled={status.pending}>
            {status.pending ? "Starting…" : "Start recording"}
          </button>
        </footer>
      </form>
    </dialog>
  );
}
