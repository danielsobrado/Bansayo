import { describe, expect, it } from "vitest";
import state from "../../../../contracts/examples/system_state.json";
import skeleton from "../../../../contracts/examples/skeleton.json";
import type { SkeletonDefinition, SystemState } from "@contracts";
import { assignmentOptions, attentionReasons } from "./presentation";

describe("workspace presentation", () => {
  it("lists unassigned segments before used ones without dropping the current assignment", () => {
    const options = assignmentOptions(
      skeleton.skeleton as SkeletonDefinition,
      state as SystemState,
      "pelvis",
    );
    expect(options).toHaveLength(19);
    const lastFree = options.findLastIndex((o) => !o.assigned);
    const firstUsed = options.findIndex((o) => o.assigned);
    expect(lastFree).toBeLessThan(firstUsed);
    expect(options.find((o) => o.id === "pelvis")?.current).toBe(true);
  });
  it("keeps the engine’s reason wording and removes duplicated attention messages", () => {
    const s = structuredClone(state) as SystemState;
    const reason = {
      code: "missing",
      message: "Capture mounting first.",
      subject: null,
    };
    s.features[0]!.reasons = [reason];
    s.features[1]!.reasons = [reason];
    expect(
      attentionReasons(s).filter((r) => r.message === reason.message),
    ).toHaveLength(1);
  });
});
