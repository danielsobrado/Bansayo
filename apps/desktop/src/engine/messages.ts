/**
 * Boundary check for stream messages. Unknown message types and unknown fields pass through
 * untouched (docs/04 §2); a known type is accepted only if the fields the client relies on are
 * present, so a malformed message is dropped instead of breaking a view.
 */
import type { ServerMessage } from "@contracts";

type Obj = Record<string, unknown>;

const isObj = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v);
const isNum = (v: unknown): v is number => typeof v === "number" && Number.isFinite(v);

function isSkeleton(v: unknown): boolean {
  return isObj(v) && isNum(v.rev) && Array.isArray(v.segments) && Array.isArray(v.joints);
}

function isState(v: unknown): boolean {
  return isObj(v) && isObj(v.engine) && Array.isArray(v.devices) && Array.isArray(v.features);
}

function isFrame(m: Obj): boolean {
  const pose = m.pose;
  const poseOk = pose === null || (isObj(pose) && isNum(pose.skeleton_rev) && Array.isArray(pose.segments));
  return isNum(m.seq) && isNum(m.t_us) && Array.isArray(m.origins) && Array.isArray(m.joints) && isObj(m.quality) && poseOk;
}

export type ParsedMessage =
  | { kind: "message"; message: ServerMessage }
  | { kind: "unknown"; type: string }
  | { kind: "invalid" };

/** Parses one text frame. */
export function parseServerMessage(text: string): ParsedMessage {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    return { kind: "invalid" };
  }
  if (!isObj(raw) || typeof raw.type !== "string") return { kind: "invalid" };

  let valid: boolean;
  switch (raw.type) {
    case "hello":
      // Only the version is required here; the rest is checked after the version is accepted.
      valid = isNum(raw.protocol_version);
      break;
    case "skeleton":
      valid = isSkeleton(raw.skeleton);
      break;
    case "state":
      valid = isState(raw.state);
      break;
    case "frame":
      valid = isFrame(raw);
      break;
    case "landmarks_2d":
      valid = isObj(raw.landmarks) && isObj(raw.landmarks.frame_meta) && typeof raw.landmarks.frame_meta.device_id === "string";
      break;
    case "notice":
      valid = typeof raw.message === "string";
      break;
    default:
      return { kind: "unknown", type: raw.type };
  }
  // The checks above establish the fields the client reads; the cast records that boundary.
  return valid ? { kind: "message", message: raw as unknown as ServerMessage } : { kind: "invalid" };
}

/** Whether a `hello` that passed the version check carries a usable skeleton and state. */
export function isCompleteHello(m: Extract<ServerMessage, { type: "hello" }>): boolean {
  return isSkeleton(m.skeleton) && isState(m.state);
}
