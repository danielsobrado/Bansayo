//! Validation of real takes (martial-arts 16 §5 V-19, V-20; 28 §2–3; 20 §4.1): repetition
//! annotations, counting and phase-boundary evaluation against them, clap instants for aligning
//! phone video with the session clock, and promotion of a stored take into a real golden set.
use crate::regress::{Count, Cues, Expect, GoldenSession, Manifest, TrainingFile};
use crate::training::reanalyse::{AthleteSettings, Original};
use bansayo_contracts::*;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const ANNOTATIONS_SCHEMA: &str = "bansayo.annotations/v1";
/// File beside a golden session's `recording.mcap`.
pub const ANNOTATIONS_FILE: &str = "annotations.json";

/// 20 §4.1, times on the session clock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepAnnotation {
    pub schema_version: u32,
    pub annotator: String,
    pub label: String,
    pub side: Option<Side>,
    pub phases: Vec<AnnotatedPhase>,
    pub strike_t_us: Option<u64>,
    pub quality: String,
    #[serde(default)]
    pub variation: Option<String>,
    #[serde(default)]
    pub error_tags: Vec<String>,
    #[serde(default)]
    pub cue_agreement: Vec<serde_json::Value>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnnotatedPhase {
    pub phase: String,
    pub t_start_us: u64,
    pub t_end_us: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnnotationFile {
    pub schema: String,
    pub session: SessionId,
    pub annotations: Vec<RepAnnotation>,
}

pub fn load_annotations(path: &Path) -> Result<AnnotationFile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let f: AnnotationFile =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if f.schema != ANNOTATIONS_SCHEMA {
        return Err(format!(
            "{}: schema must be {ANNOTATIONS_SCHEMA}",
            path.display()
        ));
    }
    Ok(f)
}

/// V-19 and V-20 figures for one take.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Evaluation {
    pub annotated: usize,
    pub detected: usize,
    pub matched: usize,
    pub precision: f64,
    pub recall: f64,
    /// |detected − annotated| per 20 annotated repetitions.
    pub count_error_per_20: f64,
    /// Absolute start-boundary differences of phases present in both, milliseconds.
    pub boundary_n: usize,
    pub boundary_mean_ms: f64,
    pub boundary_sd_ms: f64,
    pub boundary_max_ms: f64,
}

fn interval(a: &RepAnnotation) -> Option<(u64, u64)> {
    let start = a.phases.iter().map(|p| p.t_start_us).min()?;
    let end = a.phases.iter().map(|p| p.t_end_us).max()?;
    (end > start).then_some((start, end))
}

fn iou((a0, a1): (u64, u64), (b0, b1): (u64, u64)) -> f64 {
    let inter = a1.min(b1).saturating_sub(a0.max(b0)) as f64;
    let union = (a1.max(b1) - a0.min(b0)) as f64;
    if union > 0.0 { inter / union } else { 0.0 }
}

