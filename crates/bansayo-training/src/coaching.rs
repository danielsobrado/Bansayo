//! Cue conditions, the evidence gate and the `after_rep` / `after_set` scheduler (07 §8).
use crate::content::types::{BeyondBound, ComponentSource, CueDef, CueWhen, LowHigh, ScoringModel};
use bansayo_contracts::{
    CoachingLevel, CueCategory, CueChannel, CueEvidence, CueInstance, Grade, ScoreResult,
    TextFormat, TextParam, TextRef,
};
use std::collections::BTreeMap;

/// One cue's evaluation on one scored repetition.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Held {
    held: bool,
    /// Worst grade among the involved measurements.
    grade: Grade,
    /// Deviation / outer tolerance.
    norm_dev: f64,
}

#[derive(Debug, Clone, Default)]
struct RepRecord {
    cues: BTreeMap<String, Held>,
    components: BTreeMap<String, f64>,
    within_inner: BTreeMap<String, bool>,
}

#[derive(Debug, Clone)]
pub struct Scheduler {
    pub level: CoachingLevel,
    pub cooldown_reps: u32,
    pub slow_stage: bool,
    set_target: u32,
    reps: Vec<RepRecord>,
    issued: BTreeMap<String, (usize, f64)>,
    last_had_cue: bool,
    previous_set: BTreeMap<String, f64>,
}

fn format_for(unit: &str) -> (TextFormat, &'static str) {
    match unit {
        "rad" => (TextFormat::Deg, "deviation_deg"),
        "m" => (TextFormat::Cm, "deviation_cm"),
        "ms" => (TextFormat::Ms, "deviation_ms"),
        _ => (TextFormat::Percent, "deviation_percent"),
    }
}

fn component_def<'a>(
    model: &'a ScoringModel,
    id: &str,
) -> Option<&'a crate::content::types::ComponentDef> {
    model.components.iter().find(|c| c.id == id)
}

/// Evaluates a cue condition on one repetition (component and metric forms only).
fn evaluate(
    cue: &CueDef,
    score: &ScoreResult,
    model: &ScoringModel,
) -> Option<(Held, Option<CueEvidence>)> {
    let (component, beyond) = match &cue.when {
        CueWhen::Component { component, below } => (component.clone(), Err(*below)),
        CueWhen::Metric {
            metric,
            beyond,
            direction,
        } => {
            let c = model.components.iter().find(
                |c| matches!(&c.source, ComponentSource::Metric { metric: m, .. } if m == metric),
            )?;
            (c.id.clone(), Ok((*beyond, *direction)))
        }
        // `trend` and `step` are not evaluated in roadmap Phase 1.
        _ => return None,
    };
    let c = score.components.iter().find(|c| c.id == component)?;
    let def = component_def(model, &component)?;
    let m = c.measurement.as_ref()?;
    let s = c.score?;
    let outer = def.tolerance.as_ref().map_or(1.0, |t| t.outer);
    let dev = c.deviation.unwrap_or(0.0);
    let held = match beyond {
        Err(below) => s < below,
        Ok((bound, dir)) => {
            let t = def.tolerance.as_ref()?;
            let limit = if bound == BeyondBound::Inner {
                t.inner
            } else {
                t.outer
            };
            match dir {
                LowHigh::Low => -dev > limit,
                LowHigh::High => dev > limit,
            }
        }
    };
    Some((
        Held {
            held,
            grade: m.grade,
            norm_dev: (dev.abs() / outer).min(10.0),
        },
        Some(CueEvidence {
            metric: m.metric.clone(),
            grade: m.grade,
            deviation: c.deviation,
        }),
    ))
}

fn instance(
    cue: &CueDef,
    channel: CueChannel,
    evidence: Vec<CueEvidence>,
    unit: &str,
    resolved: bool,
) -> CueInstance {
    let mut params = BTreeMap::new();
    if let Some(d) = evidence.first().and_then(|e| e.deviation) {
        let (format, name) = format_for(unit);
        params.insert(
            name.to_owned(),
            TextParam {
                value: d.abs(),
                format,
            },
        );
    }
    CueInstance {
        id: cue.id.clone(),
        channel,
        category: cue.category,
        basis: cue.basis,
        observed: TextRef {
            key: if resolved {
                "cue.resolved".into()
            } else {
                cue.observed.clone()
            },
            params: params.clone(),
        },
        suggestion: TextRef {
            key: cue.suggestion.clone(),
            params,
        },
        drill: cue.drill.clone(),
        evidence,
        resolved,
    }
}

