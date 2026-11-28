import { describe, expect, it } from "vitest";
import type { ApiResponse, TrainingEvent } from "@contracts";
import { parseServerMessage } from "./messages";
import { createQueryCache } from "./queries";
import { createTrainingEvents, observablesStore } from "./training";

const ev = (seq: number, type = "phase_changed"): TrainingEvent => ({
  event_seq: seq,
  t_us: seq * 1000,
  type,
  position: { block: 0, item: 0, set: 1, round: null, step: null },
  data: {},
});

describe("trainingEvents", () => {
  it("refills a gap from the stored log and keeps order without duplicates", async () => {
    const asked: number[] = [];
    const events = createTrainingEvents(async (_id, after) => {
      asked.push(after);
      return [ev(2), ev(3), ev(4)];
    });
    await events.apply("s1", ev(1));
    await events.apply("s1", ev(5));
    expect(asked).toEqual([1]);
    expect(events.store.get().map((e) => e.event_seq)).toEqual([1, 2, 3, 4, 5]);
    await events.apply("s1", ev(5));
    expect(events.store.get()).toHaveLength(5);
  });

  it("keeps the last 256, resets on a new session and syncs after reconnect", async () => {
    let calls = 0;
    const events = createTrainingEvents(async (_id, after) => {
      calls++;
      return Array.from({ length: 300 - after }, (_, i) => ev(after + 1 + i));
    });
    await events.sync("s1", 300);
    expect(calls).toBe(1);
    expect(events.store.get()).toHaveLength(256);
    expect(events.store.get()[0]!.event_seq).toBe(45);
    await events.sync("s1", 300);
    expect(calls).toBe(1);
    await events.apply("s2", ev(1));
    expect(events.session()).toBe("s2");
    expect(events.store.get().map((e) => e.event_seq)).toEqual([1]);
  });

  it("retries a missing range even after the newest event arrives", async () => {
    let attempts = 0;
    const events = createTrainingEvents(async () => ++attempts === 1 ? null : [ev(1), ev(2), ev(3)]);
    await events.apply("s1", ev(4));
    await events.sync("s1", 4);
    expect(events.store.get().map(e => e.event_seq)).toEqual([1, 2, 3, 4]);
  });
  it("does not let a previous session block or contaminate a new refill", async () => {
    let finish!: (events: TrainingEvent[]) => void;
    const events = createTrainingEvents(async id => id === "s1"
      ? new Promise<TrainingEvent[]>(resolve => { finish = resolve; })
      : [ev(1), ev(2)]);
    const old = events.apply("s1", ev(5));
    await events.apply("s2", ev(3));
    finish([ev(4)]);
    await old;
    expect(events.store.get().map(e => e.event_seq)).toEqual([1, 2, 3]);
  });
  it("a failed refill keeps what arrived", async () => {
    const events = createTrainingEvents(async () => null);
    await events.apply("s1", ev(4));
    expect(events.store.get().map((e) => e.event_seq)).toEqual([4]);
  });
});

describe("observablesStore", () => {
  it("applies newer sequences only", () => {
    observablesStore.clear();
    const value = { id: "knee_flexion.right", value: 1, norm: null, evidence: {} as never, fresh: true, reasons: [] };
    observablesStore.apply(2, 10, [value], 0);
    observablesStore.apply(1, 5, [], 0);
    expect(observablesStore.values.get("knee_flexion.right")?.value).toBe(1);
    expect(observablesStore.tUs).toBe(10);
  });
});

describe("queryCache", () => {
  it("memoises successes, retries failures and invalidates by prefix", async () => {
    let n = 0;
    const cache = createQueryCache(async (path): Promise<ApiResponse<unknown>> => {
      n++;
      return path.includes("bad")
        ? { ok: false, error: { code: "unavailable", message: "x", details: null } }
        : { ok: true, data: n };
    });
    expect(await cache.get("/training/a")).toEqual({ ok: true, data: 1 });
    expect(await cache.get("/training/a")).toEqual({ ok: true, data: 1 });
    await cache.get("/bad");
    await cache.get("/bad");
    expect(n).toBe(3);
    cache.invalidate("/training/");
    expect(await cache.get("/training/a")).toEqual({ ok: true, data: 4 });
    expect(cache.version.get()).toBe(1);
  });
});

describe("training messages", () => {
  it("accepts training topics and rejects malformed ones", () => {
    expect(parseServerMessage(JSON.stringify({ type: "training_state", state: null })).kind).toBe("message");
    expect(
      parseServerMessage(JSON.stringify({ type: "training_event", session_id: "s", event: ev(1) })).kind,
    ).toBe("message");
    expect(
      parseServerMessage(JSON.stringify({ type: "training_event", session_id: "s", event: { event_seq: "1" } })).kind,
    ).toBe("invalid");
    expect(
      parseServerMessage(
        JSON.stringify({ type: "observables", seq: 1, t_us: 2, values: [{ id: "a", value: null, reasons: [] }] }),
      ).kind,
    ).toBe("message");
    expect(parseServerMessage(JSON.stringify({ type: "training_state" })).kind).toBe("invalid");
  });
});
