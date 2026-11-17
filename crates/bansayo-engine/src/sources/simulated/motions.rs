//! Scripted motions of the simulated body. Each motion is periodic and smooth, and its key
//! angles are named constants so tests can assert the solved joint angles against them.

use std::f64::consts::{PI, TAU};

/// Static T-pose: every segment rotation is identity.
pub const REFERENCE_POSE_ARM_ELEVATION_DEG: f64 = 90.0;

pub const STANDING_SWAY_DEG: f64 = 1.0;
pub const STANDING_PERIOD_S: f64 = 5.0;

pub const SQUAT_PERIOD_S: f64 = 4.0;
pub const SQUAT_KNEE_MAX_DEG: f64 = 100.0;
/// Forward tilt of the pelvis at the bottom of the squat.
pub const SQUAT_PELVIS_TILT_MAX_DEG: f64 = 40.0;
/// Forward lean of the chest from vertical at the bottom of the squat.
pub const SQUAT_TRUNK_LEAN_MAX_DEG: f64 = 30.0;

pub const KNEE_FLEXION_PERIOD_S: f64 = 4.0;
pub const KNEE_FLEXION_MAX_DEG: f64 = 90.0;

pub const ARM_RAISE_PERIOD_S: f64 = 4.0;
pub const ARM_RAISE_MAX_DEG: f64 = 150.0;

pub const FRONT_KICK_PERIOD_S: f64 = 2.5;
pub const FRONT_KICK_HIP_MAX_DEG: f64 = 80.0;
pub const FRONT_KICK_KNEE_MAX_DEG: f64 = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Motion {
    ReferencePose,
    Standing,
    Squat,
    KneeFlexion,
    ArmRaise,
    FrontKick,
    RoundhouseKick,
}

/// Joint angles a motion prescribes at a time, radians, in the joint definitions' sense.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ScriptedAngles {
    pub left_knee: f64,
    pub right_knee: f64,
    pub left_hip: f64,
    pub right_hip: f64,
    pub pelvis_tilt: f64,
    pub pelvis_yaw: f64,
    pub kick_plane_yaw: f64,
    pub trunk_lean: f64,
    pub left_shoulder: f64,
    pub right_shoulder: f64,
}

/// Smooth 0 → 1 → 0 over one period.
fn bump(phase: f64) -> f64 {
    (1.0 - (TAU * phase).cos()) / 2.0
}

fn phase(t_s: f64, period_s: f64) -> f64 {
    (t_s / period_s).rem_euclid(1.0)
}

