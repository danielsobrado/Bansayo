import goldenState from "../../../../contracts/examples/system_state.json";
import { describe, expect, it } from "vitest";
import { isCompleteHello, parseServerMessage } from "./messages";
import type { ServerMessage } from "@contracts";
const frame = {
  type: "frame",
  seq: 1,
  t_us: 1000,
  origins: ["simulated"],
  joints: [],
  quality: {},
  pose: { skeleton_rev: 1, segments: [{}] },
};
describe("stream boundaries", () => {
  it("drops a frame with incomplete segment geometry before it reaches three.js", () => {
    expect(parseServerMessage(JSON.stringify(frame)).kind).toBe("invalid");
  });
  it("drops skeletons without the fields used by the renderer", () => {
    expect(
      parseServerMessage(
        JSON.stringify({
          type: "skeleton",
          skeleton: { rev: 1, segments: [{}], joints: [] },
        }),
      ).kind,
    ).toBe("invalid");
  });
  it("requires renderable state in hello", () => {
    expect(
      isCompleteHello({
        type: "hello",
        protocol_version: 1,
        skeleton: { rev: 1, segments: [{}], joints: [] },
        state: { engine: {}, devices: [], features: [] },
      } as unknown as Extract<ServerMessage, { type: "hello" }>),
    ).toBe(false);
  });
  it("tolerates unknown messages and malformed JSON", () => {
    expect(parseServerMessage('{"type":"future_feature"}').kind).toBe(
      "unknown",
    );
    expect(parseServerMessage("{").kind).toBe("invalid");
  });
});

describe("camera and device boundaries", () => {
  it("drops device state with unusable statistics before a card renders", () => {
    const state = {
      ...goldenState,
      devices: [{ ...goldenState.devices[0], stream: null }],
    };
    expect(
      parseServerMessage(JSON.stringify({ type: "state", state })).kind,
    ).toBe("invalid");
  });
  it("drops incomplete replay metadata before the transport renders", () => {
    const state = { ...goldenState, playback: { session: null } };
    expect(
      parseServerMessage(JSON.stringify({ type: "state", state })).kind,
    ).toBe("invalid");
  });
  it("drops landmark frames without drawable finite points", () => {
    expect(
      parseServerMessage(
        JSON.stringify({
          type: "landmarks_2d",
          landmarks: { frame_meta: { device_id: "CAM1" }, points: [{}] },
        }),
      ).kind,
    ).toBe("invalid");
  });
});

describe("triangulated landmark boundaries", () => {
  const point = {
    m: [0, 0.93, 0],
    views: 2,
    reprojection_error_px: 1.4,
    confidence: 0.9,
    used_views: ["SIM-CAM1", "SIM-CAM2"],
    rejected_views: [],
  };
  const message = {
    type: "landmarks_3d",
    landmarks: {
      kind: "triangulated",
      t_us: 303800000,
      frame: { type: "world" },
      origin: "simulated",
      schema_id: "bansayo.body.v1.joints",
      calibration_refs: ["camera-extrinsics-SIM-CAM1"],
      points: [point, null],
    },
  };
  const parse = (landmarks: object) =>
    parseServerMessage(
      JSON.stringify({ ...message, landmarks: { ...message.landmarks, ...landmarks } }),
    ).kind;
  it("accepts a well-formed world-frame set with unreconstructed points", () => {
    expect(parseServerMessage(JSON.stringify(message)).kind).toBe("message");
  });
  it("drops non-finite coordinates", () => {
    const text = JSON.stringify(message).replace("0.93", "1e999");
    expect(parseServerMessage(text).kind).toBe("invalid");
  });
  it("drops wrong shapes and missing arrays", () => {
    expect(parse({ points: [{ ...point, m: [0, 1] }] })).toBe("invalid");
    expect(parse({ points: [{ ...point, used_views: undefined }] })).toBe(
      "invalid",
    );
    expect(parse({ points: [{ ...point, rejected_views: "SIM-CAM2" }] })).toBe(
      "invalid",
    );
    expect(parse({ points: undefined })).toBe("invalid");
    expect(parse({ calibration_refs: undefined })).toBe("invalid");
    expect(parse({ points: [{ ...point, reprojection_error_px: "1.4" }] })).toBe(
      "invalid",
    );
  });
  it("drops sets that are not triangulated or not in the world frame", () => {
    expect(parse({ kind: "model_relative" })).toBe("invalid");
    expect(parse({ frame: { type: "camera", id: "SIM-CAM1" } })).toBe(
      "invalid",
    );
  });
});
