//! Validated identifier newtypes. Each serialises as a plain JSON string.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ts_rs::TS;

/// Maximum length of a device, source or segment identifier.
const MAX_ID_LEN: usize = 64;
/// Maximum length of a calibration profile identifier.
const MAX_PROFILE_ID_LEN: usize = 160;

/// An identifier string did not match its pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdError {
    kind: &'static str,
    expected: &'static str,
}

impl fmt::Display for IdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid {}: expected {}", self.kind, self.expected)
    }
}

impl std::error::Error for IdError {}

macro_rules! identifier {
    ($(#[$meta:meta])* $name:ident, $kind:literal, $expected:literal, $is_valid:path) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TS)]
        pub struct $name(String);

        impl $name {
            /// Validates `value` against the identifier pattern.
            pub fn new(value: impl Into<String>) -> Result<Self, IdError> {
                let value = value.into();
                if $is_valid(&value) {
                    Ok(Self(value))
                } else {
                    Err(IdError { kind: $kind, expected: $expected })
                }
            }

            /// The identifier as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl FromStr for $name {
            type Err = IdError;
            fn from_str(s: &str) -> Result<Self, IdError> {
                Self::new(s)
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdError;
            fn try_from(s: String) -> Result<Self, IdError> {
                Self::new(s)
            }
        }

        impl TryFrom<&str> for $name {
            type Error = IdError;
            fn try_from(s: &str) -> Result<Self, IdError> {
                Self::new(s)
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> String {
                id.0
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

identifier!(
    /// Hardware identity of a device, e.g. `SIM-T01`. Pattern `^[A-Za-z0-9][A-Za-z0-9_.-]{0,63}$`;
    /// safe to use in a file name.
    DeviceId,
    "device id",
    "1-64 characters from A-Z a-z 0-9 _ . -, starting with a letter or digit",
    is_device_like
);

identifier!(
    /// Identity of a configured source, e.g. `sim`. Same pattern as `DeviceId`.
    SourceId,
    "source id",
    "1-64 characters from A-Z a-z 0-9 _ . -, starting with a letter or digit",
    is_device_like
);

identifier!(
    /// Body segment identifier defined by the active skeleton, e.g. `left_thigh`.
    /// Pattern `^[a-z][a-z0-9_]{0,63}$`.
    SegmentId,
    "segment id",
    "1-64 characters from a-z 0-9 _, starting with a letter",
    is_segment
);

identifier!(
    /// Recording session identifier, e.g. `20261116T102205Z-a3f9`. UTC start time plus a random
    /// suffix. Pattern `^[0-9]{8}T[0-9]{6}Z-[a-z0-9]{4}$`; safe to use as a directory name.
    SessionId,
    "session id",
    "the form 20261116T102205Z-a3f9",
    is_session
);

identifier!(
    /// Calibration profile identifier, e.g. `mounting-SIM-T01-pelvis`.
    /// Pattern `^[A-Za-z0-9][A-Za-z0-9_.-]{0,159}$`; safe to use as a file name.
    ProfileId,
    "profile id",
    "1-160 characters from A-Z a-z 0-9 _ . -, starting with a letter or digit",
    is_profile
);

identifier!(
    /// Athlete identifier chosen at creation, e.g. `daniel`. Pattern `^[a-z0-9][a-z0-9_-]{0,31}$`;
    /// immutable and used as a directory name.
    AthleteId,
    "athlete id",
    "1-32 characters from a-z 0-9 _ -, starting with a letter or digit",
    is_athlete
);

fn is_athlete(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= 32
        && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit())
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'_' | b'-'))
}

fn is_file_safe(s: &str, max_len: usize) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= max_len
        && b[0].is_ascii_alphanumeric()
        && b.iter()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'.' | b'-'))
}

fn is_device_like(s: &str) -> bool {
    is_file_safe(s, MAX_ID_LEN)
}

fn is_profile(s: &str) -> bool {
    is_file_safe(s, MAX_PROFILE_ID_LEN)
}

fn is_segment(s: &str) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= MAX_ID_LEN
        && b[0].is_ascii_lowercase()
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_')
}

