import {
  AmbientLight,
  DirectionalLight,
  GridHelper,
  PerspectiveCamera,
  Raycaster,
  Scene,
  Vector2,
  WebGLRenderer,
} from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type {
  LandmarkSchema,
  SkeletonDefinition,
  SkeletonPose,
} from "@contracts";
import type { CameraPose } from "../engine/cameraCalibration";
import { frameStore, STALE_FRAME_MS } from "../engine/stores";
import { CameraFrustums } from "./CameraFrustums";
import { LandmarkRig } from "./LandmarkRig";
import { readPalette } from "./palette";
import { SkeletonRig } from "./SkeletonRig";

export type ViewPreset = "Front" | "Side" | "Top" | "3D";
export type PoseSource = (now: number) => SkeletonPose | null;
/** Height the views centre on when there is no skeleton, metres. */
const DEFAULT_HEIGHT_M = 1.8;

/**
 * Shows the skeleton from `frameStore`, or (with a `landmarkSchema`) the triangulated landmarks
 * from `landmarks3dStore`. `select` receives the picked segment or landmark index.
 */
export class MotionViewport {
  private readonly renderer = new WebGLRenderer({ antialias: true });
  private readonly scene = new Scene();
  private readonly camera = new PerspectiveCamera(38, 1, 0.01, 100);
  private readonly controls: OrbitControls;
  private readonly rig: SkeletonRig | null;
  private readonly landmarks: LandmarkRig | null;
  private readonly frustums: CameraFrustums;
  private readonly grid: GridHelper;
  private readonly resize: ResizeObserver;
  private readonly raycaster = new Raycaster();
  private readonly pointer = new Vector2();
  private animation = 0;
  private skeletonVisible = true;
  private floorVisible = true;
  private pointerX = 0;
  private pointerY = 0;

  constructor(
    private readonly host: HTMLElement,
    private readonly definition: SkeletonDefinition | null,
    private readonly select: (index: number) => void,
    landmarkSchema: LandmarkSchema | null = null,
    /** Replaces the live `frameStore` pose, e.g. a stored clip; called once per animation frame. */
    private readonly poseSource: PoseSource | null = null,
  ) {
    const palette = readPalette();
    this.scene.background = palette.surface0;
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.renderer.domElement.setAttribute(
      "aria-label",
      landmarkSchema
        ? "Triangulated 3D landmarks and calibrated camera frustums. Use the landmark list to select landmarks with a keyboard."
        : "Live motion skeleton. Use the segment list to select bones with a keyboard.",
    );
    this.renderer.domElement.setAttribute("role", "img");
    host.append(this.renderer.domElement);
    this.scene.add(new AmbientLight(palette.measured, 2));
    const key = new DirectionalLight(palette.measured, 2.5);
    key.position.set(3, 4, 5);
    this.scene.add(key);
    this.grid = new GridHelper(10, 20, palette.ink3, palette.line);
    const mats = Array.isArray(this.grid.material)
      ? this.grid.material
      : [this.grid.material];
    for (const m of mats) {
      m.transparent = true;
      m.opacity = 0.36;
    }
    this.scene.add(this.grid);
    this.rig = definition && new SkeletonRig(definition, palette);
    if (this.rig) {
      this.rig.root.visible = false;
      this.scene.add(this.rig.root);
    }
    this.landmarks = landmarkSchema && new LandmarkRig(landmarkSchema, palette);
    if (this.landmarks) this.scene.add(this.landmarks.root);
    this.frustums = new CameraFrustums(palette);
    this.scene.add(this.frustums.root);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.controls.minDistance = 0.5;
    this.controls.maxDistance = 12;
    this.setView("3D");
    this.resize = new ResizeObserver(() => this.measure());
    this.resize.observe(host);
    this.measure();
    this.renderer.domElement.addEventListener("pointerdown", this.down);
    this.renderer.domElement.addEventListener("pointerup", this.pick);
    this.renderer.domElement.addEventListener("dblclick", this.centre);
    this.draw();
  }

