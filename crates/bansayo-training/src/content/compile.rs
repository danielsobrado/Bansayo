//! Compilation of validated definitions for one active side (05 §3): roles resolve to catalogue
//! indices, units to value-or-norm comparisons, signed observables to a mirror sign.
use super::types::*;
use super::validate::{Library, Role, parse_role, unit_mode, validate_technique};
use crate::observables;
use bansayo_contracts::{ContentProblem, Side, Stance};

#[derive(Debug, Clone, PartialEq)]
pub struct Obs {
    /// Catalogue index; `None` when the role cannot be resolved (always unknown).
    pub index: Option<usize>,
    /// Compare `norm` instead of `value`.
    pub norm: bool,
    /// −1 for a signed observable under a mirrored (right) active side.
    pub sign: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CAtom {
    pub obs: Obs,
    pub op: Op,
    pub lo: f64,
    pub hi: f64,
    pub reference: RefPoint,
    pub hold_us: u64,
    pub prominence: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CCondition {
    All(Vec<CCondition>),
    Any(Vec<CCondition>),
    Atom(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CPhase {
    pub id: String,
    pub optional: bool,
    pub enter: CCondition,
    pub min_us: u64,
    pub max_us: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CStrike {
    pub obs: Obs,
    pub max: bool,
    pub prominence: f64,
    pub in_phase: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CMetric {
    pub def: MetricDef,
    /// `id@version`.
    pub reference: String,
    pub obs: Option<Obs>,
    pub suppress_when: Vec<CAtom>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompiledTechnique {
    pub def: TechniqueDef,
    pub reference: String,
    pub side: Side,
    pub stance: Option<Stance>,
    pub driver: Obs,
    pub start_above: f64,
    pub end_below: f64,
    pub min_us: u64,
    pub max_us: u64,
    /// Every atom of every condition, addressed by `CCondition::Atom`.
    pub atoms: Vec<CAtom>,
    pub phases: Vec<CPhase>,
    pub strike: Option<CStrike>,
    pub complete: CCondition,
    pub required: Vec<Obs>,
    pub optional: Vec<Obs>,
    pub max_gap_us: u64,
    pub metrics: Vec<CMetric>,
    pub scoring: Option<ScoringModel>,
}

impl CompiledTechnique {
    pub fn content_ref(def: &TechniqueDef) -> String {
        format!("{}@{}", def.id, def.version)
    }
}

fn other(s: Side) -> Side {
    match s {
        Side::Left => Side::Right,
        Side::Right => Side::Left,
    }
}

fn side_name(s: Side) -> &'static str {
    match s {
        Side::Left => "left",
        Side::Right => "right",
    }
}

/// Resolves `name.role` for an active side and session stance.
pub fn resolve(
    obs: &str,
    unit: Option<Unit>,
    side: Side,
    stance: Option<Stance>,
    mirror: bool,
) -> Obs {
    let Some((entry, role)) = parse_role(obs) else {
        return Obs {
            index: None,
            norm: false,
            sign: 1.0,
        };
    };
    let concrete = match role {
        Role::Unsided => Some(None),
        Role::Active => Some(Some(side)),
        Role::Support => Some(Some(other(side))),
        Role::Lead | Role::Rear => stance.map(|s| {
            let lead = if s == Stance::Orthodox {
                Side::Left
            } else {
                Side::Right
            };
            Some(if role == Role::Lead {
                lead
            } else {
                other(lead)
            })
        }),
    };
    let index = concrete.and_then(|s| {
        observables::index(&match s {
            Some(s) => format!("{}.{}", entry.name, side_name(s)),
            None => entry.name.to_string(),
        })
    });
    Obs {
        index,
        norm: unit.and_then(|u| unit_mode(entry, u)).unwrap_or(false),
        sign: if entry.signed && mirror && side == Side::Right {
            -1.0
        } else {
            1.0
        },
    }
}

struct Ctx<'a> {
    side: Side,
    stance: Option<Stance>,
    mirror: bool,
    atoms: &'a mut Vec<CAtom>,
}

impl Ctx<'_> {
    fn atom(&mut self, a: &Atom) -> CAtom {
        let (lo, hi) = match a.value {
            AtomValue::One(v) => (v, v),
            AtomValue::Range([lo, hi]) => (lo, hi),
        };
        CAtom {
            obs: resolve(&a.obs, Some(a.unit), self.side, self.stance, self.mirror),
            op: a.op,
            lo,
            hi,
            reference: a.reference.unwrap_or_default(),
            hold_us: u64::from(a.hold_ms.unwrap_or(0)) * 1000,
            prominence: a.prominence.unwrap_or(0.0),
        }
    }

    fn condition(&mut self, c: &Condition) -> CCondition {
        match c {
            Condition::Atom(a) => {
                let atom = self.atom(a);
                self.atoms.push(atom);
                CCondition::Atom(self.atoms.len() - 1)
            }
            Condition::All { all } => {
                CCondition::All(all.iter().map(|c| self.condition(c)).collect())
            }
            Condition::Any { any } => {
                CCondition::Any(any.iter().map(|c| self.condition(c)).collect())
            }
        }
    }
}

pub fn compile_metric(m: &MetricDef, side: Side, stance: Option<Stance>, mirror: bool) -> CMetric {
    let mut atoms = vec![];
    let mut ctx = Ctx {
        side,
        stance,
        mirror,
        atoms: &mut atoms,
    };
    let suppress_when = m.gates.suppress_when.iter().map(|a| ctx.atom(a)).collect();
    CMetric {
        reference: format!("{}@{}", m.id, m.version),
        obs: m
            .obs
            .as_ref()
            .map(|o| resolve(o, None, side, stance, mirror)),
        def: m.clone(),
        suppress_when,
    }
}

/// Validates and compiles a technique for one active side.
pub fn compile_technique(
    def: &TechniqueDef,
    side: Side,
    stance: Option<Stance>,
    lib: &Library,
) -> Result<CompiledTechnique, Vec<ContentProblem>> {
    let problems = validate_technique(def, lib);
    if !problems.is_empty() {
        return Err(problems);
    }
    if !def.sides.contains(&side) {
        return Err(vec![super::validate::problem(
            "sides",
            "side_not_allowed",
            format!(
                "{} may not be performed on the {} side",
                def.id,
                side_name(side)
            ),
        )]);
    }
    let mirror = def.mirror_signed;
    let mut atoms = vec![];
    let mut ctx = Ctx {
        side,
        stance,
        mirror,
        atoms: &mut atoms,
    };
    let seg = def.segmentation.clone().expect("validated");
    let phases: Vec<CPhase> = def
        .phases
        .iter()
        .map(|p| CPhase {
            id: p.id.clone(),
            optional: p.optional,
            enter: ctx.condition(&p.enter),
            min_us: u64::from(p.min_ms) * 1000,
            max_us: u64::from(p.max_ms) * 1000,
        })
        .collect();
    let complete = ctx.condition(def.complete.as_ref().expect("validated"));
    let strike = def.strike.as_ref().map(|s| CStrike {
        obs: resolve(&s.obs, Some(s.unit), side, stance, mirror),
        max: s.op == Op::LocalMax,
        prominence: s.prominence.unwrap_or(0.0),
        in_phase: phases.iter().position(|p| p.id == s.in_phase).unwrap_or(0),
    });
    let obs_list = |list: &[String]| {
        list.iter()
            .map(|o| resolve(o, None, side, stance, mirror))
            .collect::<Vec<_>>()
    };
    Ok(CompiledTechnique {
        reference: CompiledTechnique::content_ref(def),
        side,
        stance,
        driver: resolve(&seg.driver, Some(seg.unit), side, stance, mirror),
        start_above: seg.start_above,
        end_below: seg.end_below,
        min_us: u64::from(seg.min_ms) * 1000,
        max_us: u64::from(seg.max_ms) * 1000,
        atoms,
        phases,
        strike,
        complete,
        required: obs_list(&def.required_observables),
        optional: obs_list(&def.optional_observables),
        max_gap_us: u64::from(def.max_gap_ms.unwrap_or(80)) * 1000,
        metrics: def
            .metrics
            .iter()
            .filter_map(|u| lib.metrics.get(&u.metric))
            .map(|m| compile_metric(m, side, stance, mirror))
            .collect(),
        scoring: def
            .scoring
            .as_ref()
            .and_then(|s| lib.scoring.get(s))
            .cloned(),
        def: def.clone(),
    })
}
