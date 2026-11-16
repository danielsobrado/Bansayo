import { useEffect, useRef } from "react";
import type { DeviceStatus } from "@contracts";
import { freshLandmarks } from "../../engine/stores";
import { tokenCss } from "../../viewport/palette";
import { OriginBadge } from "../../ui/OriginBadge";

/** Landmarks stay in the external store. This canvas reads them without React frame updates. */
export function CameraPreview({
  device,
  highlight = null,
}: {
  device: DeviceStatus;
  /** Landmark index to ring, e.g. the one selected in the reconstruction inspector. */
  highlight?: number | null;
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  const highlightRef = useRef(highlight);
  highlightRef.current = highlight;
  useEffect(() => {
    const canvas = ref.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;
    const resolution = device.descriptor.camera?.resolution_px ?? [1280, 720];
    canvas.width = resolution[0];
    canvas.height = resolution[1];
    const ink = tokenCss("--prov-derived");
    const faint = tokenCss("--line");
    const unavailable = tokenCss("--ink-3");
    const accent = tokenCss("--accent");
    const schema = device.descriptor.camera?.landmark_schema;
    let id = 0;
    function draw() {
      id = requestAnimationFrame(draw);
      if (document.hidden || !ctx || !canvas) return;
      const w = canvas.width;
      const h = canvas.height;
      ctx.clearRect(0, 0, w, h);
      ctx.strokeStyle = faint;
      ctx.lineWidth = 1;
      ctx.strokeRect(w * 0.05, h * 0.05, w * 0.9, h * 0.9);
      const landmarks = freshLandmarks(performance.now()).get(
        device.descriptor.id,
      );
      if (!landmarks) {
        ctx.fillStyle = unavailable;
        ctx.font = `${Math.max(16, w / 55)}px sans-serif`;
        ctx.textAlign = "center";
        ctx.fillText("Waiting for 2D landmarks", w / 2, h / 2);
        return;
      }
      ctx.strokeStyle = ink;
      ctx.fillStyle = ink;
      ctx.lineWidth = Math.max(2, w / 450);
      ctx.beginPath();
      for (const edge of schema?.edges ?? []) {
        const a = landmarks.points[edge[0]];
        const b = landmarks.points[edge[1]];
        if (!a || !b) continue;
        ctx.moveTo(a.x_px, a.y_px);
        ctx.lineTo(b.x_px, b.y_px);
      }
      ctx.stroke();
      for (const point of landmarks.points) {
        if (!point) continue;
        ctx.globalAlpha = Math.max(0.25, point.confidence);
        ctx.beginPath();
        ctx.arc(point.x_px, point.y_px, Math.max(3, w / 240), 0, Math.PI * 2);
        ctx.fill();
      }
      ctx.globalAlpha = 1;
      const selected = landmarks.points[highlightRef.current ?? -1];
      if (selected) {
        ctx.strokeStyle = accent;
        ctx.lineWidth = Math.max(2, w / 400);
        ctx.beginPath();
        ctx.arc(selected.x_px, selected.y_px, Math.max(8, w / 90), 0, Math.PI * 2);
        ctx.stroke();
      }
    }
    draw();
    return () => cancelAnimationFrame(id);
  }, [device.descriptor]);
  return (
    <div className="camera-preview">
      <canvas
        ref={ref}
        role="img"
        aria-label={`2D landmarks from ${device.descriptor.id}, in image pixels`}
      />
      <div className="camera-preview-label">
        <OriginBadge label={device.descriptor.origin} />
        <span>No image · 2D landmarks</span>
      </div>
    </div>
  );
}
