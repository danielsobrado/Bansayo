import { useEffect, useState } from "react";
import QRCode from "qrcode";
import { request } from "../../engine/http";

interface Pairing {
  enabled: boolean;
  host?: string | null;
  port?: number;
  token?: string;
  uri?: string | null;
  warning?: string;
  message?: string;
}

/**
 * Phone pairing (ADR-042): the engine's LAN-mode link as a QR code for the Android app, served
 * only to this machine. The token never appears anywhere else.
 */
export function PhonePairing() {
  const [pairing, setPairing] = useState<Pairing | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [qr, setQr] = useState<string | null>(null);
  const [shown, setShown] = useState(false);

  useEffect(() => {
    void request<Pairing>("GET", "/lan/pairing").then((r) => {
      if (r.ok) setPairing(r.data);
      else setError(r.error.message);
    });
  }, []);

  useEffect(() => {
    if (!shown || !pairing?.uri) return;
    // Inline SVG: the app's content security policy allows no data: images.
    void QRCode.toString(pairing.uri, { type: "svg", margin: 1, width: 220 }).then(setQr);
  }, [shown, pairing?.uri]);

  return (
    <div className="page-panel-content" aria-label="Phone pairing">
      <h3 className="eyebrow">Phone pairing</h3>
      {error && <p className="subtle">{error}</p>}
      {pairing && !pairing.enabled && <p className="subtle">{pairing.message}</p>}
      {pairing?.enabled && !pairing.host && (
        <p className="subtle">LAN mode is on, but this PC has no network address to show.</p>
      )}
      {pairing?.enabled && pairing.host && (
        <>
          <p className="subtle">
            Scan with the phone's camera to open the Bansayo app paired with this engine. Phone and PC must be on the
            same Wi-Fi.
          </p>
          <p className="calibration-note">{pairing.warning}</p>
          {!shown ? (
            <button onClick={() => setShown(true)}>Show pairing code</button>
          ) : (
            <>
              {qr && (
                <div
                  role="img"
                  aria-label="Pairing QR code for the Bansayo Android app"
                  style={{ width: 220, height: 220, background: "#fff" }}
                  // The SVG is generated locally from the engine's pairing link: path data only.
                  dangerouslySetInnerHTML={{ __html: qr }}
                />
              )}
              <dl className="mono">
                <dt>Address</dt>
                <dd>{pairing.host}</dd>
                <dt>Port</dt>
                <dd>{pairing.port}</dd>
                <dt>Token</dt>
                <dd>{pairing.token}</dd>
              </dl>
              <button onClick={() => setShown(false)}>Hide</button>
            </>
          )}
        </>
      )}
    </div>
  );
}
