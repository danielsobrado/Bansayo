/**
 * Rolling history of every joint value for the inspector trace. Fixed-size typed ring buffers,
 * filled from each applied frame without allocating.
 */
import type { JointMeasurement } from "@contracts";

/** Width of the trace window. */
export const TRACE_WINDOW_US = 10_000_000;
/** Ring capacity per joint; covers the window at up to 200 frames per second. */
const CAPACITY = 2048;

interface FrameLike {
  t_us: number;
  joints: JointMeasurement[];
}

class JointHistory {
  readonly times = new Float64Array(CAPACITY);
  private values: Float32Array[] = [];
  /** Index of the next write. */
  head = 0;
  count = 0;

  push(frame: FrameLike): void {
    const n = frame.joints.length;
    if (n !== this.values.length) this.reset(n);
    this.times[this.head] = frame.t_us;
    for (let j = 0; j < n; j++) {
      const v = frame.joints[j]?.value;
      this.values[j]![this.head] =
        v === null || v === undefined ? Number.NaN : v;
    }
    this.head = (this.head + 1) % CAPACITY;
    if (this.count < CAPACITY) this.count++;
  }

  /** Values of joint `index` (NaN = unavailable), index-aligned with `times`; null if unknown. */
  valuesOf(index: number): Float32Array | null {
    return this.values[index] ?? null;
  }

  get capacity(): number {
    return CAPACITY;
  }

  clear(): void {
    this.head = 0;
    this.count = 0;
  }

  private reset(jointCount: number): void {
    this.values = Array.from(
      { length: jointCount },
      () => new Float32Array(CAPACITY),
    );
    this.clear();
  }
}

export const jointHistory = new JointHistory();
