/**
 * Meshes for one skeleton revision. Built once from the `SkeletonDefinition`, then updated in
 * place from each pose: no allocation per frame.
 */
import {
  BackSide,
  Group,
  Line,
  LineBasicMaterial,
  LineDashedMaterial,
  LatheGeometry,
  Mesh,
  MeshStandardMaterial,
  SphereGeometry,
  Vector2,
  Vector3,
  BufferGeometry,
  type Material,
} from "three";
import type { SkeletonDefinition, SkeletonPose } from "@contracts";
import type { Palette } from "./palette";

const INFERRED_OPACITY = 0.35;
const DIM_FACTOR = 0.35;
const RADIUS_PER_LENGTH = 0.14;
const RADIUS_MIN_M = 0.014;
const RADIUS_MAX_M = 0.045;
const DISTAL_TAPER = 0.7;
const JOINT_RADIUS_FACTOR = 0.85;
const OUTLINE_SCALE = 1.45;
const CAP_STEPS = 6;
const RADIAL_SEGMENTS = 12;
const DASH_M = 0.03;

type Style = "solid-measured" | "solid-derived" | "inferred" | "hidden";

/** Unknown provenance values are drawn as inferred: never presented as better than they may be. */
function styleOf(provenance: string): Style {
  switch (provenance) {
    case "measured":
      return "solid-measured";
    case "derived":
      return "solid-derived";
    case "unavailable":
      return "hidden";
    default:
      return "inferred";
  }
}

interface MaterialPair {
  normal: MeshStandardMaterial;
  dim: MeshStandardMaterial;
}

function fill(color: Palette["derived"], opacity: number): MaterialPair {
  const make = (o: number): MeshStandardMaterial =>
    new MeshStandardMaterial({
      color,
      roughness: 0.85,
      metalness: 0,
      transparent: o < 1,
      opacity: o,
      depthWrite: o >= 1,
    });
  return { normal: make(opacity), dim: make(opacity * DIM_FACTOR) };
}

/** A capsule along +Y from 0 to `length`, radius `r0` at the base tapering to `r1`. */
function taperedCapsule(length: number, r0: number, r1: number): LatheGeometry {
  const pts: Vector2[] = [];
  for (let i = 0; i <= CAP_STEPS; i++) {
    const a = -Math.PI / 2 + (i / CAP_STEPS) * (Math.PI / 2);
    pts.push(new Vector2(r0 * Math.cos(a), r0 * Math.sin(a)));
  }
  for (let i = 0; i <= CAP_STEPS; i++) {
    const a = (i / CAP_STEPS) * (Math.PI / 2);
    pts.push(new Vector2(r1 * Math.cos(a), length + r1 * Math.sin(a)));
  }
  return new LatheGeometry(pts, RADIAL_SEGMENTS);
}

export class SkeletonRig {
  readonly root = new Group();
  /** Meshes that can be picked; `userData.segmentIndex` is the segment index. */
  readonly pickables: Mesh[] = [];
  readonly count: number;

  private readonly bones: Group[] = [];
  private readonly capsules: (Mesh | null)[] = [];
  private readonly joints: Mesh[] = [];
  private readonly centreLines: (Line | null)[] = [];
  private readonly outlines: (Mesh | null)[] = [];
  private readonly styles: Style[] = [];
  private readonly materials: Record<Exclude<Style, "hidden">, MaterialPair>;
  private readonly lineMaterial: LineDashedMaterial;
  private readonly outlineMaterial: Material;
  private readonly disposables: { dispose(): void }[] = [];
  private dimmed = false;
  private selected = -1;

