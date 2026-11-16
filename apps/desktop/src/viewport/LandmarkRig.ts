/**
 * Triangulated landmarks for one `LandmarkSchema`: a marker per landmark and the schema edges as
 * bones. Built once, then updated in place from `landmarks3dStore`: no allocation per frame.
 */
import {
  BackSide,
  BufferAttribute,
  BufferGeometry,
  Group,
  LineBasicMaterial,
  LineSegments,
  Mesh,
  MeshBasicMaterial,
  MeshStandardMaterial,
  OctahedronGeometry,
  SphereGeometry,
} from "three";
import type { LandmarkSchema } from "@contracts";
import { HIGH_REPROJECTION_PX } from "../engine/readouts";
import { landmarks3dStore, STALE_FRAME_MS } from "../engine/stores";
import type { Palette } from "./palette";

const MARKER_M = 0.024;
/** Invisible, larger sphere so small markers are easy to click. */
const PICK_M = 0.06;
const OUTLINE_SCALE = 1.7;
const ERROR_MARK_M = 0.055;
const MIN_OPACITY = 0.25;
const MIN_SIZE = 0.6;

export class LandmarkRig {
  readonly root = new Group();
  /** Pick proxies; `userData.landmarkIndex` is the landmark index. */
  readonly pickables: Mesh[] = [];
  private readonly nodes: Group[] = [];
  private readonly markers: Mesh[] = [];
  private readonly materials: MeshStandardMaterial[] = [];
  private readonly errorMarks: Mesh[] = [];
  private readonly outlines: Mesh[] = [];
  private readonly positions: Float32Array;
  private readonly bones: LineSegments;
  private readonly disposables: { dispose(): void }[] = [];
  private selected = -1;

  constructor(
    private readonly schema: LandmarkSchema,
    palette: Palette,
  ) {
    const sphere = new SphereGeometry(1, 12, 6);
    const octahedron = new OctahedronGeometry(ERROR_MARK_M);
    const outlineMaterial = new MeshStandardMaterial({
      color: palette.accent,
      side: BackSide,
      roughness: 1,
      metalness: 0,
    });
    const errorMaterial = new MeshBasicMaterial({
      color: palette.degraded,
      wireframe: true,
    });
    const pickMaterial = new MeshBasicMaterial({ visible: false });
    this.disposables.push(
      sphere,
      octahedron,
      outlineMaterial,
      errorMaterial,
      pickMaterial,
    );
    schema.names.forEach((name, i) => {
      const node = new Group();
      node.name = name;
      node.visible = false;
      const material = new MeshStandardMaterial({
        color: palette.derived,
        roughness: 0.85,
        metalness: 0,
        transparent: true,
      });
      this.disposables.push(material);
      const marker = new Mesh(sphere, material);
      const outline = new Mesh(sphere, outlineMaterial);
      outline.scale.setScalar(MARKER_M * OUTLINE_SCALE);
      outline.visible = false;
      const errorMark = new Mesh(octahedron, errorMaterial);
      const pick = new Mesh(sphere, pickMaterial);
      pick.scale.setScalar(PICK_M);
      pick.userData.landmarkIndex = i;
      node.add(marker, outline, errorMark, pick);
      this.root.add(node);
      this.nodes.push(node);
      this.markers.push(marker);
      this.materials.push(material);
      this.outlines.push(outline);
      this.errorMarks.push(errorMark);
      this.pickables.push(pick);
    });
    this.positions = new Float32Array(schema.edges.length * 6);
    const geometry = new BufferGeometry();
    geometry.setAttribute("position", new BufferAttribute(this.positions, 3));
    const boneMaterial = new LineBasicMaterial({ color: palette.derived });
    this.bones = new LineSegments(geometry, boneMaterial);
    this.bones.frustumCulled = false; // positions change every frame; the bounds would go stale
    this.root.add(this.bones);
    this.disposables.push(geometry, boneMaterial);
  }

  /** Draws the latest set if it is fresh and matches the schema; otherwise hides everything. */
  update(now: number): void {
    const set = landmarks3dStore.landmarks;
    const live =
      set !== null &&
      set.schema_id === this.schema.id &&
      now - landmarks3dStore.receivedAt <= STALE_FRAME_MS;
    this.root.visible = live;
    if (!live) return;
    for (let i = 0; i < this.nodes.length; i++) {
      const p = set.points[i];
      const node = this.nodes[i]!;
      node.visible = !!p;
      if (!p) continue;
      node.position.set(p.m[0], p.m[1], p.m[2]);
      // Confidence is opacity and size; a high reprojection error adds a wire octahedron.
      const c = Math.min(1, Math.max(0, p.confidence));
      this.materials[i]!.opacity = MIN_OPACITY + (1 - MIN_OPACITY) * c;
      this.markers[i]!.scale.setScalar(MARKER_M * (MIN_SIZE + (1 - MIN_SIZE) * c));
      this.errorMarks[i]!.visible =
        p.reprojection_error_px > HIGH_REPROJECTION_PX;
    }
    const edges = this.schema.edges;
    for (let e = 0; e < edges.length; e++) {
      const a = set.points[edges[e]![0]];
      const b = set.points[edges[e]![1]];
      // A bone with an unreconstructed end collapses to a zero-length segment: not drawn.
      for (let k = 0; k < 3; k++) {
        this.positions[e * 6 + k] = a && b ? a.m[k]! : 0;
        this.positions[e * 6 + 3 + k] = a && b ? b.m[k]! : 0;
      }
    }
    this.bones.geometry.getAttribute("position").needsUpdate = true;
  }

  setSelected(index: number): void {
    if (index === this.selected) return;
    const prev = this.outlines[this.selected];
    if (prev) prev.visible = false;
    this.selected = index;
    const next = this.outlines[index];
    if (next) next.visible = true;
  }

  dispose(): void {
    this.root.removeFromParent();
    for (const d of this.disposables) d.dispose();
  }
}
