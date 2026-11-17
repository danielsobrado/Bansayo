#![allow(clippy::expect_used, clippy::unwrap_used)]
use super::*;
use crate::content::compile::compile_technique;
use crate::observables::{Cutoffs, Observer};
use crate::testkit::{library, roundhouse_sample, skeleton};
use bansayo_contracts::{Side, Stance};

fn bank(side: Side) -> Vec<(Role, Arc<CompiledTechnique>)> {
    let lib = library();
    let rh = &lib.techniques["taekwondo.dollyeo_chagi"];
    let ap = &lib.techniques["taekwondo.ap_chagi"];
    let other = if side == Side::Right {
        Side::Left
    } else {
        Side::Right
    };
    let c =
        |t, s| Arc::new(compile_technique(t, s, Some(Stance::Orthodox), &lib).expect("compiles"));
    vec![
        (Role::Expected, c(rh, side)),
        (Role::Mirror, c(rh, other)),
        (Role::Distractor, c(ap, side)),
    ]
}

/// Runs `seconds` of the scripted roundhouse at `rate` Hz through observers and recognition.
pub fn run(side: Side, rate: f64, seconds: f64) -> Vec<RecognitionOutput> {
    let sk = skeleton(1.75);
    let mut o = Observer::new(&sk, Cutoffs::default()).unwrap();
    let mut r = Recognizer::new(RecognizerConfig::default(), bank(side));
    let n = (seconds * rate) as u64;
    (0..n)
        .flat_map(|k| {
            // A short standing lead-in, then kicks.
            let t = k as f64 / rate;
            let s = roundhouse_sample(&sk, (t - 1.0).max(0.0) + 2.6 * f64::from(t < 1.0));
            let mut s = s;
            s.t_us = (t * 1e6) as u64 + 1;
            if let Some(b) = s.body.as_mut() {
                for k in b.keypoints.iter_mut().flatten() {
                    k.evidence.t_obs_us = Some(s.t_us);
                }
                for g in &mut b.segments {
                    g.axis.t_obs_us = Some(s.t_us);
                    g.twist.t_obs_us = Some(s.t_us);
                }
            }
            let f = o.step(&s);
            r.step(&f)
        })
        .collect()
}

fn results(out: &[RecognitionOutput]) -> Vec<&Finalized> {
    out.iter()
        .filter_map(|o| match o {
            RecognitionOutput::Result(f) => Some(f.as_ref()),
            _ => None,
        })
        .collect()
}

#[test]
fn shipped_content_validates() {
    let lib = library();
    for t in lib.techniques.values() {
        let p = crate::content::validate::validate_technique(t, &lib);
        assert!(p.is_empty(), "{}: {p:?}", t.id);
    }
    for s in lib.scoring.values() {
        assert!(crate::content::validate::validate_scoring(s, &lib).is_empty());
    }
    for (i, m) in lib.metrics.values().enumerate() {
        let p = crate::content::validate::validate_metric(m, &format!("metrics[{i}]"), true);
        assert!(p.is_empty(), "{}: {p:?}", m.id);
    }
}

#[test]
fn counts_three_scripted_right_roundhouse_kicks() {
    let out = run(Side::Right, 60.0, 1.0 + 3.0 * 2.8);
    let res = results(&out);
    let done: Vec<_> = res
        .iter()
        .filter(|f| f.outcome == Outcome::Completed)
        .collect();
    assert_eq!(
        done.len(),
        3,
        "{:?}",
        res.iter()
            .map(|f| (f.outcome, f.reached, f.reason))
            .collect::<Vec<_>>()
    );
    for f in done {
        let ids: Vec<&str> = f
            .phases
            .iter()
            .map(|(p, _)| f.tech.phases[*p].id.as_str())
            .collect();
        assert_eq!(
            ids,
            ["chamber", "pivot", "extension", "retraction", "recovery"]
        );
        let (ts, _) = f.strike.expect("strike");
        let ext = f.phases[2].1;
        let ret = f.phases[3].1;
        assert!(
            ts >= ext && ts <= ret,
            "strike {ts} outside extension {ext}..{ret}"
        );
        assert!(f.confidence > 0.5);
    }
}

#[test]
fn the_other_side_reports_wrong_side() {
    let out = run(Side::Left, 60.0, 1.0 + 2.0 * 2.8);
    let res = results(&out);
    assert!(!res.is_empty());
    assert!(
        res.iter().all(|f| f.outcome == Outcome::WrongSide),
        "{:?}",
        res.iter().map(|f| f.outcome).collect::<Vec<_>>()
    );
}