fn is_session(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 21
        && b[..8].iter().all(u8::is_ascii_digit)
        && b[8] == b'T'
        && b[9..15].iter().all(u8::is_ascii_digit)
        && b[15] == b'Z'
        && b[16] == b'-'
        && b[17..]
            .iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_and_source_ids() {
        for ok in [
            "SIM-T01",
            "T01",
            "sim",
            "a",
            "AA-BB-CC-00-11-22",
            "cam.left_1",
            &"x".repeat(64),
        ] {
            assert!(DeviceId::new(ok).is_ok(), "{ok}");
            assert!(SourceId::new(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "AA:BB:CC:00:11:22",
            "-T01",
            "_a",
            ".hidden",
            "a b",
            "a/b",
            "a\\b",
            "ü",
            "../x",
            &"x".repeat(65),
        ] {
            assert!(DeviceId::new(bad).is_err(), "{bad:?}");
            assert!(SourceId::new(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn athlete_ids() {
        for ok in ["daniel", "a", "0", "a_b-c", &"a".repeat(32)] {
            assert!(AthleteId::new(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "Daniel",
            "-a",
            "_a",
            "a b",
            "../a",
            "a.b",
            &"a".repeat(33),
        ] {
            assert!(AthleteId::new(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn profile_ids() {
        for ok in [
            "mounting-SIM-T01-pelvis",
            "camera-intrinsics-SIM-CAM1",
            "a",
            &"p".repeat(160),
        ] {
            assert!(ProfileId::new(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "../mounting",
            "..",
            ".hidden",
            "a/b",
            "a\\b",
            "C:x",
            "a b",
            "mounting-SIM-T01-pelvis.json\0",
            &"p".repeat(161),
        ] {
            assert!(ProfileId::new(bad).is_err(), "{bad:?}");
        }
        assert!(serde_json::from_str::<ProfileId>("\"../../x\"").is_err());
    }

    #[test]
    fn segment_ids() {
        for ok in ["pelvis", "left_thigh", "spine2", &"a".repeat(64)] {
            assert!(SegmentId::new(ok).is_ok(), "{ok}");
        }
        for bad in [
            "",
            "Pelvis",
            "left-thigh",
            "_pelvis",
            "2spine",
            "left thigh",
            &"a".repeat(65),
        ] {
            assert!(SegmentId::new(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn session_ids() {
        assert!(SessionId::new("20261116T102205Z-a3f9").is_ok());
        assert!(SessionId::new("00000000T000000Z-0000").is_ok());
        for bad in [
            "",
            "20261116T102205Z-A3F9",
            "20261116T102205Z-a3f",
            "20261116T102205Z-a3f9x",
            "20261116t102205Z-a3f9",
            "2026111T6102205Z-a3f9",
            "20261116T102205Z_a3f9",
            "../../../etc/passwd",
            "20261116T102205Z-/../",
            "20261116T102205Z-..\\a",
            "..\\20261116T102205Z-a3f9",
            "20261116T102205Z-a3f9/../x",
            "20261116T102205Z-a3f\0",
        ] {
            assert!(SessionId::new(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn serde_is_plain_string_and_validates() {
        let id = DeviceId::new("SIM-T01").ok();
        assert_eq!(
            serde_json::to_string(&id).ok().as_deref(),
            Some("\"SIM-T01\"")
        );
        assert!(serde_json::from_str::<DeviceId>("\"SIM-T01\"").is_ok());
        assert!(serde_json::from_str::<DeviceId>("\"bad id\"").is_err());
        assert!(serde_json::from_str::<SessionId>("\"../../etc\"").is_err());
        assert!(serde_json::from_str::<SegmentId>("42").is_err());
    }
}