  constructor(def: SkeletonDefinition, palette: Palette) {
    this.count = def.segments.length;
    this.materials = {
      "solid-measured": fill(palette.measured, 1),
      "solid-derived": fill(palette.derived, 1),
      inferred: fill(palette.inferred, INFERRED_OPACITY),
    };
    this.lineMaterial = new LineDashedMaterial({ color: palette.inferred, dashSize: DASH_M, gapSize: DASH_M });
    this.outlineMaterial = new LineBasicMaterial(); // replaced below; keeps the field definitely assigned
    this.outlineMaterial.dispose();
    this.outlineMaterial = new MeshStandardMaterial({ color: palette.accent, side: BackSide, roughness: 1, metalness: 0 });
    for (const pair of Object.values(this.materials)) this.disposables.push(pair.normal, pair.dim);
    this.disposables.push(this.lineMaterial, this.outlineMaterial);

    const jointGeometry = new SphereGeometry(1, RADIAL_SEGMENTS, RADIAL_SEGMENTS / 2);
    this.disposables.push(jointGeometry);
    const up = new Vector3(0, 1, 0);

    def.segments.forEach((seg, i) => {
      const bone = new Group();
      bone.name = seg.id;
      const tail = new Vector3(...seg.tail_m);
      const length = tail.length();
      const r0 = Math.min(RADIUS_MAX_M, Math.max(RADIUS_MIN_M, length * RADIUS_PER_LENGTH));

      const joint = new Mesh(jointGeometry, this.materials["solid-derived"].normal);
      joint.scale.setScalar(r0 * JOINT_RADIUS_FACTOR);
      joint.userData.segmentIndex = i;
      bone.add(joint);
      this.joints.push(joint);
      this.pickables.push(joint);

      if (length > 0) {
        const geometry = taperedCapsule(length, r0, r0 * DISTAL_TAPER);
        this.disposables.push(geometry);
        const capsule = new Mesh(geometry, this.materials["solid-derived"].normal);
        capsule.quaternion.setFromUnitVectors(up, tail.clone().normalize());
        capsule.userData.segmentIndex = i;
        const outline = new Mesh(geometry, this.outlineMaterial);
        outline.scale.set(OUTLINE_SCALE, 1, OUTLINE_SCALE);
        outline.visible = false;
        capsule.add(outline);
        bone.add(capsule);
        this.capsules.push(capsule);
        this.outlines.push(outline);
        this.pickables.push(capsule);

        const lineGeometry = new BufferGeometry().setFromPoints([new Vector3(), tail]);
        this.disposables.push(lineGeometry);
        const line = new Line(lineGeometry, this.lineMaterial);
        line.computeLineDistances();
        line.visible = false;
        bone.add(line);
        this.centreLines.push(line);
      } else {
        this.capsules.push(null);
        this.outlines.push(null);
        this.centreLines.push(null);
      }

      bone.position.set(...seg.offset_m);
      this.bones.push(bone);
      this.styles.push("solid-derived");
      this.root.add(bone);
    });
  }

  /** Applies a pose whose segments are index-aligned with the definition. */
  update(pose: SkeletonPose): void {
    const segs = pose.segments;
    for (let i = 0; i < this.count; i++) {
      const s = segs[i];
      if (!s) continue;
      const bone = this.bones[i]!;
      const p = s.position_m;
      const q = s.rotation;
      bone.position.set(p[0], p[1], p[2]);
      bone.quaternion.set(q[0], q[1], q[2], q[3]);
      const style = styleOf(s.provenance);
      if (style !== this.styles[i]) {
        this.styles[i] = style;
        this.applyStyle(i);
      }
    }
  }

  setDimmed(dimmed: boolean): void {
    if (dimmed === this.dimmed) return;
    this.dimmed = dimmed;
    for (let i = 0; i < this.count; i++) this.applyStyle(i);
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

  private applyStyle(i: number): void {
    const style = this.styles[i]!;
    const visible = style !== "hidden";
    const capsule = this.capsules[i];
    const joint = this.joints[i]!;
    joint.visible = visible;
    if (capsule) capsule.visible = visible;
    const line = this.centreLines[i];
    if (line) line.visible = style === "inferred";
    if (!visible) return;
    const pair = this.materials[style];
    const material = this.dimmed ? pair.dim : pair.normal;
    joint.material = material;
    if (capsule) capsule.material = material;
  }
}
