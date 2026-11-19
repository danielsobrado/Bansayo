//! Content store (martial-arts 13 §2, §11–12): scan, parse, resolve, validate, compile-check,
//! index and snapshot. Rescans happen at start (and after engine writes, a later phase).
use bansayo_contracts::{
    ContentIndex, ContentIndexEntry, ContentNames, ContentProblem, ContentSource, Side,
    TimedWorkoutDef,
};
use bansayo_training::content::compile::compile_technique;
use bansayo_training::content::types::{DisciplineDef, ScoringModel, TechniqueDef};
use bansayo_training::content::validate::{
    Library, METRICS_SCHEMA, apply_extends, problem, schema_problems, valid_id,
    validate_discipline, validate_metric, validate_scoring,
};
use bansayo_training::hash::sha256;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Kinds served by `GET /content/{kind}/{id}` that this phase loads.
pub const KINDS: &[&str] = &[
    "disciplines",
    "techniques",
    "metrics",
    "scoring",
    "workouts",
];

#[derive(Debug, Clone)]
struct Loaded {
    kind: &'static str,
    id: String,
    body: Value,
    source: ContentSource,
    file: String,
    problems: Vec<ContentProblem>,
}

/// A definition as stored with sessions: canonical JSON and its SHA-256.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    pub hash: String,
    pub kind: String,
    pub id: String,
    pub version: u32,
    pub body_json: String,
}

#[derive(Debug, Clone, Default)]
pub struct ContentStore {
    pub rev: u64,
    pub library: Library,
    pub index: Vec<ContentIndexEntry>,
    pub i18n: BTreeMap<String, BTreeMap<String, String>>,
    bodies: BTreeMap<(String, String), Value>,
}

/// JSON with lexically sorted object keys: the canonical form hashed for snapshots.
pub fn canonical(v: &Value) -> String {
    fn sorted(v: &Value) -> Value {
        match v {
            Value::Object(o) => Value::Object(
                o.iter()
                    .map(|(k, v)| (k.clone(), sorted(v)))
                    .collect::<BTreeMap<_, _>>()
                    .into_iter()
                    .collect(),
            ),
            Value::Array(a) => Value::Array(a.iter().map(sorted).collect()),
            _ => v.clone(),
        }
    }
    sorted(v).to_string()
}

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            files(&p, out);
        } else if p.extension().is_some_and(|e| e == "yaml" || e == "yml") {
            out.push(p);
        }
    }
}

fn parse_yaml(path: &Path) -> Result<Value, ContentProblem> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| problem("", "unreadable", format!("cannot read the file: {e}")))?;
    serde_saphyr::from_str::<Value>(&text).map_err(|e| problem("", "parse_error", e.to_string()))
}

fn typed<T: serde::de::DeserializeOwned>(v: &Value) -> Result<T, ContentProblem> {
    serde_json::from_value(v.clone()).map_err(|e| {
        let msg = e.to_string();
        let path = msg
            .split('`')
            .nth(1)
            .filter(|_| msg.starts_with("unknown field") || msg.starts_with("missing field"))
            .unwrap_or("")
            .to_owned();
        let code = if msg.starts_with("unknown field") {
            "unknown_field"
        } else if msg.starts_with("missing field") {
            "missing_field"
        } else {
            "invalid_value"
        };
        problem(path, code, msg)
    })
}

