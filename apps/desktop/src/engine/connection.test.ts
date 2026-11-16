import { beforeEach, describe, expect, it } from "vitest";
import hello from "../../../../contracts/examples/hello.json";
import frame from "../../../../contracts/examples/frame.json";
import { EngineConnection, backoffDelay, type SocketLike } from "./connection";
import { jointHistory } from "./jointHistory";
import {
  clearLiveValues,
  connectionStore,
  frameStore,
  skeletonStore,
  systemStateStore,
} from "./stores";

class FakeSocket implements SocketLike {
  onopen: ((ev: Event) => void) | null = null;
  onmessage: ((ev: MessageEvent<unknown>) => void) | null = null;
  onclose: ((ev: CloseEvent) => void) | null = null;
  onerror: ((ev: Event) => void) | null = null;
  closed = false;
  close() {
    this.closed = true;
  }
  disconnect() {
    this.onclose?.({} as CloseEvent);
  }
}
function setup() {
  const sockets: FakeSocket[] = [];
  const timers: (() => void)[] = [];
  const connection = new EngineConnection("ws://test", {
    createSocket: () => {
      const socket = new FakeSocket();
      sockets.push(socket);
      return socket;
    },
    now: () => 1000,
    monotonic: () => 20,
    setTimer: (fn) => {
      timers.push(fn);
      return fn;
    },
    clearTimer: (fn) => {
      const i = timers.indexOf(fn as () => void);
      if (i >= 0) timers.splice(i, 1);
    },
  });
  connection.start();
  return { connection, sockets, timers };
}
beforeEach(() => {
  clearLiveValues();
  skeletonStore.set(null);
  connectionStore.set({ kind: "connecting", attempt: 0 });
});
describe("connection and live data lifecycle", () => {
  it("accepts the golden hello and drops duplicate or out-of-order frames", () => {
    const { connection } = setup();
    connection.handleText(JSON.stringify(frame));
    expect(frameStore.frame).toBeNull();
    connection.handleText(JSON.stringify(hello));
    expect(connectionStore.get().kind).toBe("open");
    expect(skeletonStore.get()?.segments.length).toBe(19);
    connection.handleText(JSON.stringify(frame));
    expect(frameStore.frame?.seq).toBe(frame.seq);
    connection.handleText(JSON.stringify({ ...frame, seq: frame.seq - 1 }));
    expect(frameStore.frame?.seq).toBe(frame.seq);
    connection.stop();
  });
  it("clears live values on disconnect and resets the sequence after reconnect", () => {
    const { connection, sockets, timers } = setup();
    connection.handleText(JSON.stringify(hello));
    connection.handleText(JSON.stringify(frame));
    sockets[0]!.disconnect();
    expect(frameStore.frame).toBeNull();
    expect(systemStateStore.get()).toBeNull();
    expect(connectionStore.get()).toEqual({
      kind: "disconnected",
      retryAt: 1500,
    });
    timers.shift()!();
    connection.handleText(JSON.stringify(hello));
    connection.handleText(JSON.stringify({ ...frame, seq: 0 }));
    expect(frameStore.frame?.seq).toBe(0);
    connection.stop();
  });
  it("blocks a mismatched protocol without retrying", () => {
    const { connection, sockets, timers } = setup();
    connection.handleText(JSON.stringify({ ...hello, protocol_version: 99 }));
    expect(connectionStore.get().kind).toBe("incompatible");
    expect(sockets[0]!.closed).toBe(true);
    expect(timers).toHaveLength(0);
    expect(systemStateStore.get()).toBeNull();
  });
  it("clears traces and readouts on engine mode and skeleton revision changes without resetting seq", () => {
    const { connection } = setup();
    connection.handleText(JSON.stringify(hello));
    connection.handleText(JSON.stringify(frame));
    connection.handleText(
      JSON.stringify({
        type: "state",
        state: {
          ...hello.state,
          engine: { ...hello.state.engine, mode: "replay" },
        },
      }),
    );
    expect(jointHistory.count).toBe(0);
    expect(frameStore.frame).toBeNull();
    expect(frameStore.lastSeq).toBe(frame.seq);
    connection.handleText(JSON.stringify({ ...frame, seq: frame.seq + 1 }));
    connection.handleText(
      JSON.stringify({
        type: "skeleton",
        skeleton: { ...hello.skeleton, rev: 2 },
      }),
    );
    expect(jointHistory.count).toBe(0);
    expect(frameStore.frame).toBeNull();
    expect(frameStore.lastSeq).toBe(frame.seq + 1);
    connection.stop();
  });
  it("caps exponential reconnect backoff", () => {
    expect([0, 1, 4, 20].map(backoffDelay)).toEqual([500, 1000, 8000, 8000]);
  });
});
