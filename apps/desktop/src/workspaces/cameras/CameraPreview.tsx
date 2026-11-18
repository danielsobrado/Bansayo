import { useEffect, useRef } from "react";
import type { DeviceStatus } from "@contracts";
import { freshLandmarks } from "../../engine/stores";
import { tokenCss } from "../../viewport/palette";
import { OriginBadge } from "../../ui/OriginBadge";
import { API_BASE } from "../../engine/config";

type AprilTagSnapshot = {
  tags: Array<{
    id: number;
    corners_px: [number, number][];
  }>;
};

/** The engine refreshes previews at most every 100 ms. */
const PREVIEW_POLL_MS = 150;

/** Landmarks stay in the external store. This canvas reads them without React frame updates. */
export function CameraPreview({
  device,
  highlight = null,
  showAprilTags = false,
}: {
  device: DeviceStatus;
  /** Landmark index to ring, e.g. the one selected in the reconstruction inspector. */
  highlight?: number | null;
  showAprilTags?: boolean;
}) {
  const ref = useRef<HTMLCanvasElement>(null);
  const highlightRef = useRef(highlight);
  highlightRef.current = highlight;
  const hasImage = device.descriptor.origin !== "simulated";
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
    // Physical cameras: newest JPEG preview, polled; drawn stretched to image pixels.
    let image: ImageBitmap | null = null;
    let tags: AprilTagSnapshot["tags"] = [];
    let stopped = false;
    let poll = 0;
    async function fetchPreview() {
      if (stopped) return;
      try {
        if (!document.hidden) {
          const response = await fetch(
            `${API_BASE}/cameras/${encodeURIComponent(device.descriptor.id)}/preview.jpg`,
            { cache: "no-store" },
          );
          if (response.ok) {
            const next = await createImageBitmap(await response.blob());
            image?.close();
            image = next;
          }
        }
        if (showAprilTags) {
          const markers = await fetch(
            `${API_BASE}/cameras/${encodeURIComponent(device.descriptor.id)}/fiducials`,
            { cache: "no-store" },
          );
          if (markers.ok) {
            const response: { ok: boolean; data?: AprilTagSnapshot } = await markers.json();
            tags = response.ok && Array.isArray(response.data?.tags)
              ? response.data.tags
              : [];
          } else {
            tags = [];
          }
        }
      } catch {
        // Engine restarting or camera gone: keep the last image and retry.
        tags = [];
      }
      if (!stopped) poll = window.setTimeout(fetchPreview, PREVIEW_POLL_MS);
    }
    if (device.descriptor.origin !== "simulated") void fetchPreview();
    function draw() {
      id = requestAnimationFrame(draw);
      if (document.hidden || !ctx || !canvas) return;
      const w = canvas.width;
      const h = canvas.height;
      ctx.clearRect(0, 0, w, h);
      if (image) {
        ctx.drawImage(image, 0, 0, w, h);
      } else {
        ctx.strokeStyle = faint;
        ctx.lineWidth = 1;
        ctx.strokeRect(w * 0.05, h * 0.05, w * 0.9, h * 0.9);
      }
      ctx.strokeStyle = accent;
      ctx.fillStyle = accent;
      ctx.lineWidth = Math.max(2, w / 450);
      ctx.textAlign = "left";
      ctx.font = `${Math.max(14, w / 65)}px sans-serif`;
      for (const tag of tags) {
        if (tag.corners_px.length !== 4) continue;
        ctx.beginPath();
        tag.corners_px.forEach(([x, y], index) => {
          if (index === 0) ctx.moveTo(x, y);
          else ctx.lineTo(x, y);
        });
        ctx.closePath();
        ctx.stroke();
        ctx.fillText(`Tag ${tag.id}`, tag.corners_px[0][0], tag.corners_px[0][1] - 8);
      }
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
      const detected = landmarks.points.filter(Boolean).length;
      ctx.font = `${Math.max(16, w / 55)}px sans-serif`;
      ctx.textAlign = "left";
      ctx.fillStyle = detected ? ink : unavailable;
      ctx.fillText(
        detected
          ? `${detected}/${landmarks.points.length} landmarks`
          : "No person detected",
        w * 0.02,
        h * 0.06,
      );
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
    return () => {
      stopped = true;
      window.clearTimeout(poll);
      image?.close();
      cancelAnimationFrame(id);
    };
  }, [device.descriptor, showAprilTags]);
  return (
    <div className="camera-preview">
      <canvas
        ref={ref}
        role="img"
        aria-label={`${hasImage ? "Camera image and " : ""}2D landmarks from ${device.descriptor.id}, in image pixels`}
      />
      <div className="camera-preview-label">
        <OriginBadge label={device.descriptor.origin} />
        <span>{hasImage ? "Camera image" : "No image"} · 2D landmarks</span>
      </div>
    </div>
  );
}