  setView(view: ViewPreset): void {
    const centre = (this.definition?.height_m ?? DEFAULT_HEIGHT_M) / 2;
    // Without a skeleton the subject is the capture volume, cameras included: stand further back.
    const r = this.definition ? 1 : 1.9;
    this.controls.target.set(0, centre, 0);
    if (view === "Front") this.camera.position.set(0, centre, 4.2 * r);
    if (view === "Side") this.camera.position.set(4.2 * r, centre, 0);
    if (view === "Top") this.camera.position.set(0, 5.5 * r, 0.001);
    if (view === "3D") this.camera.position.set(2.5 * r, 1.8 * r, 4.1 * r);
    this.controls.update();
  }
  setLayers(skeleton: boolean, floor: boolean): void {
    this.skeletonVisible = skeleton;
    this.floorVisible = floor;
  }
  setSelected(index: number): void {
    this.rig?.setSelected(index);
    this.landmarks?.setSelected(index);
  }
  /** Frustums for these cameras; `selected` is highlighted. */
  setCameras(cameras: readonly CameraPose[], selected: string | null): void {
    this.frustums.set(cameras, selected);
  }
  dispose(): void {
    cancelAnimationFrame(this.animation);
    this.resize.disconnect();
    this.controls.dispose();
    this.renderer.domElement.removeEventListener("pointerdown", this.down);
    this.renderer.domElement.removeEventListener("pointerup", this.pick);
    this.renderer.domElement.removeEventListener("dblclick", this.centre);
    this.rig?.dispose();
    this.landmarks?.dispose();
    this.frustums.dispose();
    this.grid.geometry.dispose();
    const materials = Array.isArray(this.grid.material)
      ? this.grid.material
      : [this.grid.material];
    for (const material of materials) material.dispose();
    this.renderer.dispose();
    this.renderer.domElement.remove();
  }
  private measure(): void {
    const { clientWidth: w, clientHeight: h } = this.host;
    this.renderer.setSize(w, h);
    this.camera.aspect = w / Math.max(h, 1);
    this.camera.updateProjectionMatrix();
  }
  private draw = (): void => {
    this.animation = requestAnimationFrame(this.draw);
    if (document.hidden) return;
    const now = performance.now();
    const frame = frameStore.frame;
    const pose = this.poseSource ? this.poseSource(now) : frame?.pose;
    const fresh =
      this.poseSource !== null ||
      (frame !== null && now - frameStore.receivedAt <= STALE_FRAME_MS);
    if (this.rig) {
      if (pose && pose.skeleton_rev === this.definition?.rev) {
        this.rig.update(pose);
        this.rig.setDimmed(!fresh);
        this.rig.root.visible = this.skeletonVisible;
      } else this.rig.root.visible = false;
    }
    this.landmarks?.update(now);
    this.grid.visible = this.floorVisible;
    this.controls.update();
    this.renderer.render(this.scene, this.camera);
  };
  private down = (e: PointerEvent): void => {
    this.pointerX = e.clientX;
    this.pointerY = e.clientY;
  };
  private pick = (e: PointerEvent): void => {
    if (Math.hypot(e.clientX - this.pointerX, e.clientY - this.pointerY) > 4)
      return;
    const targets = [this.rig, this.landmarks].flatMap((r) =>
      r?.root.visible ? r.pickables : [],
    );
    if (targets.length === 0) return;
    const rect = this.renderer.domElement.getBoundingClientRect();
    this.pointer.set(
      ((e.clientX - rect.left) / rect.width) * 2 - 1,
      -((e.clientY - rect.top) / rect.height) * 2 + 1,
    );
    this.raycaster.setFromCamera(this.pointer, this.camera);
    // The raycaster ignores visibility; skip hidden landmarks and segments.
    const hit = this.raycaster
      .intersectObjects(targets, false)
      .find((h) => h.object.visible && h.object.parent?.visible !== false);
    const data = hit?.object.userData;
    if (data) this.select((data.segmentIndex ?? data.landmarkIndex) as number);
  };
  private centre = (): void => {
    const frame = frameStore.frame;
    if (!frame?.pose) return;
    // Double-click centres the selected view around the body, without modifying engine data.
    this.controls.target.set(...frame.pose.root_position_m);
    this.controls.update();
  };
}
