//! Validation with path-qualified problems (13 §11). Cross-file checks run against a
//! [`Library`] of everything loaded.
use super::types::*;
use crate::observables::{self, Class};
use bansayo_contracts::ContentProblem;
use std::collections::{BTreeMap, BTreeSet};

pub const TECHNIQUE_SCHEMA: &str = "bansayo.technique/v1";
pub const METRICS_SCHEMA: &str = "bansayo.metrics/v1";
pub const SCORING_SCHEMA: &str = "bansayo.scoring/v1";
pub const DISCIPLINE_SCHEMA: &str = "bansayo.discipline/v1";
/// Closed family list (05 §4) of the families content may use now.
pub const FAMILIES: &[&str] = &[
    "kick.front",
    "kick.roundhouse",
    "kick.side",
    "kick.back",
    "kick.hook",
    "kick.crescent",
    "kick.axe",
    "knee.straight",
    "knee.round",
    "punch.straight",
    "punch.hook",
    "punch.uppercut",
    "strike.elbow",
    "strike.open_hand",
    "block",
    "stance",
    "transition",
    "mobility.hold",
    "mobility.dynamic",
    "strength",
];

/// Every definition loaded, keyed by id. Metric ids come from metric packs.
#[derive(Debug, Clone, Default)]
pub struct Library {
    pub disciplines: BTreeMap<String, DisciplineDef>,
    pub techniques: BTreeMap<String, TechniqueDef>,
    pub metrics: BTreeMap<String, MetricDef>,
    pub scoring: BTreeMap<String, ScoringModel>,
}

pub fn problem(path: impl Into<String>, code: &str, message: impl Into<String>) -> ContentProblem {
    ContentProblem {
        path: path.into(),
        code: code.into(),
        message: message.into(),
    }
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

fn suggest<'a>(word: &str, options: impl Iterator<Item = &'a str>) -> String {
    options
        .map(|o| (distance(word, o), o))
        .filter(|(d, _)| *d <= 3)
        .min()
        .map(|(_, o)| format!(", did you mean \"{o}\"?"))
        .unwrap_or_default()
}

/// `^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*){1,4}$`
pub fn valid_id(id: &str) -> bool {
    let parts: Vec<&str> = id.split('.').collect();
    (2..=5).contains(&parts.len())
        && parts.iter().all(|p| {
            let b = p.as_bytes();
            !b.is_empty()
                && b[0].is_ascii_lowercase()
                && b.iter()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_')
        })
}

fn local_id(id: &str) -> bool {
    let b = id.as_bytes();
    !b.is_empty()
        && b[0].is_ascii_lowercase()
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_')
}

pub fn schema_problems(schema: &str, expected: &str) -> Vec<ContentProblem> {
    if schema == expected {
        return vec![];
    }
    let kind = expected.rsplit_once('/').map_or(expected, |(k, _)| k);
    if schema.starts_with(&format!("{kind}/")) {
        vec![problem(
            "schema",
            "unsupported_major",
            format!("schema \"{schema}\" is not supported; this engine reads {expected}"),
        )]
    } else {
        vec![problem(
            "schema",
            "wrong_schema",
            format!("expected schema {expected}, found \"{schema}\""),
        )]
    }
}

fn header(
    id: &str,
    version: u32,
    status: Status,
    reviewed: (&Option<String>, &Option<String>),
    out: &mut Vec<ContentProblem>,
) {
    if !valid_id(id) {
        out.push(problem(
            "id",
            "invalid_id",
            format!("\"{id}\" must match ^[a-z][a-z0-9_]*(\\.[a-z][a-z0-9_]*){{1,4}}$"),
        ));
    }
    if version < 1 {
        out.push(problem(
            "version",
            "invalid_version",
            "version must be at least 1",
        ));
    }
    if status == Status::Reviewed && (reviewed.0.is_none() || reviewed.1.is_none()) {
        out.push(problem(
            "status",
            "review_incomplete",
            "a reviewed definition needs reviewed_by and reviewed_on",
        ));
    }
}

/// How an observable reference resolves before a side is chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Active,
    Support,
    Lead,
    Rear,
    Unsided,
}

/// Splits `knee_height.active` into a catalogue entry and a role.
pub fn parse_role(obs: &str) -> Option<(&'static observables::Entry, Role)> {
    let (name, role) = match obs.rsplit_once('.') {
        Some((n, "active")) => (n, Role::Active),
        Some((n, "support")) => (n, Role::Support),
        Some((n, "lead")) => (n, Role::Lead),
        Some((n, "rear")) => (n, Role::Rear),
        _ => (obs, Role::Unsided),
    };
    let entry = observables::ENTRIES.iter().find(|e| e.name == name)?;
    (entry.sided == (role != Role::Unsided)).then_some((entry, role))
}

