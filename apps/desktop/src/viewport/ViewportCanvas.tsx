import { useEffect, useRef, useState } from "react";
import type { SkeletonDefinition } from "@contracts";
import { MotionViewport, type ViewPreset } from "./MotionViewport";
interface Props {
  skeleton: SkeletonDefinition | null;
  selected: number;
  onSelect: (index: number) => void;
  view: ViewPreset;
  skeletonVisible: boolean;
  floorVisible: boolean;
}
export function ViewportCanvas({
  skeleton,
  selected,
  onSelect,
  view,
  skeletonVisible,
  floorVisible,
}: Props) {
  const host = useRef<HTMLDivElement>(null);
  const viewport = useRef<MotionViewport | null>(null);
  const selectRef = useRef(onSelect);
  selectRef.current = onSelect;
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!host.current || !skeleton) return;
    try {
      viewport.current = new MotionViewport(host.current, skeleton, (index) =>
        selectRef.current(index),
      );
      setError(null);
    } catch {
      setError(
        "The 3D viewport could not start. Check that WebGL is available. Segment and joint readouts remain accessible.",
      );
    }
    return () => {
      viewport.current?.dispose();
      viewport.current = null;
    };
  }, [skeleton]);
  useEffect(
    () => viewport.current?.setSelected(selected),
    [selected, skeleton],
  );
  useEffect(() => viewport.current?.setView(view), [view, skeleton]);
  useEffect(
    () => viewport.current?.setLayers(skeletonVisible, floorVisible),
    [skeletonVisible, floorVisible, skeleton],
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
