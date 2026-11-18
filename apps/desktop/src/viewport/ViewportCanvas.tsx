import { useEffect, useRef, useState } from "react";
import type { LandmarkSchema, SkeletonDefinition } from "@contracts";
import type { CameraPose } from "../engine/cameraCalibration";
import type { SceneSelection } from "../lib/sceneSelection";
import {
  MotionViewport,
  type PoseSource,
  type ViewPreset,
} from "./MotionViewport";
const NO_CAMERAS: readonly CameraPose[] = [];

interface Props {
  skeleton: SkeletonDefinition | null;
  landmarkSchema?: LandmarkSchema | null;
  selection?: SceneSelection | null;
  onSelect?: (selection: SceneSelection) => void;
  view: ViewPreset;
  viewRevision?: number;
  skeletonVisible: boolean;
  floorVisible: boolean;
  cameras?: readonly CameraPose[];
  active?: boolean;
  /** Draws this pose instead of the live frame (stored clips). */
  poseSource?: PoseSource | null;
}
export function ViewportCanvas({
  skeleton,
  landmarkSchema = null,
  selection = null,
  onSelect,
  view,
  viewRevision = 0,
  skeletonVisible,
  floorVisible,
  cameras = NO_CAMERAS,
  active = true,
  poseSource = null,
}: Props) {
  const host = useRef<HTMLDivElement>(null);
  const viewport = useRef<MotionViewport | null>(null);
  const selectRef = useRef(onSelect);
  const selectedCamera = selection?.kind === "camera" ? selection.id : null;
  selectRef.current = onSelect;
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!host.current || !(skeleton || landmarkSchema)) return;
    try {
      viewport.current = new MotionViewport(
        host.current,
        skeleton,
        (target) => selectRef.current?.(target),
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
    () => viewport.current?.setSelection(selection),
    [selection, skeleton, landmarkSchema, poseSource],
  );
  useEffect(
    () => viewport.current?.setView(view),
    [view, viewRevision, skeleton, landmarkSchema, poseSource],
  );
  useEffect(
    () => viewport.current?.setLayers(skeletonVisible, floorVisible),
    [skeletonVisible, floorVisible, skeleton, landmarkSchema, poseSource],
  );
  useEffect(
    () => viewport.current?.setCameras(cameras, selectedCamera),
    [cameras, selectedCamera, skeleton, landmarkSchema, poseSource],
  );
  useEffect(
    () => viewport.current?.setActive(active),
    [active, skeleton, landmarkSchema, poseSource],
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