/// Counted detections (completed and rejected for quality) against annotated repetitions of a
/// real movement (label not `none`, quality not `unusable`); a match needs IoU ≥ 0.5 (16 §5),
/// pairs taken by highest IoU first, one to one.
pub fn evaluate(results: &[RepetitionResult], annotations: &[RepAnnotation]) -> Evaluation {
    let detected: Vec<&Repetition> = results
        .iter()
        .map(|r| &r.repetition)
        .filter(|r| {
            matches!(
                r.outcome,
                RepOutcome::Completed | RepOutcome::RejectedQuality
            )
        })
        .collect();
    let truth: Vec<(&RepAnnotation, (u64, u64))> = annotations
        .iter()
        .filter(|a| a.label != "none" && a.quality != "unusable")
        .filter_map(|a| Some((a, interval(a)?)))
        .collect();
    let mut pairs: Vec<(f64, usize, usize)> = vec![];
    for (i, d) in detected.iter().enumerate() {
        for (j, (_, t)) in truth.iter().enumerate() {
            let o = iou((d.t_start_us, d.t_end_us), *t);
            if o >= 0.5 {
                pairs.push((o, i, j));
            }
        }
    }
    pairs.sort_by(|a, b| b.0.total_cmp(&a.0));
    let (mut used_d, mut used_t) = (vec![false; detected.len()], vec![false; truth.len()]);
    let mut errors_ms = vec![];
    let mut matched = 0;
    for (_, i, j) in pairs {
        if used_d[i] || used_t[j] {
            continue;
        }
        used_d[i] = true;
        used_t[j] = true;
        matched += 1;
        for p in &truth[j].0.phases {
            if let Some(q) = detected[i].phases.iter().find(|q| q.phase == p.phase) {
                errors_ms.push(q.t_start_us.abs_diff(p.t_start_us) as f64 / 1000.0);
            }
        }
    }
    let n = errors_ms.len() as f64;
    let mean = if n > 0.0 {
        errors_ms.iter().sum::<f64>() / n
    } else {
        0.0
    };
    let sd = if n > 1.0 {
        (errors_ms.iter().map(|e| (e - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt()
    } else {
        0.0
    };
    let ratio = |a: usize, b: usize| if b == 0 { 0.0 } else { a as f64 / b as f64 };
    Evaluation {
        annotated: truth.len(),
        detected: detected.len(),
        matched,
        precision: ratio(matched, detected.len()),
        recall: ratio(matched, truth.len()),
        count_error_per_20: if truth.is_empty() {
            0.0
        } else {
            detected.len().abs_diff(truth.len()) as f64 * 20.0 / truth.len() as f64
        },
        boundary_n: errors_ms.len(),
        boundary_mean_ms: mean,
        boundary_sd_ms: sd,
        boundary_max_ms: errors_ms.iter().copied().fold(0.0, f64::max),
    }
}

/// Session times of hand claps in a session's `motion.mcap`: both wrists above the neck and
/// closer than 0.15 torso lengths in the planar body, one instant per clap (the closest approach),
/// claps at least 0.5 s apart. They align phone video with the session clock (owner checklist).
pub fn claps(session_dir: &Path) -> Result<Vec<u64>, String> {
    let rows = crate::recording::motion::read_range(
        &session_dir.join("motion.mcap"),
        crate::recording::motion::FRAME,
        0,
        u64::MAX / 1000,
    )?;
    let k = |name: &str| BODY_KEYPOINT_NAMES.iter().position(|n| *n == name);
    let (lw, rw, neck) = (
        k("left_wrist").unwrap_or(8),
        k("right_wrist").unwrap_or(12),
        k("neck").unwrap_or(3),
    );
    let mut out: Vec<u64> = vec![];
    let mut best: Option<(u64, f32)> = None;
    for (_, bytes) in rows {
        let Ok(s) = serde_json::from_slice::<MotionSample>(&bytes) else {
            continue;
        };
        let close = s.planar.first().and_then(|b| {
            let (l, r, n) = (
                b.keypoints.get(lw)?.as_ref()?,
                b.keypoints.get(rw)?.as_ref()?,
                b.keypoints.get(neck)?.as_ref()?,
            );
            let d = ((l.x - r.x).powi(2) + (l.y - r.y).powi(2)).sqrt();
            (d < 0.15 && l.y > n.y && r.y > n.y).then_some((b.t_obs_us, d))
        });
        match (close, best) {
            (Some((t, d)), Some((_, bd))) if d < bd => best = Some((t, d)),
            (Some(c), None) => best = Some(c),
            (Some(_), _) => {}
            (None, Some((t, _))) => {
                if out.last().is_none_or(|l| t.saturating_sub(*l) >= 500_000) {
                    out.push(t);
                }
                best = None;
            }
            (None, None) => {}
        }
    }
    if let Some((t, _)) = best {
        out.push(t);
    }
    Ok(out)
}

/// Copies a stored take into a real golden set (28 §2): `recording.mcap`, `session.json` and
/// the replay facts, and appends a manifest entry that needs annotations and pins content. The
/// expectations start from what the take produced; run `regress --bless` after annotating.
pub fn promote(data_dir: &Path, session: &str, golden_dir: &Path, rig: &str) -> Result<(), String> {
    let id = SessionId::new(session).map_err(|e| e.to_string())?;
    let src = data_dir.join("sessions").join(session);
    let dst = golden_dir.join("sessions").join(session);
    if !src.join("recording.mcap").is_file() {
        return Err(format!(
            "{}: no recording.mcap; the take needs motion-data consent and input recording",
            src.display()
        ));
    }
    let c = crate::store::open(&data_dir.join("db/bansayo.sqlite")).map_err(|e| e.to_string())?;
    let detail = crate::store::read::session(&c, &id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("no stored session {session}"))?;
    let Plan::Technique(plan) = detail.plan;
    let events = crate::store::read::events(&c, &id, 0, 10_000_000).map_err(|e| e.to_string())?;
    let snapshots = crate::store::read::snapshots(&c, &id).map_err(|e| e.to_string())?;
    let profile = crate::training::athletes::Athletes::new(data_dir)
        .get(&detail.session.athlete_id)
        .map_err(|e| format!("athlete {}: {e:?}", detail.session.athlete_id))?;
    let athlete = AthleteSettings {
        id: profile.id.clone(),
        stance: profile.default_stance,
        coaching: profile.coaching.level,
        tolerance_scale: profile.coaching.tolerance_scale,
    };
    std::fs::create_dir_all(&dst).map_err(|e| e.to_string())?;
    crate::recording::recompress(&src.join("recording.mcap"), &dst.join("recording.mcap"))?;
    std::fs::copy(src.join("session.json"), dst.join("session.json")).map_err(|e| e.to_string())?;
    let file = TrainingFile {
        original: Original::from_events(plan, athlete, &events),
        snapshots,
    };
    std::fs::write(
        dst.join("training.json"),
        serde_json::to_vec_pretty(&file).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let manifest_path = golden_dir.join("manifest.yaml");
    let mut manifest = if manifest_path.is_file() {
        crate::regress::load(&manifest_path)?
    } else {
        Manifest {
            schema: crate::regress::SCHEMA.into(),
            id: "golden.real".into(),
            sessions: vec![],
        }
    };
    manifest.sessions.retain(|s| s.session != id);
    manifest.sessions.push(GoldenSession {
        session: id,
        origin: detail.session.origin,
        rig: rig.into(),
        tags: vec![detail.session.title.clone()],
        annotations: Some("required".into()),
        expect: Expect {
            content: "pinned".into(),
            reps: Count {
                completed: detail.session.reps_completed,
                tolerance: 0,
            },
            outcomes: None,
            phases: None,
            metrics: vec![],
            cues: Cues { max_per_rep: 1 },
            sources: None,
            labels: Some(crate::regress::Labels {
                precision_min: 0.95,
                recall_min: 0.95,
            }),
            phase_boundary_ms: None,
            ends: match detail.session.status {
                SessionStatus::Aborted => SessionPhase::Aborted,
                _ => SessionPhase::Completed,
            },
        },
        masks: vec![],
    });
    let text = serde_saphyr::to_string(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(&manifest_path, text).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rep(i: u32, start: u64, end: u64, phases: &[(&str, u64)]) -> RepetitionResult {
        serde_json::from_value(serde_json::json!({
            "repetition": {
                "rep_index": i, "technique": "taekwondo.dollyeo_chagi@1", "side": "right",
                "stance": null, "outcome": "completed", "t_start_us": start, "t_end_us": end,
                "phases": phases.iter().map(|(p, t)| serde_json::json!({
                    "phase": p, "t_start_us": t, "t_end_us": end,
                    "evidence": { "provenance": "derived", "method": "image_plane", "confidence": null,
                                  "sigma": null, "sigma_basis": null, "t_obs_us": null, "sources": [] }
                })).collect::<Vec<_>>(),
                "strike_t_us": null, "recognition_confidence": 0.8, "capability_profile": "basic"
            },
            "measurements": [], "score": null, "cue": null, "targets": [], "scored": true, "excluded": false
        }))
        .unwrap()
    }

    fn truth(start: u64, end: u64, ext: u64) -> RepAnnotation {
        RepAnnotation {
            schema_version: 1,
            annotator: "a".into(),
            label: "taekwondo.dollyeo_chagi".into(),
            side: Some(Side::Right),
            phases: vec![
                AnnotatedPhase {
                    phase: "chamber".into(),
                    t_start_us: start,
                    t_end_us: ext,
                },
                AnnotatedPhase {
                    phase: "extension".into(),
                    t_start_us: ext,
                    t_end_us: end,
                },
            ],
            strike_t_us: None,
            quality: "positive".into(),
            variation: None,
            error_tags: vec![],
            cue_agreement: vec![],
            note: None,
        }
    }

    #[test]
    fn counting_and_boundaries_against_annotations() {
        let detected = [
            rep(
                1,
                1_000_000,
                2_000_000,
                &[("chamber", 1_010_000), ("extension", 1_400_000)],
            ),
            rep(2, 4_000_000, 5_000_000, &[("extension", 4_430_000)]),
            // A false detection overlapping nothing.
            rep(3, 8_000_000, 8_400_000, &[]),
        ];
        let annotated = [
            truth(1_000_000, 2_000_000, 1_420_000),
            truth(4_000_000, 5_000_000, 4_400_000),
            // Missed, and one too short to overlap anything by IoU 0.5.
            truth(10_000_000, 11_000_000, 10_400_000),
            truth(4_000_000, 4_100_000, 4_050_000),
        ];
        let e = evaluate(&detected, &annotated);
        assert_eq!((e.annotated, e.detected, e.matched), (4, 3, 2));
        assert!((e.precision - 2.0 / 3.0).abs() < 1e-9 && (e.recall - 0.5).abs() < 1e-9);
        assert!((e.count_error_per_20 - 5.0).abs() < 1e-9);
        // chamber 10 ms, extension 20 ms and 30 ms.
        assert_eq!(e.boundary_n, 3);
        assert!((e.boundary_mean_ms - 20.0).abs() < 1e-9 && e.boundary_max_ms == 30.0);
        assert!((e.boundary_sd_ms - 10.0).abs() < 1e-9);
    }
}
