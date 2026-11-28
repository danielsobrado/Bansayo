/**
 * Training data from the engine (martial-arts 10 §4): `trainingStore` (replace-only),
 * `trainingEvents` (ring of the last 256 with gap refill) and `observablesStore` (a plain mutable
 * holder read in `requestAnimationFrame`, never React state). The UI derives nothing from them.
 */
import type { ObservableValue, TrainingEvent, TrainingState } from "@contracts";
import { createStore, type Store } from "./store";
import { trainingActions } from "./trainingActions";

export const trainingStore = createStore<TrainingState | null>(null);

/** Fetches stored events with `event_seq > afterSeq`; `null` on failure. */
export type EventFetcher = (
  sessionId: string,
  afterSeq: number,
) => Promise<TrainingEvent[] | null>;

export interface TrainingEvents {
  /** Events of the current session in `event_seq` order, at most `capacity`. */
  readonly store: Store<readonly TrainingEvent[]>;
  session(): string | null;
  /** Applies one pushed event; fetches the missing range when `event_seq` skips. */
  apply(sessionId: string, event: TrainingEvent): Promise<void>;
  /** Refills when `TrainingState.last_event_seq` is ahead, e.g. after a reconnect. */
  sync(sessionId: string, lastEventSeq: number): Promise<void>;
  clear(): void;
}

export function createTrainingEvents(
  fetchAfter: EventFetcher,
  capacity = 256,
): TrainingEvents {
  const store = createStore<readonly TrainingEvent[]>([]);
  let session: string | null = null;
  let generation = 0;
  let refilling: Promise<void> | null = null;
  let missingAfter: number | null = null;
  const last = () => store.get().at(-1)?.event_seq ?? 0;
  const insert = (incoming: readonly TrainingEvent[]) => {
    const bySeq = new Map(store.get().map((e) => [e.event_seq, e]));
    for (const e of incoming) bySeq.set(e.event_seq, e);
    store.set(
      [...bySeq.values()]
        .sort((a, b) => a.event_seq - b.event_seq)
        .slice(-capacity),
    );
  };
  const enter = (sessionId: string) => {
    if (session === sessionId) return;
    session = sessionId;
    generation++;
    refilling = null;
    missingAfter = null;
    store.set([]);
  };
  const refill = async (sessionId: string, afterSeq: number) => {
    missingAfter = missingAfter === null ? afterSeq : Math.min(missingAfter, afterSeq);
    if (refilling) return refilling;
    const epoch = generation;
    const cursor = missingAfter;
    const task = (async () => {
      try {
        const got = await fetchAfter(sessionId, cursor);
        if (generation !== epoch || session !== sessionId || !got) return;
        insert(got);
        const rows = store.get();
        let expected = rows.length === capacity
          ? Math.max(cursor + 1, rows[0]?.event_seq ?? cursor + 1)
          : cursor + 1;
        for (const event of rows) {
          if (event.event_seq < expected) continue;
          if (event.event_seq !== expected) break;
          expected++;
        }
        missingAfter = expected <= last() ? expected - 1 : null;
      } catch {
        // Keep the pending cursor for the next state update.
      }
    })();
    refilling = task;
    try {
      await task;
    } finally {
      if (generation === epoch && refilling === task) refilling = null;
    }
  };
  return {
    store,
    session: () => session,
    async apply(sessionId, event) {
      enter(sessionId);
      const before = last();
      insert([event]);
      if (event.event_seq > before + 1 || missingAfter !== null)
        await refill(sessionId, missingAfter ?? before);
    },
    async sync(sessionId, lastEventSeq) {
      enter(sessionId);
      if (lastEventSeq > last() || missingAfter !== null)
        await refill(sessionId, missingAfter ?? last());
    },
    clear() {
      session = null;
      generation++;
      refilling = null;
      missingAfter = null;
      store.set([]);
    },
  };
}

/** The application's event ring, refilled from the stored or live session log. */
export const trainingEvents = createTrainingEvents(async (id, after) => {
  const r = await trainingActions.events(id, after);
  return r.ok ? r.data : null;
});

/** Latest observable values. A plain holder like `frameStore`: gauges poll it. */
export const observablesStore = {
  seq: -1,
  tUs: 0,
  /** `performance.now()` when the values were applied. */
  receivedAt: 0,
  values: new Map<string, ObservableValue>(),
  apply(seq: number, tUs: number, values: ObservableValue[], now: number): void {
    if (seq <= this.seq) return;
    this.seq = seq;
    this.tUs = tUs;
    this.receivedAt = now;
    this.values = new Map(values.map((v) => [v.id, v]));
  },
  clear(): void {
    this.seq = -1;
    this.tUs = 0;
    this.receivedAt = 0;
    this.values = new Map();
  },
};
