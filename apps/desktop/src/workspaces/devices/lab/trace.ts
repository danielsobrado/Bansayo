import { isNum, isObj, tuple } from "../../../engine/messages";

export const CHANNELS = {
  orientation: { title: "Sensor orientation", unit: "quaternion", axes: ["X", "Y", "Z", "W"] },
  gyro: { title: "Gyroscope", unit: "rad/s", axes: ["X", "Y", "Z"] },
  accel: { title: "Accelerometer", unit: "m/s²", axes: ["X", "Y", "Z"] },
  position: { title: "Position", unit: "m", axes: ["X", "Y", "Z"] },
  pose_rotation: { title: "Tracked rotation", unit: "quaternion", axes: ["X", "Y", "Z", "W"] },
  pose_position: { title: "Tracked position", unit: "m", axes: ["X", "Y", "Z"] },
  distance: { title: "Distance", unit: "m", axes: ["Range"] },
  contact: { title: "Contact", unit: "0/1", axes: ["Contact"] },
  mag0: { title: "BMM150 #0", unit: "BSP compensated units", axes: ["X", "Y", "Z"] },
  mag1: { title: "BMM150 #1", unit: "BSP compensated units", axes: ["X", "Y", "Z"] },
} as const;
export type ChannelId = keyof typeof CHANNELS;
export const CHANNEL_IDS = Object.keys(CHANNELS) as ChannelId[];
export const STALE_MS = 500;
export const POLL_MS = 250;
export const AXIS_COLORS = ["#ee7878", "#58bd91", "#68a6f1", "#d4a8f1"];

export interface Point {
  t_us: number;
  seq: number;
  device_time_us: number | null;
  values: number[];
}
export interface Channel {
  age_ms: number;
  points: Point[];
}
export interface Timing {
  intervals_ms: [number,number][];
  sequence_gaps: number;
}
export interface TransportSnapshot {
  generation:number;received:number;missing:number;rejected_order:number;last_receive_us:number;intervals_ms:[number,number][];
}
export interface Snapshot {
  transport?:TransportSnapshot;
  reference?: number;
  channels: Partial<Record<ChannelId, Channel>>;
  timing?: Timing;
}

export function parseSnapshot(value: unknown): Snapshot | null {
  if (!isObj(value) || !isObj(value.channels)) return null;
  const reference=value.reference;
  if(reference!==undefined&&(!isNum(reference)||!Number.isSafeInteger(reference)||reference<0))return null;
  const channels: Snapshot["channels"] = {};
  for (const id of CHANNEL_IDS) {
    const c = value.channels[id];
    if (c === undefined) continue;
    if (!isObj(c) || !isNum(c.age_ms) || c.age_ms < 0 || !Array.isArray(c.points) || c.points.length > 256) return null;
    const points: Point[] = [];
    for (const p of c.points) {
      if (!isObj(p) || !isNum(p.t_us) || !isNum(p.seq) ||
          !(p.device_time_us === null || isNum(p.device_time_us)) ||
          !tuple(p.values, CHANNELS[id].axes.length)) return null;
      if (points.length && p.t_us < points[points.length - 1]!.t_us) return null;
      points.push({ t_us: p.t_us, seq: p.seq, device_time_us: p.device_time_us, values: p.values });
    }
    channels[id] = { age_ms: c.age_ms, points };
  }
  const timing = value.timing;
  const transport=value.transport;
  const identity:Pick<Snapshot,"reference"|"transport">=reference===undefined?{}:{reference};
  if(transport!=null){
    if(!isObj(transport)||!["generation","received","missing","rejected_order","last_receive_us"].every(k=>typeof transport[k]==="number"&&Number.isSafeInteger(transport[k])&&(transport[k] as number)>=0)||
      !Array.isArray(transport.intervals_ms)||transport.intervals_ms.length>256||!transport.intervals_ms.every(v=>tuple(v,2)&&v[0]!>0&&v[1]!>=0))return null;
    identity.transport=transport as unknown as TransportSnapshot;
  }
  if (timing == null) return { ...identity, channels };
  if (!isObj(timing) || !isNum(timing.sequence_gaps) || timing.sequence_gaps < 0 ||
      !Array.isArray(timing.intervals_ms) || timing.intervals_ms.length > 256 ||
      !timing.intervals_ms.every(v => tuple(v,2) && v.every(n => n>=0))) return null;
  return { ...identity, channels, timing: { intervals_ms: timing.intervals_ms as [number,number][], sequence_gaps: timing.sequence_gaps } };
}

/** Never show a historical observation as live after the HTTP stream stalls. */
export function freshChannels(channels: Snapshot["channels"], elapsedMs: number, available: boolean): Snapshot["channels"] {
  const result: Snapshot["channels"] = {};
  if (!available || !Number.isFinite(elapsedMs)) return result;
  for (const id of CHANNEL_IDS) {
    const channel = channels[id];
    if (!channel) continue;
    const age_ms = channel.age_ms + Math.max(0, elapsedMs);
    if (age_ms <= STALE_MS) result[id] = { ...channel, age_ms };
  }
  return result;
}

export function rotate(v: readonly number[], q: readonly number[]): number[] {
  const n = Math.hypot(...q);
  if (n < 1e-6 || !Number.isFinite(n)) return [...v];
  const [x, y, z, w] = q.map((p) => p / n) as [number, number, number, number];
  const [a, b, c] = v as readonly [number, number, number];
  const u = 2 * (y*c-z*b), t = 2 * (z*a-x*c), s = 2 * (x*b-y*a);
  return [a+w*u+y*s-z*t, b+w*t+z*u-x*s, c+w*s+x*t-y*u];
}

export function motionSummary(gyro?: Channel, accel?: Channel) {
  if (!gyro?.points.length || !accel?.points.length ||
      gyro.age_ms > STALE_MS || accel.age_ms > STALE_MS) return null;
  const end = Math.min(gyro.points.at(-1)!.t_us, accel.points.at(-1)!.t_us);
  const recent = (c: Channel) => c.points.filter(p => p.t_us >= end - 1_000_000 && p.t_us <= end);
  const g = recent(gyro), a = recent(accel);
  const continuous=(p:Point[])=>p.every((v,i)=>i===0||v.t_us-p[i-1]!.t_us<=150_000);
  if (!continuous(g)||!continuous(a)||g.length < 2 || a.length < 2 ||
      g.at(-1)!.t_us-g[0]!.t_us < 750_000 ||
      a.at(-1)!.t_us-a[0]!.t_us < 750_000) return null;
  const peak = Math.max(...g.map(p => Math.hypot(...p.values)));
  const gravityError = Math.max(...a.map(p => Math.abs(Math.hypot(...p.values)-9.80665)));
  return { peak, gravityError, still: peak < 0.12 && gravityError < 0.08*9.80665 };
}
