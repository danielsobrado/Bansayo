/** Wire frustums of calibrated cameras, labelled with the device id. Rebuilt only when they change. */
import {
  BufferGeometry,
  CanvasTexture,
  Group,
  LineBasicMaterial,
  LineSegments,
  Sprite,
  SpriteMaterial,
  Vector3,
} from "three";
import { frustumCorners, type CameraPose } from "../engine/cameraCalibration";
import { tokenCss, type Palette } from "./palette";

const DEPTH_M = 0.35;
/** Apex (0) to each image corner (1–4), then the image rectangle. */
const SEGMENTS = [0, 1, 0, 2, 0, 3, 0, 4, 1, 2, 2, 3, 3, 4, 4, 1];
const LABEL_W_PX = 320;
const LABEL_H_PX = 48;
/** Label height as a fraction of the view height at unit distance: constant on screen. */
const LABEL_H = 0.045;

export class CameraFrustums {
  readonly root = new Group();
  private key = "";
  private disposables: { dispose(): void }[] = [];

  constructor(private readonly palette: Palette) {}

  set(cameras: readonly CameraPose[], selected: string | null): void {
    const key = JSON.stringify([cameras, selected]);
    if (key === this.key) return;
    this.key = key;
    this.clear();
    for (const camera of cameras) {
      const isSelected = camera.id === selected;
      const corners = [camera.t, ...frustumCorners(camera, DEPTH_M)].map(
        (p) => new Vector3(...p),
      );
      const geometry = new BufferGeometry().setFromPoints(
        SEGMENTS.map((i) => corners[i]!),
      );
      const material = new LineBasicMaterial({
        color: isSelected ? this.palette.accent : this.palette.ink2,
      });
      this.disposables.push(geometry, material);
      this.root.add(new LineSegments(geometry, material));
      // The selected camera also carries a glyph, so the highlight is not colour alone.
      const label = this.label(
        isSelected ? `● ${camera.id}` : camera.id,
        isSelected ? "--accent" : "--ink-2",
      );
      label.position.set(camera.t[0], camera.t[1] + 0.12, camera.t[2]);
      this.root.add(label);
    }
  }

  dispose(): void {
    this.clear();
    this.root.removeFromParent();
  }

  private clear(): void {
    this.root.clear();
    for (const d of this.disposables) d.dispose();
    this.disposables = [];
  }

  private label(text: string, token: string): Sprite {
    const canvas = document.createElement("canvas");
    canvas.width = LABEL_W_PX;
    canvas.height = LABEL_H_PX;
    const ctx = canvas.getContext("2d");
    if (ctx) {
      ctx.font = `28px ${tokenCss("--font-mono")}`;
      ctx.fillStyle = tokenCss(token);
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(text, LABEL_W_PX / 2, LABEL_H_PX / 2);
    }
    const texture = new CanvasTexture(canvas);
    const material = new SpriteMaterial({
      map: texture,
      transparent: true,
      depthWrite: false,
      sizeAttenuation: false,
    });
    this.disposables.push(texture, material);
    const sprite = new Sprite(material);
    sprite.scale.set((LABEL_H * LABEL_W_PX) / LABEL_H_PX, LABEL_H, 1);
    return sprite;
  }
}
