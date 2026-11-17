/**
 * The WebSocket stream: lifecycle, capped-backoff reconnect, protocol check, and dispatch into
 * the stores. Server push only; nothing is ever sent.
 */
import { announce } from "../lib/announcer";
import { STREAM_URL, SUPPORTED_PROTOCOL_VERSION } from "./config";
import { isCompleteHello, parseServerMessage } from "./messages";
import {
  clearLandmarks,
  clearLiveValues,
  connectionStore,
  frameStore,
  landmarks3dStore,
  landmarksReceivedAt,
  landmarksStore,
  skeletonStore,
  systemStateStore,
} from "./stores";
import { contentCache, queryCache } from "./queries";
import { observablesStore, trainingEvents, trainingStore } from "./training";
import type { ServerMessage } from "@contracts";

/** Live-region text for a counted repetition; the count is the engine's. */
function announceRep(data: Record<string, unknown>): void {
  const rep = data.repetition as { rep_index?: unknown; outcome?: unknown } | undefined;
  if (rep && typeof rep.rep_index === "number")
    announce(`Repetition ${rep.rep_index}: ${String(rep.outcome ?? "")}`.trim());
}

export const BACKOFF_INITIAL_MS = 500;
export const BACKOFF_MAX_MS = 8_000;

/** The part of `WebSocket` the connection uses, so tests can supply a fake. */
export interface SocketLike {
  onopen: ((ev: Event) => void) | null;
  onmessage: ((ev: MessageEvent<unknown>) => void) | null;
  onclose: ((ev: CloseEvent) => void) | null;
  onerror: ((ev: Event) => void) | null;
  close(): void;
}

export interface ConnectionDeps {
  createSocket: (url: string) => SocketLike;
  /** Wall clock, for the retry countdown. */
  now: () => number;
  /** Monotonic clock, for frame age. */
  monotonic: () => number;
  setTimer: (fn: () => void, ms: number) => unknown;
  clearTimer: (handle: unknown) => void;
}

const browserDeps = (): ConnectionDeps => ({
  createSocket: (url) => new WebSocket(url),
  now: () => Date.now(),
  monotonic: () => performance.now(),
  setTimer: (fn, ms) => setTimeout(fn, ms),
  clearTimer: (h) => clearTimeout(h as ReturnType<typeof setTimeout>),
});

export function backoffDelay(attempt: number): number {
  return Math.min(BACKOFF_MAX_MS, BACKOFF_INITIAL_MS * 2 ** attempt);
}

export class EngineConnection {
  private socket: SocketLike | null = null;
  private timer: unknown = null;
  private attempt = 0;
  private greeted = false;
  private stopped = true;

  constructor(
    private readonly url: string = STREAM_URL,
    private readonly deps: ConnectionDeps = browserDeps(),
  ) {}

  start(): void {
    if (!this.stopped) return;
    this.stopped = false;
    this.connect();
  }

  stop(): void {
    this.stopped = true;
    this.cancelTimer();
    this.detach();
  }

  /** Skips the remaining backoff. */
  retryNow(): void {
    if (this.stopped || connectionStore.get().kind !== "disconnected") return;
    this.cancelTimer();
    this.connect();
  }

  /** Handles one text frame. Public for tests. */
  handleText(text: string): void {
    const parsed = parseServerMessage(text);
    // Unknown types and malformed messages are ignored (docs/04 §2).
    if (parsed.kind !== "message") return;
    this.handleMessage(parsed.message);
  }

