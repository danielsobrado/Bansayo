//! Detector schema maps (martial-arts 04 §2.2): data tables converting a detector's landmarks to
//! the canonical keypoint set `bansayo.body.v1.points`. The list of schemas is closed in code.
use bansayo_contracts::{BODY_KEYPOINT_SCHEMA_ID, Landmark2D};

/// One canonical point as a weighted sum of detector landmarks.
type Entry = (usize, &'static [(usize, f32)]);

/// COCO-17: no clavicles, hand tips, toes or heels. `spine` and `chest` would be inferred on the
/// pelvis–neck line; inferred points are left out of `PlanarBody`, which carries no provenance.
const COCO17: &[Entry] = &[
    (0, &[(11, 0.5), (12, 0.5)]), // pelvis: hip midpoint
    (3, &[(5, 0.5), (6, 0.5)]),   // neck: shoulder midpoint
    (4, &[(3, 0.5), (4, 0.5)]),   // head: ear midpoint
    (6, &[(5, 1.0)]),
    (7, &[(7, 1.0)]),
    (8, &[(9, 1.0)]),
    (10, &[(6, 1.0)]),
    (11, &[(8, 1.0)]),
    (12, &[(10, 1.0)]),
    (13, &[(11, 1.0)]),
    (14, &[(13, 1.0)]),
    (15, &[(15, 1.0)]),
    (16, &[(12, 1.0)]),
    (17, &[(14, 1.0)]),
    (18, &[(16, 1.0)]),
];

/// Canonical points of a detector schema, or `None` for an unsupported schema
/// (`landmark_schema_unsupported`). A point needs every contributing landmark; its confidence is
/// the lowest of them.
pub fn to_canonical(
    schema_id: &str,
    points: &[Option<Landmark2D>],
) -> Option<Vec<Option<Landmark2D>>> {
    let table = match schema_id {
        super::skeleton_fit::COCO17_SCHEMA_ID => COCO17,
        // Simulated cameras already project canonical points (the first 19 or all 26).
        s if s == BODY_KEYPOINT_SCHEMA_ID
            || s == crate::skeleton::proportions::LANDMARK_SCHEMA_ID =>
        {
            let mut out = vec![None; 26];
            for (o, p) in out.iter_mut().zip(points) {
                *o = p.clone();
            }
            return Some(out);
        }
        _ => return None,
    };
    let mut out = vec![None; 26];
    for (canonical, sources) in table {
        let mut sum = (0.0f32, 0.0f32);
        let mut confidence = f32::INFINITY;
        let mut complete = true;
        for (index, weight) in *sources {
            match points.get(*index).and_then(Option::as_ref) {
                Some(p) => {
                    sum.0 += p.x_px * weight;
                    sum.1 += p.y_px * weight;
                    confidence = confidence.min(p.confidence);
                }
                None => complete = false,
            }
        }
        if complete {
            out[*canonical] = Some(Landmark2D {
                x_px: sum.0,
                y_px: sum.1,
                confidence,
            });
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(x: f32, y: f32, c: f32) -> Option<Landmark2D> {
        Some(Landmark2D {
            x_px: x,
            y_px: y,
            confidence: c,
        })
    }
    #[test]
    fn coco17_maps_midpoints_with_the_weakest_confidence_and_leaves_feet_absent() {
        let mut coco = vec![None; 17];
        coco[5] = p(10.0, 20.0, 0.9);
        coco[6] = p(30.0, 20.0, 0.5);
        coco[11] = p(12.0, 60.0, 0.8);
        coco[15] = p(12.0, 100.0, 0.7);
        let c = to_canonical("coco17.body", &coco).unwrap();
        let neck = c[3].as_ref().unwrap();
        assert_eq!((neck.x_px, neck.y_px, neck.confidence), (20.0, 20.0, 0.5));
        assert!(c[0].is_none(), "pelvis needs both hips");
        assert_eq!(c[15].as_ref().unwrap().y_px, 100.0);
        assert!(c[19..].iter().all(Option::is_none));
        assert!(to_canonical("unknown", &coco).is_none());
    }
}
