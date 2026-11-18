import type { ReactNode } from "react";

export type IconName =
  | "play" | "target" | "chart" | "monitor" | "crosshair" | "usb"
  | "camera" | "folder" | "panel-left" | "panel-right" | "grid"
  | "layers" | "settings" | "ruler" | "rotate" | "record"
  | "chevron-down" | "focus" | "video" | "sliders" | "close" | "check" | "warning" | "expand"
  | "pause" | "stop" | "back";

const glyphs: Record<IconName, ReactNode> = {
  play: <path d="m8 5 11 7-11 7z" />,
  target: <><circle cx="12" cy="12" r="9" /><circle cx="12" cy="12" r="4" /><circle cx="12" cy="12" r="1" /></>,
  chart: <><path d="M4 19V5M4 19h16M8 15l4-5 3 2 5-7" /></>,
  monitor: <><rect x="3" y="4" width="18" height="14" rx="1.5" /><path d="M8 21h8M12 18v3" /></>,
  crosshair: <><circle cx="12" cy="12" r="8" /><path d="M12 2v4m0 12v4M2 12h4m12 0h4" /></>,
  usb: <><path d="M12 21V4m0 0-3 3m3-3 3 3M12 12h6v-2M12 17H6v-3" /><circle cx="18" cy="9" r="1" /><rect x="4" y="12" width="4" height="3" rx=".5" /></>,
  camera: <><rect x="3" y="6" width="18" height="14" rx="2" /><path d="m8 6 1.5-2h5L16 6" /><circle cx="12" cy="13" r="4" /></>,
  folder: <path d="M3 6h7l2 2h9v11H3z" />,
  "panel-left": <><rect x="3" y="3" width="18" height="18" rx="2" /><path d="M9 3v18" /></>,
  "panel-right": <><rect x="3" y="3" width="18" height="18" rx="2" /><path d="M15 3v18" /></>,
  grid: <><path d="M3 3h18v18H3zM3 9h18M9 3v18" /></>,
  layers: <><path d="m12 3 9 5-9 5-9-5zM3 12l9 5 9-5M3 16l9 5 9-5" /></>,
  settings: <><circle cx="12" cy="12" r="3" /><path d="M10 2h4l1 3 3 1 3-1 2 4-2 2v2l2 2-2 4-3-1-3 1-1 3h-4l-1-3-3-1-3 1-2-4 2-2v-2L1 9l2-4 3 1 3-1z" /></>,
  ruler: <><path d="m5 21-3-3L18 2l4 4zM12 8l3 3M9 11l3 3M6 14l3 3" /></>,
  rotate: <><path d="M20 11a8 8 0 1 0-2 6M20 4v7h-7" /></>,
  record: <circle cx="12" cy="12" r="8" />,
  "chevron-down": <path d="m5 9 7 7 7-7" />,
  focus: <><path d="M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5" /><circle cx="12" cy="12" r="3" /></>,
  video: <><rect x="3" y="5" width="13" height="14" rx="2" /><path d="m16 10 5-3v10l-5-3" /></>,
  sliders: <><path d="M4 6h16M4 12h16M4 18h16" /><circle cx="8" cy="6" r="2" /><circle cx="16" cy="12" r="2" /><circle cx="10" cy="18" r="2" /></>,
  close: <path d="m6 6 12 12M18 6 6 18" />,
  check: <path d="m5 12 4 4L19 6" />,
  warning: <><path d="M12 3 2 20h20zM12 9v4" /><circle cx="12" cy="16.5" r=".5" /></>,
  expand: <path d="M7 17 17 7M10 7h7v7" />,
  pause: <path d="M9 5v14m6-14v14" />,
  stop: <rect x="6" y="6" width="12" height="12" rx="1" />,
  back: <path d="M19 12H5m7-7-7 7 7 7" />,
};

export function Icon({ name, size = 16 }: { name: IconName; size?: number }) {
  return (
    <svg aria-hidden="true" width={size} height={size} viewBox="0 0 24 24"
      fill="none" stroke="currentColor" strokeWidth="1.7"
      strokeLinecap="round" strokeLinejoin="round">
      {glyphs[name]}
    </svg>
  );
}
