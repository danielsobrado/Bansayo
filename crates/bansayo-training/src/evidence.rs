//! Evidence combination by the weakest-input rule (04 §3) and reason construction.
use bansayo_contracts::{Evidence, MethodId, Provenance, Reason, SigmaBasis};

/// Fidelity tier of a method, higher is better (04 §3.1).
pub fn fidelity(m: MethodId) -> u8 {
    use MethodId::*;
    match m {
        Triangulated | MarkerCluster | ImuOpticalFused | XrDevice | DepthCamera => 5,
        ImuMounted | ImuGyro | SkeletonFitAxis => 4,
        SkeletonFitPlane | Fk => 3,
        ImagePlane => 2,
        MonocularLift => 1,
        FloorContact | RestFill => 0,
    }
}

fn weakness(p: Provenance) -> u8 {
    match p {
        Provenance::Measured => 0,
        Provenance::Derived => 1,
        Provenance::Inferred => 2,
        Provenance::Unavailable => 3,
    }
}

/// Weakest provenance, lowest-fidelity method, minimum confidence, root-sum-square sigma (`null`
/// when any input sigma is `null`), newest observation time, union of sources.
pub fn combine<'a>(inputs: impl IntoIterator<Item = &'a Evidence>) -> Option<Evidence> {
    let mut out: Option<Evidence> = None;
    for e in inputs {
        let Some(o) = out.as_mut() else {
            out = Some(e.clone());
            continue;
        };
        if weakness(e.provenance) > weakness(o.provenance) {
            o.provenance = e.provenance;
        }
        if fidelity(e.method) < fidelity(o.method) {
            o.method = e.method;
        }
        o.confidence = match (o.confidence, e.confidence) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        o.sigma = o.sigma.zip(e.sigma).map(|(a, b)| (a * a + b * b).sqrt());
        o.sigma_basis = if o.sigma.is_some() {
            if o.sigma_basis == Some(SigmaBasis::Propagated)
                || e.sigma_basis == Some(SigmaBasis::Propagated)
            {
                Some(SigmaBasis::Propagated)
            } else {
                o.sigma_basis.or(e.sigma_basis)
            }
        } else {
            None
        };
        o.t_obs_us = o.t_obs_us.max(e.t_obs_us);
        for s in &e.sources {
            if !o.sources.contains(s) {
                o.sources.push(s.clone());
            }
        }
    }
    if let Some(o) = out.as_mut() {
        o.sources.sort();
    }
    out
}

/// Evidence of a value that does not exist.
pub fn unavailable() -> Evidence {
    Evidence {
        provenance: Provenance::Unavailable,
        method: MethodId::RestFill,
        confidence: None,
        sigma: None,
        sigma_basis: None,
        t_obs_us: None,
        sources: vec![],
    }
}

/// A value that may enter observables: observed or derived, never model fill (32 §2).
pub fn usable(e: &Evidence) -> bool {
    matches!(e.provenance, Provenance::Measured | Provenance::Derived)
        && !matches!(e.method, MethodId::RestFill)
}

pub fn reason(code: &str, subject: Option<&str>, message: &str) -> Reason {
    Reason {
        code: code.into(),
        subject: subject.map(Into::into),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bansayo_contracts::DeviceId;

    fn e(p: Provenance, m: MethodId, c: f32, s: Option<f32>, t: u64, src: &str) -> Evidence {
        Evidence {
            provenance: p,
            method: m,
            confidence: Some(c),
            sigma: s,
            sigma_basis: s.map(|_| SigmaBasis::Propagated),
            t_obs_us: Some(t),
            sources: vec![DeviceId::new(src).expect("id")],
        }
    }

    #[test]
    fn weakest_input_rule() {
        let a = e(
            Provenance::Measured,
            MethodId::Triangulated,
            0.9,
            Some(0.03),
            10,
            "A",
        );
        let b = e(Provenance::Derived, MethodId::Fk, 0.6, Some(0.04), 20, "B");
        let c = combine([&a, &b]).expect("combined");
        assert_eq!(c.provenance, Provenance::Derived);
        assert_eq!(c.method, MethodId::Fk);
        assert_eq!(c.confidence, Some(0.6));
        assert!((c.sigma.expect("sigma") - 0.05).abs() < 1e-6);
        assert_eq!(c.t_obs_us, Some(20));
        assert_eq!(c.sources.len(), 2);
        let n = e(
            Provenance::Measured,
            MethodId::Triangulated,
            1.0,
            None,
            5,
            "A",
        );
        let c = combine([&a, &n]).expect("combined");
        assert_eq!(c.sigma, None);
        assert_eq!(c.sigma_basis, None);
        assert!(combine([]).is_none());
    }
}
