/**
 * Client-side copies of what the engine reports. The connection writes them; components read them
 * through hooks; the render loop reads `frameStore` directly.
 */
import type {
  DeviceId,
  PoseLandmarks2D,
  ServerMessage,
  SkeletonDefinition,
  SystemState,
} from "@contracts";
import { createStore } from "./store";
import { jointHistory } from "./jointHistory";

/** A frame older than this is stale: the skeleton dims and the viewport says there is no pose data. */
export const STALE_FRAME_MS = 500;

export type FrameMessage = Extract<ServerMessage, { type: "frame" }>;

export type ConnectionStatus =
  | { kind: "connecting"; attempt: number }
  | { kind: "open"; engineVersion: string; protocolVersion: number }
  | { kind: "disconnected"; retryAt: number }
  | { kind: "incompatible"; engineProtocol: number; supportedProtocol: number };

export const connectionStore = createStore<ConnectionStatus>({
  kind: "connecting",
  attempt: 0,
});
export const systemStateStore = createStore<SystemState | null>(null);
export const skeletonStore = createStore<SkeletonDefinition | null>(null);
export const landmarksStore = createStore<
  ReadonlyMap<DeviceId, PoseLandmarks2D>
>(new Map());

/**
 * Latest frame. A plain mutable holder, not React state: the render loop polls it every animation
 * frame, and React reads it only through `useSampledFrame`.
 */
export const frameStore = {
  frame: null as FrameMessage | null,
  /** `performance.now()` when `frame` was applied. */
  receivedAt: 0,
  lastSeq: -1,

  /** Applies a frame unless its `seq` is not greater than the last applied one. Returns whether it was applied. */
  apply(frame: FrameMessage, now: number): boolean {
    if (frame.seq <= this.lastSeq) return false;
    if (this.frame && frame.t_us < this.frame.t_us) jointHistory.clear();
    this.lastSeq = frame.seq;
    this.frame = frame;
    this.receivedAt = now;
    jointHistory.push(frame);
    return true;
  },

  clear(): void {
    this.invalidate();
    this.lastSeq = -1;
  },

  /** A new timeline/definition clears readouts without weakening sequence ordering. */
  invalidate(): void {
    this.frame = null;
    this.receivedAt = 0;
    jointHistory.clear();
  },
};

/** Clears every live value. Called on disconnect and before a new `hello`. */
export function clearLiveValues(): void {
  systemStateStore.set(null);
  landmarksStore.set(new Map());
  frameStore.clear();
}