  private handleMessage(m: ServerMessage): void {
    if (m.type === "hello") {
      this.onHello(m);
      return;
    }
    if (!this.greeted) return; // `hello` is always first; anything earlier is not ours to apply.
    switch (m.type) {
      case "frame":
        frameStore.apply(m, this.deps.monotonic());
        break;
      case "state":
        // Live and replay are different timelines: nothing from one may show in the other.
        if (systemStateStore.get()?.engine.mode !== m.state.engine.mode) {
          frameStore.invalidate();
          clearLandmarks();
        }
        systemStateStore.set(m.state);
        contentCache.syncRev(m.state.content_rev);
        break;
      case "skeleton":
        if (skeletonStore.get()?.rev !== m.skeleton.rev)
          frameStore.invalidate();
        skeletonStore.set(m.skeleton);
        break;
      case "landmarks_2d": {
        const next = new Map(landmarksStore.get());
        next.set(m.landmarks.frame_meta.device_id, m.landmarks);
        landmarksReceivedAt.set(
          m.landmarks.frame_meta.device_id,
          this.deps.monotonic(),
        );
        landmarksStore.set(next);
        break;
      }
      case "landmarks_3d":
        // Latest set only: replay seeks may move time backwards and simply replace it.
        if (m.landmarks.kind === "triangulated") {
          landmarks3dStore.landmarks = m.landmarks;
          landmarks3dStore.receivedAt = this.deps.monotonic();
        }
        break;
      case "notice":
        announce(m.message);
        break;
      case "training_state":
        trainingStore.set(m.state);
        if (m.state)
          void trainingEvents.sync(m.state.session_id, m.state.last_event_seq);
        break;
      case "training_event":
        void trainingEvents.apply(m.session_id, m.event);
        // Stored rows of a finished session change: re-read them (10 §4).
        if (m.event.type === "session_completed" || m.event.type === "session_aborted")
          queryCache.invalidate("/training/", "/progress/");
        if (m.event.type === "rep_completed") announceRep(m.event.data);
        break;
      case "observables":
        observablesStore.apply(m.seq, m.t_us, m.values, this.deps.monotonic());
        break;
    }
  }

  private onHello(m: Extract<ServerMessage, { type: "hello" }>): void {
    if (m.protocol_version !== SUPPORTED_PROTOCOL_VERSION) {
      clearLiveValues();
      skeletonStore.set(null);
      connectionStore.set({
        kind: "incompatible",
        engineProtocol: m.protocol_version,
        supportedProtocol: SUPPORTED_PROTOCOL_VERSION,
      });
      announce(
        `Incompatible engine: protocol ${m.protocol_version}, this app supports ${SUPPORTED_PROTOCOL_VERSION}.`,
      );
      // Reconnecting cannot fix a version mismatch; the user must update one side.
      this.stop();
      return;
    }
    if (!isCompleteHello(m)) return;
    clearLiveValues();
    skeletonStore.set(m.skeleton);
    systemStateStore.set(m.state);
    this.greeted = true;
    this.attempt = 0;
    connectionStore.set({
      kind: "open",
      engineVersion: m.engine_version,
      protocolVersion: m.protocol_version,
    });
    announce("Connected to the engine.");
  }

  private connect(): void {
    this.detach();
    this.greeted = false;
    connectionStore.set({ kind: "connecting", attempt: this.attempt });
    let socket: SocketLike;
    try {
      socket = this.deps.createSocket(this.url);
    } catch {
      this.onClosed();
      return;
    }
    socket.onmessage = (ev) => {
      if (typeof ev.data === "string") this.handleText(ev.data);
    };
    socket.onclose = () => this.onClosed();
    socket.onerror = () => {
      /* `close` follows `error`; recovery happens there. */
    };
    this.socket = socket;
  }

  private onClosed(): void {
    this.socket = null;
    if (this.stopped || connectionStore.get().kind === "incompatible") return;
    const wasOpen = connectionStore.get().kind === "open";
    this.greeted = false;
    clearLiveValues();
    const delay = backoffDelay(this.attempt);
    this.attempt++;
    connectionStore.set({
      kind: "disconnected",
      retryAt: this.deps.now() + delay,
    });
    if (wasOpen) announce("Engine disconnected. Live values cleared.");
    this.timer = this.deps.setTimer(() => {
      this.timer = null;
      this.connect();
    }, delay);
  }

  private detach(): void {
    const s = this.socket;
    if (!s) return;
    this.socket = null;
    s.onmessage = null;
    s.onclose = null;
    s.onerror = null;
    s.close();
  }

  private cancelTimer(): void {
    if (this.timer !== null) this.deps.clearTimer(this.timer);
    this.timer = null;
  }
}

/** The application's single connection. */
export const engineConnection = new EngineConnection();
