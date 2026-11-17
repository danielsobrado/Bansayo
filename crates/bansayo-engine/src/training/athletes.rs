//! Athlete profiles (martial-arts 12 §1): `<data_dir>/athletes/<id>/profile.json`, written
//! atomically; the active athlete in `<data_dir>/state/active_athlete`.
use bansayo_contracts::*;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Athletes {
    dir: PathBuf,
    state: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum AthleteError {
    #[error("{0}")]
    Invalid(String),
    #[error("an athlete with this id already exists")]
    Exists,
    #[error("no athlete with this id")]
    NotFound,
    #[error("athlete storage failed")]
    Io(#[from] std::io::Error),
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Tolerance scale by experience (13 §4 `coaching.tolerance_scale` defaults).
pub fn tolerance_for(e: Experience) -> f64 {
    match e {
        Experience::Beginner => 1.5,
        Experience::Intermediate => 1.0,
        Experience::Advanced => 0.75,
    }
}

pub fn validate(p: &AthleteProfile) -> Result<(), AthleteError> {
    let name = p.display_name.trim();
    if name.is_empty() || p.display_name.chars().count() > 64 {
        return Err(AthleteError::Invalid(
            "Display name must be 1 to 64 characters.".into(),
        ));
    }
    if !(p.height_m.is_finite() && (0.5..=2.5).contains(&p.height_m)) {
        return Err(AthleteError::Invalid(
            "Height must be between 0.5 and 2.5 metres.".into(),
        ));
    }
    if !(p.coaching.tolerance_scale.is_finite()
        && p.coaching.tolerance_scale > 0.0
        && p.coaching.tolerance_scale <= 10.0)
    {
        return Err(AthleteError::Invalid(
            "Tolerance scale must be greater than 0 and at most 10.".into(),
        ));
    }
    Ok(())
}

impl Athletes {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.join("athletes"),
            state: data_dir.join("state/active_athlete"),
        }
    }

    fn file(&self, id: &AthleteId) -> PathBuf {
        self.dir.join(id.as_str()).join("profile.json")
    }

    pub fn list(&self) -> Vec<AthleteProfile> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return vec![];
        };
        let mut out: Vec<AthleteProfile> = entries
            .flatten()
            .filter_map(|e| AthleteId::new(e.file_name().to_string_lossy().to_string()).ok())
            .filter_map(|id| self.get(&id).ok())
            .collect();
        out.sort_by(|a, b| a.id.cmp(&b.id));
        out
    }

    pub fn get(&self, id: &AthleteId) -> Result<AthleteProfile, AthleteError> {
        let text = std::fs::read_to_string(self.file(id)).map_err(|_| AthleteError::NotFound)?;
        let p: AthleteProfile = serde_json::from_str(&text).map_err(|_| AthleteError::NotFound)?;
        if &p.id != id {
            return Err(AthleteError::NotFound);
        }
        Ok(p)
    }

    pub fn save(&self, p: &AthleteProfile) -> Result<(), AthleteError> {
        validate(p)?;
        let bytes =
            serde_json::to_vec_pretty(p).map_err(|e| AthleteError::Invalid(e.to_string()))?;
        write_atomic(&self.file(&p.id), &bytes)?;
        Ok(())
    }

    pub fn create(
        &self,
        r: CreateAthleteRequest,
        now_utc: &str,
        record: RecordingFlags,
    ) -> Result<AthleteProfile, AthleteError> {
        if self.file(&r.id).exists() {
            return Err(AthleteError::Exists);
        }
        let disciplines = r.disciplines.unwrap_or_default();
        let experience = disciplines
            .first()
            .map_or(Experience::Beginner, |d| d.experience);
        let p = AthleteProfile {
            schema_version: ATHLETE_PROFILE_SCHEMA_VERSION,
            id: r.id,
            display_name: r.display_name,
            created_utc: now_utc.into(),
            height_m: r.height_m,
            dominant_side: r.dominant_side,
            default_stance: r.default_stance,
            disciplines,
            preferred_discipline: r.preferred_discipline,
            goals: vec![],
            coaching: AthleteCoaching {
                level: CoachingLevel::Standard,
                tolerance_scale: tolerance_for(experience),
            },
            enabled_techniques: vec![],
            tracking: AthleteTracking {
                profile_override: None,
                record_defaults: record,
            },
            consent: Consent::default(),
            retention: None,
            safety_ack: Default::default(),
        };
        self.save(&p)?;
        Ok(p)
    }

    /// Replaces a profile; `id`, `created_utc` and consent stay as stored (consent changes only
    /// through the consent action).
    pub fn update(
        &self,
        id: &AthleteId,
        mut p: AthleteProfile,
    ) -> Result<AthleteProfile, AthleteError> {
        let old = self.get(id)?;
        p.id = old.id;
        p.created_utc = old.created_utc;
        p.consent = old.consent;
        p.schema_version = ATHLETE_PROFILE_SCHEMA_VERSION;
        self.save(&p)?;
        Ok(p)
    }

    pub fn consent(
        &self,
        id: &AthleteId,
        r: &ConsentRequest,
        now_utc: &str,
    ) -> Result<AthleteProfile, AthleteError> {
        let mut p = self.get(id)?;
        let set = |v: Option<bool>, field: &mut Option<String>| match v {
            Some(true) => *field = Some(field.clone().unwrap_or_else(|| now_utc.into())),
            Some(false) => *field = None,
            None => {}
        };
        set(r.motion_data, &mut p.consent.motion_data_utc);
        set(r.video, &mut p.consent.video_utc);
        if p.consent.motion_data_utc.is_none() {
            p.consent.video_utc = None;
        }
        self.save(&p)?;
        Ok(p)
    }

    pub fn acknowledge(
        &self,
        id: &AthleteId,
        refs: &[ContentRef],
        now_utc: &str,
    ) -> Result<AthleteProfile, AthleteError> {
        let mut p = self.get(id)?;
        for r in refs {
            p.safety_ack
                .entry(r.clone())
                .or_insert_with(|| now_utc.into());
        }
        self.save(&p)?;
        Ok(p)
    }

    pub fn active(&self) -> Option<AthleteId> {
        let id = AthleteId::new(std::fs::read_to_string(&self.state).ok()?.trim()).ok()?;
        self.get(&id).ok().map(|p| p.id)
    }

    pub fn set_active(&self, id: Option<&AthleteId>) -> Result<(), AthleteError> {
        match id {
            Some(id) => {
                self.get(id)?;
                write_atomic(&self.state, id.as_str().as_bytes())?;
            }
            None => {
                if self.state.exists() {
                    std::fs::remove_file(&self.state)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(id: &str) -> CreateAthleteRequest {
        CreateAthleteRequest {
            id: AthleteId::new(id).unwrap(),
            display_name: "Ana".into(),
            height_m: 1.7,
            dominant_side: None,
            default_stance: Some(Stance::Orthodox),
            disciplines: None,
            preferred_discipline: Some("taekwondo".into()),
        }
    }

    #[test]
    fn profiles_consent_and_active_athlete() {
        let dir = tempfile::tempdir().unwrap();
        let a = Athletes::new(dir.path());
        let p = a
            .create(
                req("ana"),
                "2026-11-17T10:00:00Z",
                RecordingFlags::default(),
            )
            .unwrap();
        assert!(matches!(
            a.create(req("ana"), "x", RecordingFlags::default()),
            Err(AthleteError::Exists)
        ));
        assert_eq!(p.consent.motion_data_utc, None);
        assert_eq!(p.coaching.tolerance_scale, 1.5);
        let c = a
            .consent(
                &p.id,
                &ConsentRequest {
                    motion_data: Some(true),
                    video: Some(true),
                },
                "2026-11-17T10:01:00Z",
            )
            .unwrap();
        assert!(c.consent.motion_data_utc.is_some() && c.consent.video_utc.is_some());
        let c = a
            .consent(
                &p.id,
                &ConsentRequest {
                    motion_data: Some(false),
                    video: None,
                },
                "t",
            )
            .unwrap();
        assert!(
            c.consent.motion_data_utc.is_none() && c.consent.video_utc.is_none(),
            "video needs motion consent"
        );
        let mut edited = c.clone();
        edited.display_name = "Ana B".into();
        edited.created_utc = "tampered".into();
        edited.consent.motion_data_utc = Some("tampered".into());
        let u = a.update(&p.id, edited).unwrap();
        assert_eq!(u.created_utc, "2026-11-17T10:00:00Z");
        assert_eq!(u.consent.motion_data_utc, None);
        let mut bad = u.clone();
        bad.height_m = 3.0;
        assert!(a.update(&p.id, bad).is_err());
        assert_eq!(a.active(), None);
        a.set_active(Some(&p.id)).unwrap();
        assert_eq!(a.active(), Some(p.id.clone()));
        assert!(
            a.set_active(Some(&AthleteId::new("nobody").unwrap()))
                .is_err()
        );
        a.set_active(None).unwrap();
        assert_eq!(a.active(), None);
        assert_eq!(a.list().len(), 1);
    }
}