impl Motion {
    pub const ALL: [Motion; 7] = [
        Motion::ReferencePose,
        Motion::Standing,
        Motion::Squat,
        Motion::KneeFlexion,
        Motion::ArmRaise,
        Motion::FrontKick,
        Motion::RoundhouseKick,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::ReferencePose => "reference_pose",
            Self::Standing => "standing",
            Self::Squat => "squat",
            Self::KneeFlexion => "knee_flexion",
            Self::ArmRaise => "arm_raise",
            Self::FrontKick => "front_kick",
            Self::RoundhouseKick => "roundhouse_kick",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::ReferencePose => "Reference pose",
            Self::Standing => "Standing",
            Self::Squat => "Squat",
            Self::KneeFlexion => "Knee flexion",
            Self::ArmRaise => "Arm raise",
            Self::FrontKick => "Front kick",
            Self::RoundhouseKick => "Roundhouse kick",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id() == id)
    }

    /// Prescribed angles at simulation time `t_s`.
    pub fn angles(self, t_s: f64) -> ScriptedAngles {
        let rad = f64::to_radians;
        match self {
            Self::ReferencePose => ScriptedAngles {
                left_shoulder: rad(REFERENCE_POSE_ARM_ELEVATION_DEG),
                right_shoulder: rad(REFERENCE_POSE_ARM_ELEVATION_DEG),
                ..Default::default()
            },
            Self::Standing => ScriptedAngles {
                trunk_lean: rad(STANDING_SWAY_DEG) * (TAU * phase(t_s, STANDING_PERIOD_S)).sin(),
                ..Default::default()
            },
            Self::Squat => {
                let s = bump(phase(t_s, SQUAT_PERIOD_S));
                let knee = rad(SQUAT_KNEE_MAX_DEG) * s;
                let tilt = rad(SQUAT_PELVIS_TILT_MAX_DEG) * s;
                // Thigh and shin tilt by half the knee angle each, so the feet stay under the hips.
                let hip = tilt + knee / 2.0;
                ScriptedAngles {
                    left_knee: knee,
                    right_knee: knee,
                    left_hip: hip,
                    right_hip: hip,
                    pelvis_tilt: tilt,
                    trunk_lean: rad(SQUAT_TRUNK_LEAN_MAX_DEG) * s,
                    ..Default::default()
                }
            }
            Self::KneeFlexion => {
                let p = phase(t_s, KNEE_FLEXION_PERIOD_S);
                let flex = |active: bool| {
                    if active {
                        rad(KNEE_FLEXION_MAX_DEG) * (2.0 * PI * p).sin().powi(2)
                    } else {
                        0.0
                    }
                };
                ScriptedAngles {
                    left_knee: flex(p < 0.5),
                    right_knee: flex(p >= 0.5),
                    ..Default::default()
                }
            }
            Self::ArmRaise => {
                let e = rad(ARM_RAISE_MAX_DEG) * bump(phase(t_s, ARM_RAISE_PERIOD_S));
                ScriptedAngles {
                    left_shoulder: e,
                    right_shoulder: e,
                    ..Default::default()
                }
            }
            Self::RoundhouseKick => {
                let t = t_s.rem_euclid(2.8);
                let smooth = |from: f64, to: f64, a: f64, b: f64| {
                    let u = ((t - a) / (b - a)).clamp(0.0, 1.0);
                    from + (to - from) * u * u * (3.0 - 2.0 * u)
                };
                let (hip, knee) = if t < 0.6 {
                    (smooth(0.0, 45.0, 0.0, 0.6), smooth(0.0, 60.0, 0.0, 0.6))
                } else if t < 0.9 {
                    (smooth(45.0, 60.0, 0.6, 0.9), 60.0)
                } else if t < 1.2 {
                    (smooth(60.0, 80.0, 0.9, 1.2), smooth(60.0, 0.0, 0.9, 1.2))
                } else if t < 1.6 {
                    (smooth(80.0, 45.0, 1.2, 1.6), smooth(0.0, 60.0, 1.2, 1.6))
                } else {
                    (smooth(45.0, 0.0, 1.6, 2.4), smooth(60.0, 0.0, 1.6, 2.4))
                };
                let pivot = if t < 1.6 {
                    smooth(0.0, 1.0, 0.6, 0.9)
                } else {
                    smooth(1.0, 0.0, 1.6, 2.4)
                };
                ScriptedAngles {
                    right_hip: rad(hip),
                    right_knee: rad(knee),
                    pelvis_yaw: rad(45.0) * pivot,
                    kick_plane_yaw: -PI / 2.0 * pivot,
                    ..Default::default()
                }
            }
            Self::FrontKick => {
                let s = bump(phase(t_s, FRONT_KICK_PERIOD_S));
                ScriptedAngles {
                    right_hip: rad(FRONT_KICK_HIP_MAX_DEG) * s,
                    right_knee: rad(FRONT_KICK_KNEE_MAX_DEG) * 4.0 * s * (1.0 - s),
                    ..Default::default()
                }
            }
        }
    }
}

/// Script metadata for deterministic tests, not a recognition algorithm.
#[derive(Debug, Clone)]
pub struct ScriptPhase {
    pub id: &'static str,
    pub start_s: f64,
    pub end_s: f64,
}
#[derive(Debug, Clone)]
pub struct ScriptPeak {
    pub id: &'static str,
    pub value: f64,
    pub t_s: f64,
}
#[derive(Debug, Clone)]
pub struct KickTruth {
    pub period_s: f64,
    pub phases: Vec<ScriptPhase>,
    pub strike_t_s: f64,
    pub peaks: Vec<ScriptPeak>,
}
impl Motion {
    pub fn kick_truth(self) -> Option<KickTruth> {
        let (period_s, strike_t_s, phases, peaks) = match self {
            Self::FrontKick => (
                FRONT_KICK_PERIOD_S,
                1.25,
                vec![
                    ("chamber", 0.0, 0.625),
                    ("extension", 0.625, 1.25),
                    ("retraction", 1.25, 1.875),
                    ("recovery", 1.875, 2.5),
                ],
                vec![
                    (
                        "right_hip_flexion",
                        FRONT_KICK_HIP_MAX_DEG.to_radians(),
                        1.25,
                    ),
                    (
                        "right_knee_flexion",
                        FRONT_KICK_KNEE_MAX_DEG.to_radians(),
                        0.625,
                    ),
                ],
            ),
            Self::RoundhouseKick => (
                2.8,
                1.2,
                vec![
                    ("chamber", 0.0, 0.6),
                    ("pivot", 0.6, 0.9),
                    ("extension", 0.9, 1.2),
                    ("retraction", 1.2, 1.6),
                    ("recovery", 1.6, 2.4),
                    ("ready", 2.4, 2.8),
                ],
                vec![
                    ("thigh_elevation.right", 80.0_f64.to_radians(), 1.2),
                    ("right_knee_flexion", 60.0_f64.to_radians(), 0.6),
                    ("pelvis_yaw", 45.0_f64.to_radians(), 0.9),
                ],
            ),
            _ => return None,
        };
        Some(KickTruth {
            period_s,
            strike_t_s,
            phases: phases
                .into_iter()
                .map(|(id, start_s, end_s)| ScriptPhase { id, start_s, end_s })
                .collect(),
            peaks: peaks
                .into_iter()
                .map(|(id, value, t_s)| ScriptPeak { id, value, t_s })
                .collect(),
        })
    }
}
