import { describe, expect, it } from "vitest";
import golden from "../../../../contracts/examples/frame.json";
import { frameStore, type FrameMessage } from "./stores";
import { jointHistory } from "./jointHistory";

describe("trace timeline", () => {
  it("starts a fresh trace on backwards time without accepting old sequence numbers", () => {
    frameStore.clear();
    const frame = golden as FrameMessage;
    frameStore.apply({ ...frame, seq: 10, t_us: 100_000 }, 1);
    frameStore.apply({ ...frame, seq: 11, t_us: 200_000 }, 2);
    frameStore.apply({ ...frame, seq: 12, t_us: 50_000 }, 3);
    expect(jointHistory.count).toBe(1);
    expect(frameStore.lastSeq).toBe(12);
    expect(frameStore.apply({ ...frame, seq: 11, t_us: 10_000 }, 4)).toBe(
      false,
    );
    expect(jointHistory.count).toBe(1);
    frameStore.clear();
  });
});
