//! Physical camera identity: parsing the operating-system endpoint, matching a configured
//! selector against the cameras present, and the persisted device profile that ties a logical
//! camera id to the hardware its calibration was made with. The system list index is never an
//! identity: enumeration order changes with hotplug and reboots.
use bansayo_contracts::{CameraIdentity, CaptureMode, DeviceId};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A camera present on the system now.
#[derive(Debug, Clone, PartialEq)]
pub struct Present {
    /// Position in the backend's current list; valid only until the next enumeration.
    pub index: u32,
    pub identity: CameraIdentity,
}

/// How a configured camera names its hardware. At least one field is set.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selector {
    /// Legacy: a position in the system list. Used only when nothing else is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    /// `vid_0c45&pid_6369`-style substring of the endpoint, case-insensitive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hardware_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<String>,
    /// The full endpoint, to tell identical cameras without serial numbers apart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interface_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MatchError {
    NotFound,
    /// Several cameras fit the selector and none is distinguished by serial or endpoint.
    Ambiguous(Vec<String>),
}

impl std::fmt::Display for MatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "no connected camera matches the configured identity"),
            Self::Ambiguous(found) => write!(
                f,
                "{} identical cameras match ({}); set interface_id to choose one",
                found.len(),
                found.join(", ")
            ),
        }
    }
}

/// Parses a Windows device interface path, e.g.
/// `\\?\usb#vid_0c45&pid_6369&mi_00#6&2a4f1d4b&0&0000#{e5323777-…}\global`.
/// Windows generates instance ids containing `&` for devices without a serial number (and for
/// every interface of a composite device), so only an `&`-free instance id is taken as a serial.
pub fn identity_from_endpoint(name: &str, endpoint: &str) -> CameraIdentity {
    let lower = endpoint.to_ascii_lowercase();
    let trimmed = lower.trim_start_matches(r"\\?\");
    // The interface class GUID and reference string do not identify the device.
    let interface = trimmed.split("#{").next().unwrap_or(trimmed).to_owned();
    let field = |key: &str| {
        interface
            .split(['#', '&'])
            .find_map(|part| part.strip_prefix(key))
            .filter(|v| v.len() == 4 && v.chars().all(|c| c.is_ascii_hexdigit()))
            .map(str::to_owned)
    };
    let (vendor_id, product_id) = (field("vid_"), field("pid_"));
    let serial = interface
        .split('#')
        .nth(2)
        .filter(|s| !s.is_empty() && !s.contains('&'))
        .map(str::to_owned);
    let fingerprint = match (&vendor_id, &product_id) {
        (Some(v), Some(p)) => format!(
            "usb:{v}:{p}:{}",
            serial.as_deref().unwrap_or(interface.as_str())
        ),
        _ => interface.clone(),
    };
    CameraIdentity {
        name: name.to_owned(),
        vendor_id,
        product_id,
        serial,
        interface_id: (!interface.is_empty()).then_some(interface),
        fingerprint,
    }
}

/// The one present camera the selector names. Order: serial (+ vendor/product), endpoint,
/// hardware id when unique, legacy index. Two identical cameras are never chosen between silently.
pub fn select<'a>(selector: &Selector, present: &'a [Present]) -> Result<&'a Present, MatchError> {
    let lower = |s: &Option<String>| s.as_ref().map(|s| s.to_ascii_lowercase());
    let hardware = lower(&selector.hardware_id);
    let fits_hardware = |p: &&Present| {
        hardware.as_ref().is_none_or(|h| {
            p.identity
                .interface_id
                .as_ref()
                .is_some_and(|i| i.contains(h.as_str()))
        })
    };
    if let Some(serial) = lower(&selector.serial) {
        return present
            .iter()
            .filter(fits_hardware)
            .find(|p| p.identity.serial.as_deref() == Some(serial.as_str()))
            .ok_or(MatchError::NotFound);
    }
    if let Some(endpoint) = lower(&selector.interface_id) {
        let endpoint = identity_from_endpoint("", &endpoint).interface_id;
        return present
            .iter()
            .find(|p| p.identity.interface_id == endpoint)
            .ok_or(MatchError::NotFound);
    }
    if hardware.is_some() {
        let found: Vec<&Present> = present.iter().filter(fits_hardware).collect();
        return match found.as_slice() {
            [one] => Ok(one),
            [] => Err(MatchError::NotFound),
            many => Err(MatchError::Ambiguous(
                many.iter()
                    .map(|p| p.identity.interface_id.clone().unwrap_or_default())
                    .collect(),
            )),
        };
    }
    selector
        .index
        .and_then(|i| present.iter().find(|p| p.index == i))
        .ok_or(MatchError::NotFound)
}