/// Whether a content unit compares the observable's value (`false`) or its norm (`true`),
/// or `None` when incompatible.
pub fn unit_mode(entry: &observables::Entry, unit: Unit) -> Option<bool> {
    use crate::content::types::NormBy::*;
    let norm = entry.norm;
    match (entry.class, entry.unit, unit) {
        (Class::Angle, _, Unit::Rad) => Some(false),
        (Class::Rate, "rad_s", Unit::RadS) => Some(false),
        (Class::Position, _, Unit::M) => Some(false),
        (Class::Rate, "m_s", Unit::MS) => Some(false),
        (Class::Position, _, Unit::LegLength) if norm == Some(LegLength) => Some(true),
        (Class::Position, _, Unit::ArmLength) if norm == Some(ArmLength) => Some(true),
        (Class::Position, _, Unit::FootLength) if norm == Some(FootLength) => Some(true),
        (Class::Position, _, Unit::ShoulderWidth) if norm == Some(ShoulderWidth) => Some(true),
        (Class::Rate, "m_s", Unit::LegLengthS) if norm == Some(LegLength) => Some(true),
        (Class::Rate, "m_s", Unit::ArmLengthS) if norm == Some(ArmLength) => Some(true),
        _ => None,
    }
}

struct ObsCheck<'a> {
    uses_lead_rear: bool,
    out: &'a mut Vec<ContentProblem>,
}

impl ObsCheck<'_> {
    fn obs(&mut self, path: &str, obs: &str, unit: Option<Unit>) {
        let Some((entry, role)) = parse_role(obs) else {
            let names = observables::ids().iter().map(|(i, _)| i.as_str());
            let hint = suggest(
                &obs.replace(".active", ".left")
                    .replace(".support", ".right"),
                names,
            )
            .replace(".left", ".active")
            .replace(".right", ".support");
            self.out.push(problem(
                format!("{path}.obs"),
                "unknown_observable",
                format!("unknown observable \"{obs}\"{hint}"),
            ));
            return;
        };
        if matches!(role, Role::Lead | Role::Rear) && !self.uses_lead_rear {
            self.out.push(problem(
                format!("{path}.obs"),
                "invalid_role",
                "lead/rear roles need a discipline that uses_lead_rear",
            ));
        }
        if let Some(u) = unit
            && unit_mode(entry, u).is_none()
        {
            self.out.push(problem(
                format!("{path}.unit"),
                "incompatible_unit",
                format!(
                    "unit {u:?} cannot be used with {obs} (native {})",
                    entry.unit
                ),
            ));
        }
    }

    fn atom(&mut self, path: &str, a: &Atom) {
        self.obs(path, &a.obs, Some(a.unit));
        match (&a.op, &a.value) {
            (Op::Between, AtomValue::Range([lo, hi])) if lo <= hi => {}
            (Op::Between, _) => self.out.push(problem(
                format!("{path}.value"),
                "invalid_range",
                "between needs two ordered values [lo, hi]",
            )),
            (_, AtomValue::Range(_)) => self.out.push(problem(
                format!("{path}.value"),
                "invalid_value",
                "only between takes a range",
            )),
            _ => {}
        }
    }

    fn condition(&mut self, path: &str, c: &Condition, depth: u32) {
        match c {
            Condition::Atom(a) => self.atom(path, a),
            Condition::All { all: list } | Condition::Any { any: list } => {
                let key = if matches!(c, Condition::All { .. }) {
                    "all"
                } else {
                    "any"
                };
                if depth >= 2 {
                    self.out.push(problem(
                        path,
                        "nesting_too_deep",
                        "all/any nest at most one level",
                    ));
                    return;
                }
                if list.is_empty() {
                    self.out
                        .push(problem(path, "empty_condition", format!("{key} is empty")));
                }
                for (i, c) in list.iter().enumerate() {
                    self.condition(&format!("{path}.{key}[{i}]"), c, depth + 1);
                }
            }
        }
    }
}

fn anchor_phase(anchor: &str) -> Option<&str> {
    anchor
        .strip_prefix("phase_start:")
        .or_else(|| anchor.strip_prefix("phase_end:"))
}

fn valid_anchor(anchor: &str) -> bool {
    matches!(anchor, "rep_start" | "rep_end" | "strike") || anchor_phase(anchor).is_some()
}

