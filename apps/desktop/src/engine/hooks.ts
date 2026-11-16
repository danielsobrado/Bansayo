/** React access to the engine stores. Low-rate only: pose data is read by the render loop. */
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
} from "react";
import type { ApiResponse, SkeletonDefinition, SystemState } from "@contracts";
import { announce } from "../lib/announcer";
import {
  connectionStore,
  frameStore,
  skeletonStore,
  systemStateStore,
  type ConnectionStatus,
  type FrameMessage,
} from "./stores";

/** Selects from the system state; recomputed only when the state or the selector changes. */
export function useSystemState<T>(select: (s: SystemState | null) => T): T {
  const cache = useRef<{ s: SystemState | null; select: unknown; v: T } | null>(
    null,
  );
  return useSyncExternalStore(systemStateStore.subscribe, () => {
    const s = systemStateStore.get();
    const c = cache.current;
    if (c && c.s === s && c.select === select) return c.v;
    const v = select(s);
    cache.current = { s, select, v };
    return v;
  });
}

export function useConnection(): ConnectionStatus {
  return useSyncExternalStore(connectionStore.subscribe, connectionStore.get);
}

export function useSkeleton(): SkeletonDefinition | null {
  return useSyncExternalStore(skeletonStore.subscribe, skeletonStore.get);
}

/**
 * Samples the latest frame at `hz` and returns what `select` takes from it. `ageMs` is the time
 * since the frame was received. Selectors should return readouts, not pose geometry.
 */
export function useSampledFrame<T>(
  hz: number,
  select: (frame: FrameMessage | null, ageMs: number) => T,
): T {
  const read = (): T => {
    const f = frameStore.frame;
    return select(
      f,
      f ? performance.now() - frameStore.receivedAt : Number.POSITIVE_INFINITY,
    );
  };
  const [value, setValue] = useState(read);
  const readRef = useRef(read);
  readRef.current = read;
  useEffect(() => {
    const id = setInterval(() => setValue(readRef.current()), 1000 / hz);
    return () => clearInterval(id);
  }, [hz]);
  return value;
}

export interface ActionStatus {
  pending: boolean;
  /** The engine's message for the last failed call, cleared by the next call. */
  error: string | null;
}

/** Runs an action, tracks pending state, and reports a failure next to the control and aloud. */
export function useAction<A extends unknown[], T>(
  fn: (...args: A) => Promise<ApiResponse<T>>,
): [(...args: A) => Promise<ApiResponse<T>>, ActionStatus] {
  const [status, setStatus] = useState<ActionStatus>({
    pending: false,
    error: null,
  });
  const run = useCallback(
    async (...args: A) => {
      setStatus({ pending: true, error: null });
      const res = await fn(...args);
      if (res.ok) {
        setStatus({ pending: false, error: null });
      } else {
        setStatus({ pending: false, error: res.error.message });
        announce(res.error.message);
      }
      return res;
    },
    [fn],
  );
  return [run, status];
}
