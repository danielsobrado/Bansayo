//! Test support: golden example values, TypeScript generation and the generated-file check.

#![allow(dead_code)]

pub mod examples;
pub mod generated;
pub mod skeleton;
pub mod state;

use bansayo_contracts::{DeviceId, ProfileId, Reason, SegmentId, SessionId, SourceId};

pub fn dev(s: &str) -> DeviceId {
    DeviceId::new(s).expect("valid device id")
}

pub fn src(s: &str) -> SourceId {
    SourceId::new(s).expect("valid source id")
}

pub fn seg(s: &str) -> SegmentId {
    SegmentId::new(s).expect("valid segment id")
}

pub fn profile(s: &str) -> ProfileId {
    ProfileId::new(s).expect("valid profile id")
}

pub fn session_id(s: &str) -> SessionId {
    SessionId::new(s).expect("valid session id")
}

pub fn reason(code: &str, subject: Option<&str>, message: &str) -> Reason {
    Reason {
        code: code.into(),
        subject: subject.map(Into::into),
        message: message.into(),
    }
}