pub fn validate_technique(t: &TechniqueDef, lib: &Library) -> Vec<ContentProblem> {
    let mut out = schema_problems(&t.schema, TECHNIQUE_SCHEMA);
    header(
        &t.id,
        t.version,
        t.status,
        (&t.reviewed_by, &t.reviewed_on),
        &mut out,
    );
    let discipline = lib.disciplines.get(&t.discipline);
    if discipline.is_none() && t.discipline != "general" {
        out.push(problem(
            "discipline",
            "unknown_discipline",
            format!(
                "unknown discipline \"{}\"{}",
                t.discipline,
                suggest(&t.discipline, lib.disciplines.keys().map(String::as_str))
            ),
        ));
    }
    let first = t.id.split('.').next().unwrap_or_default();
    if first != t.discipline && first != "general" && first != "user" {
        out.push(problem(
            "id",
            "id_discipline_mismatch",
            "the first id segment must be the discipline, general or user",
        ));
    }
    if !FAMILIES.contains(&t.family.as_str()) {
        out.push(problem(
            "family",
            "unknown_family",
            format!(
                "unknown family \"{}\"{}",
                t.family,
                suggest(&t.family, FAMILIES.iter().copied())
            ),
        ));
    }
    if t.kind != TechniqueKind::Dynamic {
        out.push(problem(
            "kind",
            "unsupported_kind",
            "only dynamic techniques run in this engine version",
        ));
    }
    let mut check = ObsCheck {
        uses_lead_rear: discipline.is_some_and(|d| d.uses_lead_rear),
        out: &mut out,
    };
    match &t.segmentation {
        Some(s) => {
            check.obs("segmentation.driver", &s.driver, Some(s.unit));
            if s.end_below > s.start_above || s.max_ms < s.min_ms {
                check.out.push(problem(
                    "segmentation",
                    "invalid_limits",
                    "end_below must not exceed start_above and max_ms must not be below min_ms",
                ));
            }
        }
        None => check.out.push(problem(
            "segmentation",
            "missing_field",
            "a dynamic technique needs segmentation",
        )),
    }
    let mut ids = BTreeSet::new();
    for (i, p) in t.phases.iter().enumerate() {
        let path = format!("phases[{i}]");
        if !local_id(&p.id) || !ids.insert(p.id.as_str()) {
            check.out.push(problem(
                format!("{path}.id"),
                "invalid_phase_id",
                format!(
                    "phase id \"{}\" must match ^[a-z][a-z0-9_]*$ and be unique",
                    p.id
                ),
            ));
        }
        if p.max_ms < p.min_ms {
            check
                .out
                .push(problem(path.clone(), "invalid_limits", "max_ms < min_ms"));
        }
        check.condition(&format!("{path}.enter"), &p.enter, 0);
    }
    if !t.phases.iter().any(|p| !p.optional) {
        check.out.push(problem(
            "phases",
            "no_mandatory_phase",
            "at least one mandatory phase is required",
        ));
    }
    if let Some(s) = &t.strike {
        check.obs("strike", &s.obs, Some(s.unit));
        if !ids.contains(s.in_phase.as_str()) {
            check.out.push(problem(
                "strike.in_phase",
                "unknown_phase",
                format!("unknown phase \"{}\"", s.in_phase),
            ));
        }
        if !matches!(s.op, Op::LocalMax | Op::LocalMin) {
            check.out.push(problem(
                "strike.op",
                "invalid_op",
                "strike uses local_max or local_min",
            ));
        }
    }
    match &t.complete {
        Some(c) => check.condition("complete", c, 0),
        None => check.out.push(problem(
            "complete",
            "missing_field",
            "a dynamic technique needs complete",
        )),
    }
    for (i, o) in t.required_observables.iter().enumerate() {
        check.obs(&format!("required_observables[{i}]"), o, None);
    }
    for (i, o) in t.optional_observables.iter().enumerate() {
        check.obs(&format!("optional_observables[{i}]"), o, None);
    }
    for (i, m) in t.metrics.iter().enumerate() {
        match lib.metrics.get(&m.metric) {
            None => out.push(problem(
                format!("metrics[{i}].use"),
                "unresolved",
                format!(
                    "unknown metric \"{}\"{}",
                    m.metric,
                    suggest(&m.metric, lib.metrics.keys().map(String::as_str))
                ),
            )),
            Some(def) => {
                for anchor in [&def.at, &def.from, &def.to].into_iter().flatten() {
                    if let Some(p) = anchor_phase(anchor)
                        && !ids.contains(p)
                    {
                        out.push(problem(
                            format!("metrics[{i}].use"),
                            "unknown_phase",
                            format!("{} anchors on phase \"{p}\", absent here", m.metric),
                        ));
                    }
                }
                let window_phases: Vec<&str> = match &def.window {
                    Some(Window::Phase { phase }) => vec![phase],
                    Some(Window::Span {
                        from_phase,
                        to_phase,
                    }) => vec![from_phase, to_phase],
                    _ => vec![],
                };
                for p in window_phases {
                    if !ids.contains(p) {
                        out.push(problem(
                            format!("metrics[{i}].use"),
                            "unknown_phase",
                            format!("{} windows on phase \"{p}\", absent here", m.metric),
                        ));
                    }
                }
            }
        }
    }
    let model = t.scoring.as_ref().and_then(|s| lib.scoring.get(s));
    if let Some(s) = &t.scoring
        && model.is_none()
    {
        out.push(problem(
            "scoring",
            "unresolved",
            format!("unknown scoring model \"{s}\""),
        ));
    }
    for (i, c) in t.confusable_with.iter().enumerate() {
        if !lib.techniques.contains_key(c) {
            out.push(problem(
                format!("confusable_with[{i}]"),
                "unresolved",
                format!("unknown technique \"{c}\""),
            ));
        }
    }
    for (key, stance) in [
        ("start_stance", &t.start_stance),
        ("end_stance", &t.end_stance),
    ] {
        if let Some(s) = stance
            && !lib.techniques.contains_key(s)
        {
            out.push(problem(
                key,
                "unresolved",
                format!("unknown stance \"{s}\""),
            ));
        }
    }
    for (i, c) in t.cues.iter().enumerate() {
        let path = format!("cues[{i}].when");
        match &c.when {
            CueWhen::Component { component, .. } => {
                if model.is_none_or(|m| !m.components.iter().any(|x| &x.id == component)) {
                    out.push(problem(
                        format!("{path}.component"),
                        "unresolved",
                        format!("component \"{component}\" is not in the scoring model"),
                    ));
                }
            }
            CueWhen::Metric { metric, .. }
            | CueWhen::Trend {
                trend: Trend { metric, .. },
            } => {
                if !t.metrics.iter().any(|m| &m.metric == metric) {
                    out.push(problem(
                        format!("{path}.metric"),
                        "unresolved",
                        format!("metric \"{metric}\" is not used by this technique"),
                    ));
                }
            }
            CueWhen::Step { .. } => {}
        }
    }
    if t.safety.max_reps_per_set == 0 || t.safety.max_sets_per_session == 0 {
        out.push(problem(
            "safety",
            "invalid_limits",
            "safety limits must be at least 1",
        ));
    }
    if t.sides.is_empty() && t.kind == TechniqueKind::Dynamic {
        out.push(problem(
            "sides",
            "missing_field",
            "a sided technique needs sides",
        ));
    }
    if let Some(e) = &t.extends {
        out.push(problem(
            "extends",
            "unresolved_extends",
            format!("\"{e}\" must be resolved before validation"),
        ));
    }
    out
}

