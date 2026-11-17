import { useEffect, useRef, useState } from "react";
import type { LandmarkSchema, SkeletonDefinition } from "@contracts";
import type { CameraPose } from "../engine/cameraCalibration";
import {
  MotionViewport,
  type PoseSource,
  type ViewPreset,
} from "./MotionViewport";
interface Props {
  skeleton: SkeletonDefinition | null;
  /** Shows triangulated landmarks; `selected`/`onSelect` then index landmarks. */
  landmarkSchema?: LandmarkSchema | null;
  selected: number;
  onSelect: (index: number) => void;
  view: ViewPreset;
  skeletonVisible: boolean;
  floorVisible: boolean;
  cameras?: readonly CameraPose[];
  selectedCamera?: string | null;
  /** Draws this pose instead of the live frame (stored clips). */
  poseSource?: PoseSource | null;
}
export function ViewportCanvas({
  skeleton,
  landmarkSchema = null,
  selected,
  onSelect,
  view,
  skeletonVisible,
  floorVisible,
  cameras = [],
  selectedCamera = null,
  poseSource = null,
}: Props) {
  const host = useRef<HTMLDivElement>(null);
  const viewport = useRef<MotionViewport | null>(null);
  const selectRef = useRef(onSelect);
  selectRef.current = onSelect;
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!host.current || !(skeleton || landmarkSchema)) return;
    try {
      viewport.current = new MotionViewport(
        host.current,
        skeleton,
        (index) => selectRef.current(index),
        landmarkSchema,
        poseSource,
      );
      setError(null);
    } catch {
      setError(
        landmarkSchema
          ? "The 3D view could not start. Check that WebGL is available. The landmark list and inspector remain accessible."
          : "The 3D viewport could not start. Check that WebGL is available. Segment and joint readouts remain accessible.",
      );
    }
    return () => {
      viewport.current?.dispose();
      viewport.current = null;
    };
  }, [skeleton, landmarkSchema, poseSource]);
  useEffect(
    () => viewport.current?.setSelected(selected),
    [selected, skeleton, landmarkSchema],
  );
  useEffect(
    () => viewport.current?.setView(view),
    [view, skeleton, landmarkSchema],
  );
  useEffect(
    () => viewport.current?.setLayers(skeletonVisible, floorVisible),
    [skeletonVisible, floorVisible, skeleton, landmarkSchema],
  );
  useEffect(
    () => viewport.current?.setCameras(cameras, selectedCamera),
    [cameras, selectedCamera, skeleton, landmarkSchema],
  );
  return (
    <div className="viewport-canvas" ref={host}>
      {error && (
        <p className="viewport-error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
