/**
 * Plays a stored motion clip (10 §5.1): play, pause, speed, scrub, loop. Positions are the
 * engine's; the player only picks the sample at the current time. Not engine replay; no kinematics.
 */
import type { MotionClip, SkeletonPose } from "@contracts";

export class ClipPlayer {
  playing = false;
  speed = 1;
  loop = true;
  private positionUs: number;
  private lastNow: number | null = null;

  constructor(private readonly clip: MotionClip) {
    this.positionUs = this.startUs;
  }

  get startUs(): number {
    return this.clip.samples[0]?.t_us ?? 0;
  }
  get endUs(): number {
    return this.clip.samples.at(-1)?.t_us ?? 0;
  }
  get timeUs(): number {
    return this.positionUs;
  }

  play(): void {
    if (this.positionUs >= this.endUs) this.positionUs = this.startUs;
    this.playing = true;
    this.lastNow = null;
  }
  pause(): void {
    this.playing = false;
  }
  seek(tUs: number): void {
    this.positionUs = Math.min(this.endUs, Math.max(this.startUs, tUs));
  }

  /** Advances by wall time `now` (ms) and returns the current sample's pose. */
  advance(now: number): SkeletonPose | null {
    if (this.playing && this.lastNow !== null) {
      this.positionUs += (now - this.lastNow) * 1000 * this.speed;
      if (this.positionUs > this.endUs) {
        if (this.loop) this.positionUs = this.startUs;
        else {
          this.positionUs = this.endUs;
          this.playing = false;
        }
      }
    }
    this.lastNow = now;
    return this.pose();
  }

  /** The last sample at or before the current time. */
  pose(): SkeletonPose | null {
    const s = this.clip.samples;
    if (s.length === 0) return null;
    let lo = 0;
    let hi = s.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (s[mid]!.t_us <= this.positionUs) lo = mid;
      else hi = mid - 1;
    }
    const sample = s[lo]!;
    return {
      skeleton_rev: this.clip.skeleton_rev,
      root_position_m: sample.root_m,
      root_position_provenance: "derived",
      segments: sample.rotations.map((rotation, i) => ({
        rotation,
        position_m: sample.positions_m[i] ?? sample.root_m,
        provenance: sample.inferred[i] ? "inferred" : "derived",
        confidence: null,
        sources: [],
      })),
    };
  }
}