pub fn validate_metric(m: &MetricDef, path: &str, uses_lead_rear: bool) -> Vec<ContentProblem> {
    let mut out = vec![];
    if !valid_id(&m.id) {
        out.push(problem(
            format!("{path}.id"),
            "invalid_id",
            format!("invalid id \"{}\"", m.id),
        ));
    }
    let mut check = ObsCheck {
        uses_lead_rear,
        out: &mut out,
    };
    let needs_obs = matches!(
        m.kind,
        MetricKind::Extremum
            | MetricKind::ValueAt
            | MetricKind::Range
            | MetricKind::Mean
            | MetricKind::Median
            | MetricKind::Rms
            | MetricKind::PeakCount
            | MetricKind::SettleTime
    );
    match (&m.obs, needs_obs) {
        (Some(o), true) => check.obs(path, o, None),
        (None, true) => check.out.push(problem(
            format!("{path}.obs"),
            "missing_field",
            format!("kind {:?} needs obs", m.kind),
        )),
        _ => {}
    }
    for (i, a) in m.gates.suppress_when.iter().enumerate() {
        check.atom(&format!("{path}.gates.suppress_when[{i}]"), a);
    }
    let anchors = [("at", &m.at), ("from", &m.from), ("to", &m.to)];
    for (k, a) in anchors {
        if let Some(a) = a
            && !valid_anchor(a)
        {
            out.push(problem(
                format!("{path}.{k}"),
                "invalid_anchor",
                format!("\"{a}\" is not rep_start, rep_end, strike or phase_start/end:<id>"),
            ));
        }
    }
    let required: &[(&str, bool)] = match m.kind {
        MetricKind::Extremum => &[("agg", m.agg.is_some()), ("window", m.window.is_some())],
        MetricKind::ValueAt => &[("at", m.at.is_some())],
        MetricKind::Range => &[("window", m.window.is_some())],
        MetricKind::Duration => &[("from", m.from.is_some()), ("to", m.to.is_some())],
        _ => &[],
    };
    for (k, present) in required {
        if !present {
            out.push(problem(
                format!("{path}.{k}"),
                "missing_field",
                format!("kind {:?} needs {k}", m.kind),
            ));
        }
    }
    if m.gates.grade_a_methods.is_empty() {
        out.push(problem(
            format!("{path}.gates.grade_a_methods"),
            "missing_field",
            "at least one grade-A method is required",
        ));
    }
    out
}

