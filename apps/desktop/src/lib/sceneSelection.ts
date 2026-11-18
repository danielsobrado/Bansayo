export type SceneSelection =
  | { kind: "segment" | "joint" | "landmark"; index: number }
  | { kind: "camera" | "sensor"; id: string };