impl ContentStore {
    /// Scans `dirs` (shipped) and `user_dir` (user content).
    pub fn load(dirs: &[PathBuf], user_dir: &Path, rev: u64) -> Self {
        let mut loaded: Vec<Loaded> = vec![];
        let mut i18n: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        let roots: Vec<(PathBuf, ContentSource)> = dirs
            .iter()
            .map(|d| (d.clone(), ContentSource::Shipped))
            .chain([(user_dir.to_owned(), ContentSource::User)])
            .collect();
        for (root, source) in &roots {
            for kind in KINDS.iter().copied().chain(["i18n"]) {
                let mut list = vec![];
                files(&root.join(kind), &mut list);
                for path in list {
                    let file = path
                        .strip_prefix(root)
                        .unwrap_or(&path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    let body = match parse_yaml(&path) {
                        Ok(b) => b,
                        Err(p) => {
                            loaded.push(Loaded {
                                kind,
                                id: file.clone(),
                                body: Value::Null,
                                source: *source,
                                file,
                                problems: vec![p],
                            });
                            continue;
                        }
                    };
                    if kind == "i18n" {
                        let lang = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        if let Value::Object(o) = body {
                            let map = i18n.entry(lang).or_default();
                            for (k, v) in o {
                                if let Some(s) = v.as_str() {
                                    map.insert(k, s.to_owned());
                                }
                            }
                        }
                        continue;
                    }
                    if kind == "metrics" {
                        loaded.extend(Self::split_pack(body, *source, &file));
                        continue;
                    }
                    let id = body
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or(&file)
                        .to_owned();
                    loaded.push(Loaded {
                        kind,
                        id,
                        body,
                        source: *source,
                        file,
                        problems: vec![],
                    });
                }
            }
        }
        Self::resolve(loaded, i18n, rev)
    }

    fn split_pack(body: Value, source: ContentSource, file: &str) -> Vec<Loaded> {
        let schema = body.get("schema").and_then(Value::as_str).unwrap_or("");
        let p = schema_problems(schema, METRICS_SCHEMA);
        let list = body
            .get("metrics")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        list.into_iter()
            .enumerate()
            .map(|(i, m)| Loaded {
                kind: "metrics",
                id: m
                    .get("id")
                    .and_then(Value::as_str)
                    .map_or_else(|| format!("{file}#{i}"), String::from),
                body: m,
                source,
                file: format!("{file}#metrics[{i}]"),
                problems: p.clone(),
            })
            .collect()
    }

    fn resolve(
        mut loaded: Vec<Loaded>,
        i18n: BTreeMap<String, BTreeMap<String, String>>,
        rev: u64,
    ) -> Self {
        // An id may exist in one place only (13 §2).
        let mut seen: BTreeMap<(String, String), usize> = BTreeMap::new();
        for i in 0..loaded.len() {
            let key = (loaded[i].kind.to_owned(), loaded[i].id.clone());
            if let Some(first) = seen.get(&key) {
                let msg = format!(
                    "\"{}\" is also defined in {}",
                    loaded[i].id, loaded[*first].file
                );
                loaded[i].problems.push(problem("id", "duplicate_id", msg));
            } else {
                seen.insert(key, i);
            }
        }
        // Single-level extends (05 §2).
        let bodies: BTreeMap<String, Value> = loaded
            .iter()
            .filter(|l| l.kind == "techniques")
            .map(|l| (l.id.clone(), l.body.clone()))
            .collect();
        for l in loaded.iter_mut().filter(|l| l.kind == "techniques") {
            let Some(base_ref) = l
                .body
                .get("extends")
                .and_then(Value::as_str)
                .map(String::from)
            else {
                continue;
            };
            let base_id = base_ref.split('@').next().unwrap_or_default();
            match bodies.get(base_id) {
                Some(base) if base.get("extends").is_none() => {
                    let same = ["kind", "family"]
                        .iter()
                        .all(|k| base.get(k) == l.body.get(k) || l.body.get(k).is_none());
                    if same {
                        l.body = apply_extends(base, &l.body);
                    } else {
                        l.problems.push(problem(
                            "extends",
                            "extends_mismatch",
                            "the base must have the same kind and family",
                        ));
                    }
                }
                Some(_) => l.problems.push(problem(
                    "extends",
                    "extends_chain",
                    "a definition that extends another cannot itself be extended",
                )),
                None => l.problems.push(problem(
                    "extends",
                    "unresolved",
                    format!("unknown base \"{base_ref}\""),
                )),
            }
        }
        let mut lib = Library::default();
        for l in loaded.iter_mut() {
            if !l.problems.is_empty() {
                continue;
            }
            let result = match l.kind {
                "disciplines" => typed::<DisciplineDef>(&l.body).map(|d| {
                    l.problems.extend(validate_discipline(&d));
                    lib.disciplines.insert(d.id.clone(), d);
                }),
                "techniques" => typed::<TechniqueDef>(&l.body).map(|t| {
                    lib.techniques.insert(t.id.clone(), t);
                }),
                "metrics" => {
                    typed::<bansayo_training::content::types::MetricDef>(&l.body).map(|m| {
                        lib.metrics.insert(m.id.clone(), m);
                    })
                }
                "scoring" => typed::<ScoringModel>(&l.body).map(|s| {
                    lib.scoring.insert(s.id.clone(), s);
                }),
                "workouts" => typed::<TimedWorkoutDef>(&l.body).map(|w| {
                    l.problems
                        .extend(bansayo_training::exercise::timed::validate(&w))
                }),
                _ => Ok(()),
            };
            if let Err(p) = result {
                l.problems.push(p);
            }
        }
        // Cross-file checks against everything that parsed.
        for l in loaded.iter_mut().filter(|l| l.problems.is_empty()) {
            match l.kind {
                "metrics" => {
                    let lead_rear = lib.disciplines.values().any(|d| d.uses_lead_rear);
                    l.problems
                        .extend(validate_metric(&lib.metrics[&l.id], "", lead_rear));
                }
                "scoring" => l
                    .problems
                    .extend(validate_scoring(&lib.scoring[&l.id], &lib)),
                "techniques" => {
                    let t = &lib.techniques[&l.id];
                    if l.source == ContentSource::User && !t.id.starts_with("user.") {
                        l.problems.push(problem(
                            "id",
                            "user_id_required",
                            "user content ids start with user.",
                        ));
                    }
                    for side in [Side::Left, Side::Right] {
                        if t.sides.contains(&side)
                            && let Err(p) = compile_technique(
                                t,
                                side,
                                Some(bansayo_contracts::Stance::Orthodox),
                                &lib,
                            )
                        {
                            l.problems.extend(p);
                            break;
                        }
                    }
                }
                _ => {}
            }
            if l.kind != "metrics" && !valid_id(&l.id) && l.kind != "disciplines" {
                l.problems.push(problem(
                    "id",
                    "invalid_id",
                    format!("invalid id \"{}\"", l.id),
                ));
            }
        }
        // Invalid definitions are reported and not offered (13 §11).
        for l in loaded.iter().filter(|l| !l.problems.is_empty()) {
            let removed = match l.kind {
                "disciplines" => lib.disciplines.remove(&l.id).is_some(),
                "techniques" => lib.techniques.remove(&l.id).is_some(),
                "metrics" => lib.metrics.remove(&l.id).is_some(),
                "scoring" => lib.scoring.remove(&l.id).is_some(),
                _ => false,
            };
            let _ = removed;
        }
        let index = loaded
            .iter()
            .map(|l| {
                let names: Option<ContentNames> = l
                    .body
                    .get("names")
                    .and_then(|n| serde_json::from_value(n.clone()).ok())
                    .or_else(|| {
                        l.body
                            .get("label")
                            .and_then(Value::as_str)
                            .map(|s| ContentNames {
                                display: s.into(),
                                native: None,
                                aliases: vec![],
                            })
                    });
                ContentIndexEntry {
                    kind: l.kind.into(),
                    id: l.id.clone(),
                    version: l.body.get("version").and_then(Value::as_u64).unwrap_or(0) as u32,
                    status: l
                        .body
                        .get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("draft")
                        .into(),
                    names: names.unwrap_or(ContentNames {
                        display: l.id.clone(),
                        native: None,
                        aliases: vec![],
                    }),
                    valid: l.problems.is_empty(),
                    problems: l.problems.clone(),
                    source: l.source,
                    discipline: l
                        .body
                        .get("discipline")
                        .and_then(Value::as_str)
                        .map(String::from),
                    family: l
                        .body
                        .get("family")
                        .and_then(Value::as_str)
                        .map(String::from),
                    file: Some(l.file.clone()),
                }
            })
            .collect();
        let bodies = loaded
            .into_iter()
            .filter(|l| l.problems.is_empty())
            .map(|l| ((l.kind.to_owned(), l.id), l.body))
            .collect();
        Self {
            rev,
            library: lib,
            index,
            i18n,
            bodies,
        }
    }

    pub fn index(&self) -> ContentIndex {
        ContentIndex {
            content_rev: self.rev,
            entries: self.index.clone(),
        }
    }

    /// The resolved definition as JSON (after `extends`).
    pub fn get(&self, kind: &str, id: &str) -> Option<&Value> {
        self.bodies.get(&(kind.to_owned(), id.to_owned()))
    }

    pub fn snapshot(&self, kind: &str, id: &str) -> Option<Snapshot> {
        let body = self.get(kind, id)?;
        let body_json = canonical(body);
        Some(Snapshot {
            hash: sha256(body_json.as_bytes()),
            kind: kind.into(),
            id: id.into(),
            version: body.get("version").and_then(Value::as_u64).unwrap_or(0) as u32,
            body_json,
        })
    }

    /// Every snapshot a session of `technique` depends on: the technique, its metrics, its
    /// scoring model (or the plan's) and its discipline.
    pub fn session_snapshots(&self, technique: &str, scoring: Option<&str>) -> Vec<Snapshot> {
        let Some(t) = self.library.techniques.get(technique) else {
            return vec![];
        };
        let mut out: Vec<Snapshot> = vec![];
        out.extend(self.snapshot("techniques", technique));
        out.extend(self.snapshot("disciplines", &t.discipline));
        for c in &t.confusable_with {
            out.extend(self.snapshot("techniques", c));
            if let Some(other) = self.library.techniques.get(c) {
                for m in &other.metrics {
                    out.extend(self.snapshot("metrics", &m.metric));
                }
            }
        }
        for m in &t.metrics {
            out.extend(self.snapshot("metrics", &m.metric));
        }
        if let Some(s) = scoring.or(t.scoring.as_deref()) {
            out.extend(self.snapshot("scoring", s));
        }
        out.sort_by(|a, b| a.hash.cmp(&b.hash));
        out.dedup_by(|a, b| a.hash == b.hash);
        out
    }

    /// Rebuilds a library from stored snapshots, for re-analysis with pinned content.
    pub fn library_from_snapshots(snapshots: &[Snapshot]) -> Result<Library, String> {
        let mut lib = Library::default();
        for s in snapshots {
            let v: Value = serde_json::from_str(&s.body_json).map_err(|e| e.to_string())?;
            match s.kind.as_str() {
                "disciplines" => {
                    let d: DisciplineDef = serde_json::from_value(v).map_err(|e| e.to_string())?;
                    lib.disciplines.insert(d.id.clone(), d);
                }
                "techniques" => {
                    let t: TechniqueDef = serde_json::from_value(v).map_err(|e| e.to_string())?;
                    lib.techniques.insert(t.id.clone(), t);
                }
                "metrics" => {
                    let m: bansayo_training::content::types::MetricDef =
                        serde_json::from_value(v).map_err(|e| e.to_string())?;
                    lib.metrics.insert(m.id.clone(), m);
                }
                "scoring" => {
                    let m: ScoringModel = serde_json::from_value(v).map_err(|e| e.to_string())?;
                    lib.scoring.insert(m.id.clone(), m);
                }
                _ => {}
            }
        }
        Ok(lib)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content")
    }

    #[test]
    fn shipped_content_loads_valid() {
        let s = ContentStore::load(&[shipped()], Path::new("/nonexistent"), 1);
        let bad: Vec<_> = s.index.iter().filter(|e| !e.valid).collect();
        assert!(bad.is_empty(), "{bad:#?}");
        assert!(s.library.techniques.contains_key("taekwondo.dollyeo_chagi"));
        assert!(s.library.techniques.contains_key("taekwondo.ap_chagi"));
        assert!(
            s.get("workouts", "general.workout.full_body_in_20")
                .is_some()
        );
        assert_eq!(s.library.metrics.len(), 11);
        assert!(s.i18n["en"].contains_key("cue.kick.pivot.observed"));
        let snaps = s.session_snapshots("taekwondo.dollyeo_chagi", None);
        assert!(snaps.iter().any(|x| x.kind == "scoring"));
        let again = s.snapshot("techniques", "taekwondo.dollyeo_chagi").unwrap();
        assert_eq!(again.hash, sha256(again.body_json.as_bytes()));
        let lib = ContentStore::library_from_snapshots(&snaps).unwrap();
        assert_eq!(
            lib.techniques["taekwondo.dollyeo_chagi"],
            s.library.techniques["taekwondo.dollyeo_chagi"]
        );
    }

    /// The illustrative example files are the loader's first validation corpus (O-19).
    #[test]
    fn example_files_report_their_differences_as_problems() {
        let dir = tempfile::tempdir().unwrap();
        let ex = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/architecture/martial-arts/examples");
        for (sub, file, name) in [
            (
                "techniques/taekwondo",
                "technique-roundhouse-kick.yaml",
                "dollyeo_chagi.yaml",
            ),
            ("metrics", "metrics-kick.yaml", "kick.yaml"),
            ("scoring", "scoring-kick-basic.yaml", "kick_basic.yaml"),
        ] {
            std::fs::create_dir_all(dir.path().join(sub)).unwrap();
            std::fs::copy(ex.join(file), dir.path().join(sub).join(name)).unwrap();
        }
        std::fs::create_dir_all(dir.path().join("disciplines")).unwrap();
        std::fs::copy(
            shipped().join("disciplines/taekwondo.yaml"),
            dir.path().join("disciplines/taekwondo.yaml"),
        )
        .unwrap();
        let s = ContentStore::load(&[dir.path().to_owned()], Path::new("/nonexistent"), 1);
        let rh = s
            .index
            .iter()
            .find(|e| e.id == "taekwondo.dollyeo_chagi")
            .unwrap();
        let paths: Vec<&str> = rh.problems.iter().map(|p| p.path.as_str()).collect();
        assert!(
            paths.contains(&"confusable_with[0]") && paths.contains(&"start_stance"),
            "{rh:#?}"
        );
        assert!(
            s.index
                .iter()
                .filter(|e| e.kind == "metrics")
                .all(|e| e.valid)
        );
        assert!(
            s.index
                .iter()
                .filter(|e| e.kind == "scoring")
                .all(|e| e.valid)
        );
    }

    #[test]
    fn duplicates_unknown_fields_and_extends() {
        let dir = tempfile::tempdir().unwrap();
        let user = tempfile::tempdir().unwrap();
        let t = std::fs::read_to_string(shipped().join("techniques/taekwondo/dollyeo_chagi.yaml"))
            .unwrap();
        std::fs::create_dir_all(user.path().join("techniques")).unwrap();
        std::fs::write(user.path().join("techniques/copy.yaml"), &t).unwrap();
        std::fs::write(
            user.path().join("techniques/mine.yaml"),
            "schema: bansayo.technique/v1\nid: user.ana.my_roundhouse\nversion: 1\nextends: taekwondo.dollyeo_chagi@1\nnames: { display: Mine }\ndiscipline: taekwondo\n",
        )
        .unwrap();
        std::fs::write(
            user.path().join("techniques/typo.yaml"),
            t.replace("max_gap_ms", "max_gap_msx")
                .replace("id: taekwondo.dollyeo_chagi", "id: user.ana.typo"),
        )
        .unwrap();
        let _ = dir;
        let s = ContentStore::load(&[shipped()], user.path(), 2);
        let get = |id: &str| s.index.iter().filter(|e| e.id == id).collect::<Vec<_>>();
        assert!(
            get("taekwondo.dollyeo_chagi")
                .iter()
                .any(|e| e.problems.iter().any(|p| p.code == "duplicate_id"))
        );
        let typo = get("user.ana.typo");
        assert_eq!(typo[0].problems[0].code, "unknown_field", "{typo:#?}");
        let mine = get("user.ana.my_roundhouse");
        assert!(
            mine[0].problems.iter().all(|p| p.code != "unknown_field"),
            "{mine:#?}"
        );
        assert_eq!(
            s.get("techniques", "user.ana.my_roundhouse")
                .and_then(|v| v.get("family"))
                .and_then(Value::as_str),
            Some("kick.roundhouse")
        );
    }
}