pub fn validate_scoring(s: &ScoringModel, lib: &Library) -> Vec<ContentProblem> {
    let mut out = schema_problems(&s.schema, SCORING_SCHEMA);
    header(
        &s.id,
        s.version,
        s.status,
        (&s.reviewed_by, &s.reviewed_on),
        &mut out,
    );
    let mut ids = BTreeSet::new();
    for (i, c) in s.components.iter().enumerate() {
        let path = format!("components[{i}]");
        if !ids.insert(c.id.as_str()) {
            out.push(problem(
                format!("{path}.id"),
                "duplicate_id",
                "component ids are unique",
            ));
        }
        if c.weight < 0.0 || !c.weight.is_finite() {
            out.push(problem(
                format!("{path}.weight"),
                "invalid_weight",
                "weight must be ≥ 0",
            ));
        }
        if let Some(t) = &c.tolerance
            && !(t.inner < t.outer)
        {
            out.push(problem(
                format!("{path}.tolerance"),
                "invalid_tolerance",
                "tolerance.inner must be below outer",
            ));
        }
        if let Some(TargetSource::Definition { rationale, .. }) = &c.target
            && rationale.trim().is_empty()
        {
            out.push(problem(
                format!("{path}.target.rationale"),
                "missing_rationale",
                "a definition target needs a rationale",
            ));
        }
        if let ComponentSource::Metric { metric, .. } = &c.source {
            if !lib.metrics.contains_key(metric) {
                out.push(problem(
                    format!("{path}.source.metric"),
                    "unresolved",
                    format!("unknown metric \"{metric}\""),
                ));
            }
            if c.target.is_none() || c.tolerance.is_none() || c.direction.is_none() {
                out.push(problem(
                    path.clone(),
                    "missing_field",
                    "a metric component needs target, direction and tolerance",
                ));
            }
        }
    }
    if !s.components.iter().any(|c| c.weight > 0.0) {
        out.push(problem(
            "components",
            "no_weight",
            "at least one component needs weight > 0",
        ));
    }
    if !(0.0..=1.0).contains(&s.composite.min_coverage) || s.composite.id != "reference_match" {
        out.push(problem(
            "composite",
            "invalid_composite",
            "composite is reference_match with min_coverage in 0..1",
        ));
    }
    out
}

pub fn validate_discipline(d: &DisciplineDef) -> Vec<ContentProblem> {
    let mut out = schema_problems(&d.schema, DISCIPLINE_SCHEMA);
    if !local_id(&d.id) {
        out.push(problem(
            "id",
            "invalid_id",
            "a discipline id is one segment",
        ));
    }
    if d.version < 1 {
        out.push(problem(
            "version",
            "invalid_version",
            "version must be at least 1",
        ));
    }
    if let Some(s) = &d.default_style
        && !d.styles.iter().any(|x| &x.id == s)
    {
        out.push(problem(
            "default_style",
            "unresolved",
            format!("unknown style \"{s}\""),
        ));
    }
    out
}

/// Overlays a child definition on its base (05 §2): present keys replace, lists whole.
pub fn apply_extends(base: &serde_json::Value, child: &serde_json::Value) -> serde_json::Value {
    let mut out = base.clone();
    if let (Some(o), Some(c)) = (out.as_object_mut(), child.as_object()) {
        for (k, v) in c {
            if k != "extends" {
                o.insert(k.clone(), v.clone());
            }
        }
    }
    out
}
