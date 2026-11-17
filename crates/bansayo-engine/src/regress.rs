//! Regression runner (martial-arts 28 §3, §5; ADR-038): re-analyses each golden session (and
//! each source mask) with pinned content and compares the result with the manifest's
//! expectations. `bless` writes the measured values back for review as a diff.
use crate::content_store::{ContentStore, Snapshot};
use crate::training::reanalyse::{Original, mask, run};
use bansayo_contracts::*;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const SCHEMA: &str = "bansayo.golden/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: String,
    pub id: String,
    pub sessions: Vec<GoldenSession>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenSession {
    pub session: SessionId,
    pub origin: Origin,
    pub rig: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub expect: Expect,
    #[serde(default)]
    pub masks: Vec<MaskSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskSpec {
    pub name: String,
    pub disable: Vec<DeviceId>,
    pub expect: Expect,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Count {
    pub completed: u32,
    #[serde(default)]
    pub tolerance: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phases {
    /// Largest allowed difference from the blessed boundaries.
    pub max_abs_ms: f64,
    /// Blessed phase starts per completed repetition, µs, written by `bless`.
    #[serde(default)]
    pub starts_us: Vec<Vec<u64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetricExpect {
    pub metric: String,
    /// `session_median` of the completed repetitions' values.
    pub stat: String,
    /// Written by `bless`; `null` when every repetition is suppressed.
    #[serde(default)]
    pub value: Option<f64>,
    pub tolerance: f64,
    /// Every completed repetition's grade.
    pub grade: Grade,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Cues {
    pub max_per_rep: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    pub content: String,
    pub reps: Count,
    /// Outcomes of every stored repetition in order, e.g. `[completed, completed, wrong_side]`.
    #[serde(default)]
    pub outcomes: Option<Vec<RepOutcome>>,
    #[serde(default)]
    pub phases: Option<Phases>,
    #[serde(default)]
    pub metrics: Vec<MetricExpect>,
    pub cues: Cues,
    /// Final session phase.
    pub ends: SessionPhase,
}

/// What a golden session directory keeps besides `recording.mcap` and `session.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingFile {
    pub original: Original,
    pub snapshots: Vec<Snapshot>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Check {
    pub session: String,
    pub run: String,
    pub name: String,
    pub pass: bool,
    pub detail: String,
}

pub struct Report {
    pub checks: Vec<Check>,
    /// The manifest with measured values, for `bless`.
    pub blessed: Manifest,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|c| c.pass)
    }
}

fn results(events: &[TrainingEvent]) -> Vec<RepetitionResult> {
    events
        .iter()
        .filter_map(|e| match e.event_type.as_str() {
            "rep_completed" => serde_json::from_value(e.data.clone()).ok(),
            "rep_rejected" => serde_json::from_value(e.data["result"].clone()).ok(),
            _ => None,
        })
        .collect()
}

/// Evaluates one run; returns the checks and the expectation with measured values filled in.
fn evaluate(session: &str, run_name: &str, expect: &Expect, events: &[TrainingEvent]) -> (Vec<Check>, Expect) {
    let mut out = vec![];
    let mut check = |name: &str, pass: bool, detail: String| {
        out.push(Check { session: session.into(), run: run_name.into(), name: name.into(), pass, detail })
    };
    let reps = results(events);
    let completed: Vec<&RepetitionResult> = reps.iter().filter(|r| r.repetition.outcome == RepOutcome::Completed).collect();
    let n = completed.len() as i64;
    let want = i64::from(expect.reps.completed);
    check("reps.completed", (n - want).unsigned_abs() <= u64::from(expect.reps.tolerance), format!("{n} completed, expected {want} ± {}", expect.reps.tolerance));
    let mut blessed = expect.clone();
    let outcomes: Vec<RepOutcome> = reps.iter().map(|r| r.repetition.outcome).collect();
    if let Some(o) = &expect.outcomes {
        check("outcomes", o == &outcomes, format!("{outcomes:?}"));
    }
    blessed.outcomes = Some(outcomes);
    let ended = events.iter().rev().find_map(|e| match e.event_type.as_str() {
        "session_completed" => Some(SessionPhase::Completed),
        "session_aborted" => Some(SessionPhase::Aborted),
        _ => None,
    });
    check("ends", ended == Some(expect.ends), format!("{ended:?}"));
    let measured_starts: Vec<Vec<u64>> = completed.iter().map(|r| r.repetition.phases.iter().map(|p| p.t_start_us).collect()).collect();
    if let Some(p) = &expect.phases {
        let worst = measured_starts
            .iter()
            .zip(&p.starts_us)
            .flat_map(|(m, e)| m.iter().zip(e).map(|(a, b)| a.abs_diff(*b)))
            .max()
            .unwrap_or(0) as f64
            / 1000.0;
        let shape = measured_starts.len() == p.starts_us.len() && measured_starts.iter().zip(&p.starts_us).all(|(m, e)| m.len() == e.len());
        check("phases", shape && worst <= p.max_abs_ms, format!("largest boundary difference {worst:.1} ms (max {})", p.max_abs_ms));
        blessed.phases = Some(Phases { max_abs_ms: p.max_abs_ms, starts_us: measured_starts });
    }
    blessed.metrics = expect
        .metrics
        .iter()
        .map(|m| {
            let ms: Vec<&MetricMeasurement> = completed
                .iter()
                .filter_map(|r| r.measurements.iter().find(|x| x.metric.split('@').next() == Some(m.metric.as_str())))
                .collect();
            let grades_ok = !ms.is_empty() && ms.iter().all(|x| x.grade == m.grade);
            let mut values: Vec<f64> = ms.iter().filter_map(|x| x.value).collect();
            values.sort_by(f64::total_cmp);
            let value = bansayo_training::progress::median(&values);
            let close = match (value, m.value) {
                (Some(a), Some(b)) => (a - b).abs() <= m.tolerance,
                (None, None) => true,
                _ => false,
            };
            check(&format!("metric {}", m.metric), grades_ok && close, format!("median {value:?} (expected {:?} ± {}), grades {:?}", m.value, m.tolerance, ms.iter().map(|x| x.grade).collect::<Vec<_>>()));
            MetricExpect { value, ..m.clone() }
        })
        .collect();
    let worst_cues = reps.iter().map(|r| u32::from(r.cue.is_some())).max().unwrap_or(0);
    let cue_events = events.iter().filter(|e| e.event_type == "rep_completed").filter(|e| !e.data["cue"].is_null()).count();
    check("cues.max_per_rep", worst_cues <= expect.cues.max_per_rep, format!("at most {worst_cues} per repetition, {cue_events} in total"));
    (out, blessed)
}

pub fn load(manifest: &Path) -> Result<Manifest, String> {
    let text = std::fs::read_to_string(manifest).map_err(|e| format!("{}: {e}", manifest.display()))?;
    let m: Manifest = serde_saphyr::from_str(&text).map_err(|e| format!("{}: {e}", manifest.display()))?;
    if m.schema != SCHEMA {
        return Err(format!("{}: schema must be {SCHEMA}", manifest.display()));
    }
    Ok(m)
}

/// Golden sessions live under `<manifest dir>/sessions/<id>/`.
pub fn run_manifest(manifest_path: &Path) -> Result<Report, String> {
    let manifest = load(manifest_path)?;
    let dir: PathBuf = manifest_path.parent().map(Path::to_owned).unwrap_or_default();
    let mut checks = vec![];
    let mut blessed = manifest.clone();
    for (k, g) in manifest.sessions.iter().enumerate() {
        let file = dir.join("sessions").join(g.session.as_str()).join("training.json");
        let training: TrainingFile = serde_json::from_slice(&std::fs::read(&file).map_err(|e| format!("{}: {e}", file.display()))?)
            .map_err(|e| format!("{}: {e}", file.display()))?;
        if g.expect.content != "pinned" {
            return Err(format!("{}: only pinned content is supported", g.session));
        }
        let library = ContentStore::library_from_snapshots(&training.snapshots)?;
        let mut runs = vec![("all sources".to_owned(), vec![], g.expect.clone())];
        runs.extend(g.masks.iter().map(|m| (m.name.clone(), m.disable.clone(), m.expect.clone())));
        for (i, (name, disable, expect)) in runs.into_iter().enumerate() {
            let recording = crate::recording::reader::load(&dir, &g.session)?;
            let out = run(mask(recording, &disable), &training.original, &library, g.session.clone())?;
            let (c, b) = evaluate(g.session.as_str(), &name, &expect, &out.events);
            checks.extend(c);
            if i == 0 {
                blessed.sessions[k].expect = b;
            } else {
                blessed.sessions[k].masks[i - 1].expect = b;
            }
        }
    }
    Ok(Report { checks, blessed })
}

/// `bansayo-engine regress <manifest> [--bless]`: prints every check; exit code 1 on failure.
pub fn main(manifest: &Path, bless: bool) -> i32 {
    let report = match run_manifest(manifest) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("regress: {e}");
            return 2;
        }
    };
    for c in &report.checks {
        println!("{} {:<26} {:<14} {:<28} {}", if c.pass { "pass" } else { "FAIL" }, c.session, c.run, c.name, c.detail);
    }
    let failed = report.checks.iter().filter(|c| !c.pass).count();
    println!("{} checks, {} failed; engine {}, observables rev {}", report.checks.len(), failed, env!("CARGO_PKG_VERSION"), bansayo_training::observables::OBSERVABLES_REV);
    if bless {
        match serde_saphyr::to_string(&report.blessed) {
            Ok(text) => {
                if let Err(e) = std::fs::write(manifest, format!("# Expected values are written by `regress --bless`; review the diff.\n{text}")) {
                    eprintln!("regress: {e}");
                    return 2;
                }
                println!("blessed {}", manifest.display());
                return 0;
            }
            Err(e) => {
                eprintln!("regress: {e}");
                return 2;
            }
        }
    }
    i32::from(failed > 0)
}
