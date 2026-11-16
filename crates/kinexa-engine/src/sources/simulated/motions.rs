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
}

/// Joint angles a motion prescribes at a time, radians, in the joint definitions' sense.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ScriptedAngles {
    pub left_knee: f64,
    pub right_knee: f64,
    pub left_hip: f64,
    pub right_hip: f64,
    pub pelvis_tilt: f64,
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
    pub const ALL: [Motion; 6] = [
        Motion::ReferencePose,
        Motion::Standing,
        Motion::Squat,
        Motion::KneeFlexion,
        Motion::ArmRaise,
        Motion::FrontKick,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::ReferencePose => "reference_pose",
            Self::Standing => "standing",
            Self::Squat => "squat",
            Self::KneeFlexion => "knee_flexion",
            Self::ArmRaise => "arm_raise",
            Self::FrontKick => "front_kick",
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
                let flex = |active: bool| if active { rad(KNEE_FLEXION_MAX_DEG) * (2.0 * PI * p).sin().powi(2) } else { 0.0 };
                ScriptedAngles { left_knee: flex(p < 0.5), right_knee: flex(p >= 0.5), ..Default::default() }
            }
            Self::ArmRaise => {
                let e = rad(ARM_RAISE_MAX_DEG) * bump(phase(t_s, ARM_RAISE_PERIOD_S));
                ScriptedAngles { left_shoulder: e, right_shoulder: e, ..Default::default() }
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
