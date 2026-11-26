import { useEffect, useState } from "react";
import { request, seg } from "../../../engine/http";
import { parseSnapshot, POLL_MS, type Snapshot } from "./trace";

interface Reading {
  snapshot: Snapshot | null;
  error: string | null;
  receivedAt: number;
}
const EMPTY: Reading = { snapshot: null, error: null, receivedAt: 0 };

export function useInspection(id: string, enabled: boolean, frozen: boolean) {
  const [reading, setReading] = useState<Reading>(EMPTY);
  // Re-render even during a hung request so freshness expires on time.
  const [,setHeartbeat] = useState(0);
  useEffect(()=>{
    if(!enabled || frozen)return;
    const timer=window.setInterval(()=>setHeartbeat(n=>n+1),POLL_MS);
    return ()=>window.clearInterval(timer);
  },[enabled,frozen]);
  useEffect(() => {
    if (!enabled || frozen) return;
    const controller = new AbortController();
    let timer: number | undefined;
    async function poll() {
      if (controller.signal.aborted) return;
      if (!document.hidden) {
        const requestedAt = performance.now();
        const res = await request<unknown>("GET", `/devices/${seg(id)}/inspection`, undefined,
          (url, init) => fetch(url, { ...init, cache: "no-store", signal: AbortSignal.any([controller.signal, AbortSignal.timeout(2000)]) }));
        if (controller.signal.aborted) return;
        if (res.ok) {
          const snapshot = parseSnapshot(res.data);
          setReading(snapshot ? { snapshot, error: null, receivedAt: requestedAt }
            : { ...EMPTY, error: "Invalid diagnostic data" });
        } else setReading({ ...EMPTY, error: res.error.message });
      }
      timer = window.setTimeout(poll, POLL_MS);
    }
    void poll();
    return () => { controller.abort(); window.clearTimeout(timer); };
  }, [id, enabled, frozen]);
  return enabled ? reading : EMPTY;
}
