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