impl Scheduler {
    pub fn new(level: CoachingLevel, cooldown_reps: u32, slow_stage: bool) -> Self {
        Self {
            level,
            cooldown_reps,
            slow_stage,
            set_target: 0,
            reps: vec![],
            issued: BTreeMap::new(),
            last_had_cue: false,
            previous_set: BTreeMap::new(),
        }
    }

    pub fn start_set(&mut self, target_reps: u32) {
        if !self.reps.is_empty() {
            self.previous_set = self.set_means();
        }
        self.set_target = target_reps;
        self.reps.clear();
        self.issued.clear();
        self.last_had_cue = false;
    }

    fn set_means(&self) -> BTreeMap<String, f64> {
        let mut sums: BTreeMap<String, (f64, f64)> = BTreeMap::new();
        for r in &self.reps {
            for (k, v) in &r.components {
                let e = sums.entry(k.clone()).or_default();
                e.0 += v;
                e.1 += 1.0;
            }
        }
        sums.into_iter().map(|(k, (s, n))| (k, s / n)).collect()
    }

    fn disabled(&self, cue: &CueDef) -> bool {
        self.slow_stage && cue.category == CueCategory::Timing
    }

    /// Gate of 07 §8.2 step 1 over the latest scored repetitions, this one included.
    fn gate(&self, id: &str, min_grade: Grade) -> bool {
        let recent: Vec<Held> = self
            .reps
            .iter()
            .rev()
            .take(4)
            .filter_map(|r| r.cues.get(id).copied())
            .collect();
        let Some(now) = recent.first() else {
            return false;
        };
        if !now.held || now.grade > min_grade || now.grade > Grade::B {
            return false;
        }
        let ok = |h: &Held| h.held && h.grade <= min_grade && h.grade <= Grade::B;
        let all_a = recent.iter().take(4).all(|h| h.grade == Grade::A);
        if all_a {
            recent.iter().take(3).filter(|h| ok(h)).count() >= 2
        } else {
            recent.iter().take(4).filter(|h| ok(h)).count() >= 3
        }
    }

    /// Records one scored repetition and returns at most one cue for it.
    pub fn after_rep(
        &mut self,
        cues: &[CueDef],
        score: &ScoreResult,
        model: &ScoringModel,
    ) -> Option<CueInstance> {
        let mut rec = RepRecord::default();
        let mut evidence = BTreeMap::new();
        for c in &score.components {
            if let Some(s) = c.score {
                rec.components.insert(c.id.clone(), s);
                if let (Some(d), Some(def)) = (c.deviation, component_def(model, &c.id))
                    && let Some(t) = &def.tolerance
                {
                    let miss = match def.direction {
                        Some(crate::content::types::ScoreDirection::AtLeast) => (-d).max(0.0),
                        Some(crate::content::types::ScoreDirection::AtMost) => d.max(0.0),
                        _ => d.abs(),
                    };
                    rec.within_inner.insert(c.id.clone(), miss <= t.inner);
                }
            }
        }
        for cue in cues {
            if let Some((h, e)) = evaluate(cue, score, model) {
                rec.cues.insert(cue.id.clone(), h);
                evidence.insert(cue.id.clone(), e);
            }
        }
        self.reps.push(rec);
        let n = self.reps.len();
        let unit_of = |e: &Option<CueEvidence>| {
            e.as_ref()
                .and_then(|e| {
                    score
                        .components
                        .iter()
                        .find_map(|c| c.measurement.as_ref().filter(|m| m.metric == e.metric))
                })
                .map_or("", |m| m.unit.as_str())
                .to_owned()
        };
        let faded = match self.level {
            CoachingLevel::Minimal => true,
            CoachingLevel::Standard => self.set_target >= 8 && self.last_had_cue,
            CoachingLevel::Detailed => false,
        };
        let mut chosen: Option<CueInstance> = None;
        if !faded {
            let mut ready: Vec<&CueDef> = cues
                .iter()
                .filter(|c| !self.disabled(c) && self.gate(&c.id, c.min_grade))
                .filter(|c| {
                    let dev = self.reps[n - 1].cues[&c.id].norm_dev;
                    match self.issued.get(&c.id) {
                        Some((at, d)) => n - at > self.cooldown_reps as usize || dev >= d + 0.25,
                        None => true,
                    }
                })
                .collect();
            let dev = |c: &CueDef| self.reps[n - 1].cues[&c.id].norm_dev;
            ready.sort_by(|a, b| {
                a.category
                    .cmp(&b.category)
                    .then(a.priority.cmp(&b.priority))
                    .then(dev(b).total_cmp(&dev(a)))
            });
            if let Some(c) = ready.first() {
                self.issued.insert(c.id.clone(), (n, dev(c)));
                let e = evidence.get(&c.id).cloned().flatten();
                let unit = unit_of(&e);
                chosen = Some(instance(
                    c,
                    CueChannel::AfterRep,
                    e.into_iter().collect(),
                    &unit,
                    false,
                ));
            }
            if chosen.is_none() {
                chosen = self.resolution(cues, &evidence);
            }
        }
        self.last_had_cue = chosen.is_some();
        chosen
    }