pub const PROFILE_SCHEMA_VERSION: u32 = 1;

/// Persisted per logical camera in `<data_dir>/cameras/<id>.json`: which hardware the camera's
/// calibration belongs to and what it was last opened with. Preferences, not live measurements.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceProfile {
    pub schema_version: u32,
    pub logical_id: DeviceId,
    pub identity: CameraIdentity,
    /// `front`, `left`, `right`, `rear` or any user label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// The last mode the camera streamed in, verified by opening it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_mode: Option<CaptureMode>,
    pub last_opened_utc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_confirmed_utc: Option<String>,
}

pub fn profile_path(data_dir: &Path, id: &DeviceId) -> PathBuf {
    data_dir.join("cameras").join(format!("{id}.json"))
}

/// The stored profile, or `None` when absent or unreadable (a broken file is never overwritten
/// silently: the caller treats the identity as unconfirmed).
pub fn load_profile(data_dir: &Path, id: &DeviceId) -> Result<Option<DeviceProfile>, String> {
    let path = profile_path(data_dir, id);
    match std::fs::read(&path) {
        // Windows editors (Notepad, PowerShell 5) write a UTF-8 byte-order mark.
        Ok(bytes) => serde_json::from_slice::<DeviceProfile>(
            bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes),
        )
        .map_err(|e| format!("{}: {e}", path.display()))
        .and_then(|p| {
            if p.schema_version == PROFILE_SCHEMA_VERSION && &p.logical_id == id {
                Ok(Some(p))
            } else {
                Err(format!("{}: unsupported schema or id", path.display()))
            }
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

pub fn save_profile(data_dir: &Path, profile: &DeviceProfile) -> std::io::Result<()> {
    let path = profile_path(data_dir, &profile.logical_id);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(profile).map_err(std::io::Error::other)?;
    crate::registry::persistence::atomic_write(&path, &json)
}

/// Whether the matched hardware is the hardware the profile (and so the calibration) belongs to.
/// A first sighting has nothing to contradict and is recorded as the reference.
pub fn confirmed(profile: Option<&DeviceProfile>, found: &CameraIdentity) -> bool {
    profile.is_none_or(|p| p.identity.fingerprint == found.fingerprint)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cam(index: u32, endpoint: &str) -> Present {
        Present {
            index,
            identity: identity_from_endpoint("USB Camera", endpoint),
        }
    }
    const A: &str = r"\\?\usb#vid_0c45&pid_6369&mi_00#6&2a4f1d4b&0&0000#{e5323777-f976-4f5b-9b55-b94699c46e44}\global";
    const B: &str = r"\\?\usb#vid_0c45&pid_6369&mi_00#6&11aa22bb&0&0000#{e5323777-f976-4f5b-9b55-b94699c46e44}\global";
    const SERIAL: &str =
        r"\\?\usb#vid_046d&pid_085e#4c7a1f2e#{e5323777-f976-4f5b-9b55-b94699c46e44}\global";

    #[test]
    fn endpoint_parsing_keeps_vendor_product_and_only_real_serials() {
        let a = identity_from_endpoint("UGREEN", A);
        assert_eq!(a.vendor_id.as_deref(), Some("0c45"));
        assert_eq!(a.product_id.as_deref(), Some("6369"));
        assert_eq!(a.serial, None, "a generated instance id is not a serial");
        assert_eq!(
            a.interface_id.as_deref(),
            Some("usb#vid_0c45&pid_6369&mi_00#6&2a4f1d4b&0&0000")
        );
        let s = identity_from_endpoint("Logitech", SERIAL);
        assert_eq!(s.serial.as_deref(), Some("4c7a1f2e"));
        assert_eq!(s.fingerprint, "usb:046d:085e:4c7a1f2e");
        let other = identity_from_endpoint("Virtual", "some-virtual-camera");
        assert_eq!(other.vendor_id, None);
        assert_eq!(other.fingerprint, "some-virtual-camera");
    }

    #[test]
    fn hardware_id_survives_a_changed_enumeration_order() {
        let selector = Selector {
            hardware_id: Some("VID_046D&PID_085E".into()),
            ..Default::default()
        };
        let before = [cam(0, SERIAL), cam(1, A)];
        let after = [cam(0, A), cam(1, SERIAL)];
        assert_eq!(select(&selector, &before).unwrap().index, 0);
        assert_eq!(select(&selector, &after).unwrap().index, 1);
    }

    #[test]
    fn identical_cameras_without_serials_are_never_chosen_between() {
        let present = [cam(0, A), cam(1, B)];
        let by_model = Selector {
            hardware_id: Some("vid_0c45&pid_6369".into()),
            ..Default::default()
        };
        assert!(matches!(
            select(&by_model, &present),
            Err(MatchError::Ambiguous(v)) if v.len() == 2
        ));
        let pinned = Selector {
            interface_id: Some(B.into()),
            ..Default::default()
        };
        assert_eq!(select(&pinned, &present).unwrap().index, 1);
        // Same pin with the cameras enumerated the other way round.
        assert_eq!(select(&pinned, &[cam(0, B), cam(1, A)]).unwrap().index, 0);
    }

    #[test]
    fn missing_camera_and_serial_selection() {
        let selector = Selector {
            serial: Some("4C7A1F2E".into()),
            ..Default::default()
        };
        assert_eq!(select(&selector, &[cam(0, A)]), Err(MatchError::NotFound));
        assert_eq!(
            select(&selector, &[cam(0, A), cam(3, SERIAL)])
                .unwrap()
                .index,
            3
        );
        let legacy = Selector {
            index: Some(2),
            ..Default::default()
        };
        assert_eq!(select(&legacy, &[cam(0, A)]), Err(MatchError::NotFound));
    }

    #[test]
    fn a_different_unit_on_the_same_logical_id_is_not_confirmed_and_profiles_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let id = DeviceId::new("CAM1").unwrap();
        assert_eq!(load_profile(dir.path(), &id).unwrap(), None);
        let a = identity_from_endpoint("UGREEN", A);
        let profile = DeviceProfile {
            schema_version: PROFILE_SCHEMA_VERSION,
            logical_id: id.clone(),
            identity: a.clone(),
            role: Some("front".into()),
            last_mode: None,
            last_opened_utc: "2026-11-19T10:00:00Z".into(),
            identity_confirmed_utc: None,
        };
        save_profile(dir.path(), &profile).unwrap();
        let back = load_profile(dir.path(), &id).unwrap().unwrap();
        assert_eq!(back, profile);
        assert!(confirmed(Some(&back), &a));
        assert!(!confirmed(
            Some(&back),
            &identity_from_endpoint("UGREEN", B)
        ));
        assert!(confirmed(None, &a));
        let mut bom = b"\xEF\xBB\xBF".to_vec();
        bom.extend(serde_json::to_vec(&profile).unwrap());
        std::fs::write(profile_path(dir.path(), &id), bom).unwrap();
        assert_eq!(
            load_profile(dir.path(), &id).unwrap(),
            Some(profile.clone())
        );
        std::fs::write(profile_path(dir.path(), &id), b"{broken").unwrap();
        assert!(load_profile(dir.path(), &id).is_err());
    }
}
