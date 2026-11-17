-- Analytics store, martial-arts 12 §4: the tables roadmap Phase 1 writes.
CREATE TABLE athletes (
  id TEXT PRIMARY KEY, created_utc TEXT NOT NULL);

CREATE TABLE content_snapshots (
  hash TEXT PRIMARY KEY,
  kind TEXT NOT NULL, id TEXT NOT NULL, version INTEGER NOT NULL,
  body_json TEXT NOT NULL, first_used_utc TEXT NOT NULL);

CREATE TABLE tracking_configs (
  id TEXT PRIMARY KEY, profile TEXT NOT NULL, body_json TEXT NOT NULL, first_used_utc TEXT NOT NULL);

CREATE TABLE calibration_sets (
  id TEXT PRIMARY KEY,
  qa TEXT NOT NULL CHECK (qa IN ('pass','warn','fail','not_run')),
  body_json TEXT NOT NULL,
  first_used_utc TEXT NOT NULL);

CREATE TABLE training_sessions (
  id TEXT PRIMARY KEY,
  athlete_id TEXT NOT NULL REFERENCES athletes(id) ON DELETE CASCADE,
  started_utc TEXT NOT NULL, ended_utc TEXT,
  status TEXT NOT NULL CHECK (status IN ('active','completed','aborted','interrupted')),
  plan_kind TEXT NOT NULL, plan_ref TEXT, plan_json TEXT NOT NULL,
  origin TEXT NOT NULL CHECK (origin IN ('live','simulated','fixture')),
  engine_version TEXT NOT NULL, observables_rev INTEGER NOT NULL,
  capability_profile TEXT NOT NULL,
  tracking_config_id TEXT NOT NULL REFERENCES tracking_configs(id),
  calibration_set_id TEXT NOT NULL REFERENCES calibration_sets(id),
  active_ms INTEGER, rpe INTEGER CHECK (rpe BETWEEN 1 AND 10),
  has_inputs INTEGER NOT NULL DEFAULT 0, has_motion INTEGER NOT NULL DEFAULT 0, has_video INTEGER NOT NULL DEFAULT 0,
  analysis_rev INTEGER NOT NULL DEFAULT 1,
  reanalysis_of TEXT REFERENCES training_sessions(id) ON DELETE CASCADE,
  source_mask_json TEXT,
  summary_json TEXT);

CREATE TABLE session_content (
  session_id TEXT NOT NULL REFERENCES training_sessions(id) ON DELETE CASCADE,
  snapshot_hash TEXT NOT NULL REFERENCES content_snapshots(hash),
  PRIMARY KEY (session_id, snapshot_hash));

CREATE TABLE sets (
  session_id TEXT NOT NULL REFERENCES training_sessions(id) ON DELETE CASCADE,
  set_index INTEGER NOT NULL,
  block_id TEXT, item_id TEXT, exercise_kind TEXT NOT NULL, content_ref TEXT, side TEXT,
  target_json TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('completed','capped','skipped','abandoned')),
  scored INTEGER NOT NULL DEFAULT 1,
  started_t_us INTEGER NOT NULL, ended_t_us INTEGER,
  summary_json TEXT,
  PRIMARY KEY (session_id, set_index));

CREATE TABLE reps (
  session_id TEXT NOT NULL, rep_index INTEGER NOT NULL, set_index INTEGER NOT NULL,
  technique_id TEXT NOT NULL, technique_version INTEGER NOT NULL,
  side TEXT NOT NULL, stance TEXT,
  outcome TEXT NOT NULL CHECK (outcome IN ('completed','incomplete','rejected_quality','wrong_side','excluded')),
  step_id TEXT, step_result TEXT,
  t_start_us INTEGER NOT NULL, t_end_us INTEGER NOT NULL, strike_t_us INTEGER,
  recognition_confidence REAL NOT NULL,
  capability_profile TEXT NOT NULL,
  phases_json TEXT NOT NULL,
  PRIMARY KEY (session_id, rep_index),
  FOREIGN KEY (session_id, set_index) REFERENCES sets(session_id, set_index) ON DELETE CASCADE);

CREATE TABLE measurements (
  id INTEGER PRIMARY KEY,
  session_id TEXT NOT NULL REFERENCES training_sessions(id) ON DELETE CASCADE,
  scope TEXT NOT NULL CHECK (scope IN ('rep','hold','round','set','session','assessment')),
  rep_index INTEGER, set_index INTEGER,
  technique_id TEXT, side TEXT,
  metric_id TEXT NOT NULL, metric_version INTEGER NOT NULL,
  value REAL, norm REAL, unit TEXT NOT NULL,
  grade TEXT NOT NULL CHECK (grade IN ('A','B','C','X')),
  confidence REAL, sigma REAL, sigma_basis TEXT,
  method TEXT, provenance TEXT NOT NULL,
  comparability_class TEXT NOT NULL,
  reasons_json TEXT);
CREATE INDEX measurements_series
  ON measurements (metric_id, comparability_class, technique_id, side, session_id);

CREATE TABLE scores (
  session_id TEXT NOT NULL, rep_index INTEGER NOT NULL,
  model_id TEXT NOT NULL, model_version INTEGER NOT NULL,
  reference_id TEXT, reference_match REAL, coverage REAL NOT NULL,
  comparability_class TEXT NOT NULL, components_json TEXT NOT NULL,
  PRIMARY KEY (session_id, rep_index),
  FOREIGN KEY (session_id, rep_index) REFERENCES reps(session_id, rep_index) ON DELETE CASCADE);

CREATE TABLE events (
  session_id TEXT NOT NULL REFERENCES training_sessions(id) ON DELETE CASCADE,
  event_seq INTEGER NOT NULL, t_us INTEGER NOT NULL, type TEXT NOT NULL, body_json TEXT NOT NULL,
  PRIMARY KEY (session_id, event_seq));

CREATE TABLE baselines (
  athlete_id TEXT NOT NULL REFERENCES athletes(id) ON DELETE CASCADE,
  technique_id TEXT NOT NULL, side TEXT NOT NULL, metric_id TEXT NOT NULL, comparability_class TEXT NOT NULL,
  value REAL NOT NULL, spread REAL NOT NULL, sessions INTEGER NOT NULL,
  from_utc TEXT NOT NULL, to_utc TEXT NOT NULL, set_by TEXT NOT NULL CHECK (set_by IN ('auto','user')),
  PRIMARY KEY (athlete_id, technique_id, side, metric_id, comparability_class));

CREATE TABLE personal_records (
  athlete_id TEXT NOT NULL REFERENCES athletes(id) ON DELETE CASCADE,
  technique_id TEXT NOT NULL, side TEXT NOT NULL, metric_id TEXT NOT NULL, comparability_class TEXT NOT NULL,
  session_id TEXT NOT NULL, rep_index INTEGER, value REAL NOT NULL, set_utc TEXT NOT NULL,
  PRIMARY KEY (athlete_id, technique_id, side, metric_id, comparability_class));

-- I1: stored measurements and scores are never updated.
CREATE TRIGGER measurements_immutable BEFORE UPDATE ON measurements
BEGIN SELECT RAISE(ABORT, 'measurements are immutable'); END;
CREATE TRIGGER scores_immutable BEFORE UPDATE ON scores
BEGIN SELECT RAISE(ABORT, 'scores are immutable'); END;