    /// One `resolved` cue when an issued cue's component stayed within `inner` for 3 reps.
    fn resolution(
        &mut self,
        cues: &[CueDef],
        evidence: &BTreeMap<String, Option<CueEvidence>>,
    ) -> Option<CueInstance> {
        for cue in cues {
            let CueWhen::Component { component, .. } = &cue.when else {
                continue;
            };
            if !self.issued.contains_key(&cue.id) || self.reps.len() < 3 {
                continue;
            }
            let within = self
                .reps
                .iter()
                .rev()
                .take(3)
                .all(|r| r.within_inner.get(component) == Some(&true));
            if within {
                self.issued.remove(&cue.id);
                let e = evidence.get(&cue.id).cloned().flatten();
                return Some(instance(
                    cue,
                    CueChannel::AfterRep,
                    e.into_iter().collect(),
                    "",
                    true,
                ));
            }
        }
        None
    }

    /// Set summary cues: at most two corrective and one positive (07 §8.2).
    pub fn after_set(&self, cues: &[CueDef], model: Option<&ScoringModel>) -> Vec<CueInstance> {
        let scored = self.reps.len();
        if scored == 0 {
            return vec![];
        }
        let mut corrective: Vec<&CueDef> = cues
            .iter()
            .filter(|c| !self.disabled(c))
            .filter(|c| {
                let held = self
                    .reps
                    .iter()
                    .filter(|r| {
                        r.cues.get(&c.id).is_some_and(|h| {
                            h.held && h.grade <= c.min_grade && h.grade <= Grade::B
                        })
                    })
                    .count();
                held as f64 >= 0.4 * scored as f64
            })
            .collect();
        corrective.sort_by(|a, b| {
            a.category
                .cmp(&b.category)
                .then(a.priority.cmp(&b.priority))
        });
        let mut out: Vec<CueInstance> = corrective
            .into_iter()
            .take(2)
            .map(|c| instance(c, CueChannel::AfterSet, vec![], "", false))
            .collect();
        if let Some(model) = model {
            let now = self.set_means();
            let best = now
                .iter()
                .filter_map(|(k, v)| self.previous_set.get(k).map(|p| (k, v - p)))
                .filter(|(_, d)| *d > 0.0)
                .max_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((id, d)) = best
                && let Some(def) = component_def(model, id)
            {
                out.push(CueInstance {
                    id: format!("improved.{id}"),
                    channel: CueChannel::AfterSet,
                    category: CueCategory::Technique,
                    basis: bansayo_contracts::CueBasis::History,
                    observed: TextRef {
                        key: "cue.positive.improved".into(),
                        params: [(
                            "change_percent".to_owned(),
                            TextParam {
                                value: d,
                                format: TextFormat::Percent,
                            },
                        )]
                        .into(),
                    },
                    suggestion: TextRef {
                        key: format!("component.{}", def.id),
                        params: BTreeMap::new(),
                    },
                    drill: None,
                    evidence: vec![],
                    resolved: false,
                });
            }
        }
        out
    }
}

#[cfg(test)]
mod tests;
