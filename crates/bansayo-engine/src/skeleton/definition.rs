//! Builds the skeleton at a given height and resolves ids to indices once.

use std::collections::HashMap;

use bansayo_contracts::{
    AngleUnit, CalibrationState, JointDef, LandmarkSchema, SegmentDef, SegmentId,
    SkeletonDefinition,
};

use super::proportions::{
    CENTRE, HEEL, JOINTS, JointKind, LANDMARK_SCHEMA_ID, LEFT_ARM, LEFT_LEG, MODEL_ID, Row, TOE,
};
use crate::math::{V3, v3_to_wire};

/// Revision of the built-in definition; increments when the definition or its scaling changes.
pub const SKELETON_REV: u32 = 1;

/// One segment, scaled to metres.
#[derive(Debug, Clone)]
pub struct Segment {
    pub id: SegmentId,
    pub parent: Option<usize>,
    /// Origin in the parent's frame at rest; for the root, its rest position in world.
    pub offset: V3,
    /// Origin to distal end, in this segment's frame.
    pub tail: V3,
    pub contact_points: Vec<V3>,
}

/// One joint measurement, resolved to segment indices.
#[derive(Debug, Clone)]
pub struct Joint {
    pub a: usize,
    pub b: usize,
    pub kind: JointKind,
}

/// The active skeleton: the contract definition plus index-resolved data for hot paths.
#[derive(Debug, Clone)]
pub struct Skeleton {
    pub definition: SkeletonDefinition,
    pub segments: Vec<Segment>,
    pub joints: Vec<Joint>,
    index: HashMap<SegmentId, usize>,
}

/// The model id is not built in.
#[derive(Debug, thiserror::Error)]
#[error("skeleton model {0:?} is not built into this engine (available: {MODEL_ID})")]
pub struct UnknownModel(pub String);

type ResolvedRow = (String, Option<String>, [f64; 3], [f64; 3]);

fn mirror(row: &Row) -> ResolvedRow {
    let flip = |v: [f64; 3]| [-v[0], v[1], v[2]];
    let name = |s: &str| s.replacen("left_", "right_", 1);
    (name(row.0), row.1.map(name), flip(row.2), flip(row.3))
}

fn plain(row: &Row) -> ResolvedRow {
    (row.0.to_owned(), row.1.map(str::to_owned), row.2, row.3)
}

fn rows() -> Vec<ResolvedRow> {
    CENTRE
        .iter()
        .map(plain)
        .chain(LEFT_ARM.iter().map(plain))
        .chain(LEFT_ARM.iter().map(mirror))
        .chain(LEFT_LEG.iter().map(plain))
        .chain(LEFT_LEG.iter().map(mirror))
        .collect()
}

/// Rounds to the micrometre so the published definition has short decimals and no `-0.0`.
fn micrometres(v: V3) -> V3 {
    v.map(|c| (c * 1e6).round() / 1e6 + 0.0)
}

fn scaled(v: [f64; 3], height_m: f64) -> V3 {
    micrometres(V3::from(v) * height_m)
}

fn segment_id(s: &str) -> SegmentId {
    // The proportion table is static and every name in it matches the segment pattern.
    SegmentId::new(s).unwrap_or_else(|_| unreachable!("invalid built-in segment id {s}"))
}

impl Skeleton {
    /// Builds a built-in model scaled to `height_m`.
    pub fn builtin(model: &str, height_m: f64) -> Result<Self, UnknownModel> {
        if model != MODEL_ID {
            return Err(UnknownModel(model.to_owned()));
        }
        let rows = rows();
        let index: HashMap<SegmentId, usize> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| (segment_id(&r.0), i))
            .collect();
        let segments: Vec<Segment> = rows
            .iter()
            .map(|(id, parent, offset, tail)| Segment {
                id: segment_id(id),
                parent: parent.as_deref().map(|p| index[&segment_id(p)]),
                offset: scaled(*offset, height_m),
                tail: scaled(*tail, height_m),
                contact_points: if id.ends_with("_foot") {
                    vec![scaled(HEEL, height_m), scaled(TOE, height_m)]
                } else {
                    vec![]
                },
            })
            .collect();
        let joints = JOINTS
            .iter()
            .map(|&(_, _, a, b, _, kind)| Joint {
                a: index[&segment_id(a)],
                b: index[&segment_id(b)],
                kind,
            })
            .collect();
        let definition = SkeletonDefinition {
            model_id: MODEL_ID.into(),
            rev: SKELETON_REV,
            height_m: height_m as f32,
            proportions: CalibrationState::Assumed,
            segments: segments
                .iter()
                .map(|s| SegmentDef {
                    id: s.id.clone(),
                    parent: s.parent.map(|p| segments[p].id.clone()),
                    offset_m: v3_to_wire(&s.offset),
                    tail_m: v3_to_wire(&s.tail),
                    contact_points_m: s.contact_points.iter().map(v3_to_wire).collect(),
                })
                .collect(),
            joints: JOINTS
                .iter()
                .map(|&(id, label, a, b, method, _)| JointDef {
                    id: id.into(),
                    label: label.into(),
                    segments: [segment_id(a), segment_id(b)],
                    method,
                    unit: AngleUnit::Rad,
                    validated: false,
                })
                .collect(),
        };
        Ok(Self {
            definition,
            segments,
            joints,
            index,
        })
    }

    pub fn index_of(&self, id: &SegmentId) -> Option<usize> {
        self.index.get(id).copied()
    }

    pub fn len(&self) -> usize {
        self.segments.len()
    }

    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// `bansayo.body.v1.joints`: a landmark per segment origin, an edge per parent link.
    pub fn landmark_schema(&self) -> LandmarkSchema {
        LandmarkSchema {
            id: LANDMARK_SCHEMA_ID.into(),
            names: self.segments.iter().map(|s| s.id.to_string()).collect(),
            edges: self
                .segments
                .iter()
                .enumerate()
                .filter_map(|(i, s)| s.parent.map(|p| [p as u32, i as u32]))
                .collect(),
        }
    }
}
