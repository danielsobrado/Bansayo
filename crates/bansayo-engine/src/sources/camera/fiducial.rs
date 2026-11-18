//! Optional AprilTag 3 detector using the upstream C ABI at runtime.
use crate::config::FiducialsConfig;
use image::{RgbImage, imageops::grayscale};
use libloading::Library;
use serde::Serialize;
use std::{
    ffi::{c_int, c_void},
    mem::size_of,
    path::Path,
    ptr::NonNull,
};

type Create = unsafe extern "C" fn() -> *mut c_void;
type Destroy = unsafe extern "C" fn(*mut c_void);
type AddFamily = unsafe extern "C" fn(*mut c_void, *mut c_void, c_int);
type Detect = unsafe extern "C" fn(*mut c_void, *mut GrayHeader) -> *mut ZArray;
type Free = unsafe extern "C" fn(*mut ZArray);

#[repr(C)]
struct GrayHeader {
    width: c_int,
    height: c_int,
    stride: c_int,
    buf: *mut u8,
}
#[repr(C)]
struct ZArray {
    el_sz: usize,
    size: c_int,
    alloc: c_int,
    data: *mut u8,
}
#[repr(C)]
struct RawTag {
    family: *mut c_void,
    id: c_int,
    hamming: c_int,
    decision_margin: f32,
    homography: *mut c_void,
    center: [f64; 2],
    corners: [[f64; 2]; 4],
}

#[derive(Debug, Clone, Serialize)]
pub struct Tag {
    pub family: String,
    pub id: u32,
    pub corners_px: [[f32; 2]; 4],
    pub hamming: u8,
    pub decision_margin: f32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Snapshot {
    pub camera_id: String,
    pub t_us: u64,
    pub image_size_px: [u32; 2],
    pub tags: Vec<Tag>,
}

struct Symbols {
    destroy_detector: Destroy,
    destroy_family: Destroy,
    detect: Detect,
    free: Free,
}
pub struct Native {
    detector: NonNull<c_void>,
    family: NonNull<c_void>,
    symbols: Symbols,
    config: FiducialsConfig,
    _library: Library,
}

fn symbol<T: Copy>(lib: &Library, name: &[u8]) -> Result<T, String> {
    unsafe { lib.get::<T>(name).map(|s| *s).map_err(|e| e.to_string()) }
}

fn family_symbols(name: &str) -> Result<(&'static [u8], &'static [u8]), String> {
    match name {
        "tag36h11" => Ok((b"tag36h11_create\0", b"tag36h11_destroy\0")),
        "tag25h9" => Ok((b"tag25h9_create\0", b"tag25h9_destroy\0")),
        "tagStandard41h12" => Ok((b"tagStandard41h12_create\0", b"tagStandard41h12_destroy\0")),
        "tagCircle49h12" => Ok((b"tagCircle49h12_create\0", b"tagCircle49h12_destroy\0")),
        _ => Err("Unsupported AprilTag family".into()),
    }
}

impl Native {
    pub fn open(config: &FiducialsConfig) -> Result<Self, String> {
        let default_name = if cfg!(windows) {
            "apriltag.dll"
        } else if cfg!(target_os = "macos") {
            "libapriltag.dylib"
        } else {
            "libapriltag.so"
        };
        let path = config
            .library_path
            .as_deref()
            .unwrap_or(Path::new(default_name));
        let library = unsafe { Library::new(path) }
            .map_err(|e| format!("Cannot load AprilTag library {}: {e}", path.display()))?;
        let create: Create = symbol(&library, b"apriltag_detector_create\0")?;
        let destroy_detector: Destroy = symbol(&library, b"apriltag_detector_destroy\0")?;
        let add_family: AddFamily = symbol(&library, b"apriltag_detector_add_family_bits\0")?;
        let detect: Detect = symbol(&library, b"apriltag_detector_detect\0")?;
        let free: Free = symbol(&library, b"apriltag_detections_destroy\0")?;
        let (make, drop_family) = family_symbols(&config.family)?;
        let make: Create = symbol(&library, make)?;
        let destroy_family: Destroy = symbol(&library, drop_family)?;
        let detector =
            NonNull::new(unsafe { create() }).ok_or("AprilTag detector creation failed")?;
        let family = match NonNull::new(unsafe { make() }) {
            Some(v) => v,
            None => {
                unsafe { destroy_detector(detector.as_ptr()) };
                return Err("AprilTag family creation failed".into());
            }
        };
        unsafe {
            add_family(
                detector.as_ptr(),
                family.as_ptr(),
                config.max_hamming.into(),
            )
        };
        Ok(Self {
            detector,
            family,
            symbols: Symbols {
                destroy_detector,
                destroy_family,
                detect,
                free,
            },
            config: config.clone(),
            _library: library,
        })
    }

    pub fn detect(&mut self, input: &RgbImage) -> Result<Vec<Tag>, String> {
        let (width, height) = input.dimensions();
        let mut gray = grayscale(input);
        let mut header = GrayHeader {
            width: width as c_int,
            height: height as c_int,
            stride: width as c_int,
            buf: gray.as_mut().as_mut_ptr(),
        };
        let raw = unsafe { (self.symbols.detect)(self.detector.as_ptr(), &mut header) };
        let ptr = NonNull::new(raw).ok_or("AprilTag returned a null result")?;
        let detections = Batch {
            ptr,
            free: self.symbols.free,
        };
        let array = unsafe { detections.ptr.as_ref() };
        if array.el_sz != size_of::<*const RawTag>()
            || !(0..=4096).contains(&array.size)
            || (array.size > 0 && array.data.is_null())
        {
            return Err("Invalid AprilTag detection array".into());
        }
        let mut tags = Vec::new();
        for i in 0..array.size as usize {
            let pointer = unsafe { *((array.data as *const *const RawTag).add(i)) };
            let Some(raw) = (unsafe { pointer.as_ref() }) else {
                continue;
            };
            if raw.id < 0 || !(0..=2).contains(&raw.hamming) {
                continue;
            }
            let corners = raw.corners.map(|c| [c[0] as f32, c[1] as f32]);
            if corners.iter().any(|c| {
                !c[0].is_finite()
                    || !c[1].is_finite()
                    || c[0] < -8.0
                    || c[1] < -8.0
                    || c[0] > width as f32 + 8.0
                    || c[1] > height as f32 + 8.0
            }) {
                continue;
            }
            let tag = Tag {
                family: self.config.family.clone(),
                id: raw.id as u32,
                corners_px: corners,
                hamming: raw.hamming as u8,
                decision_margin: raw.decision_margin,
            };
            if accepts(&self.config, &tag) {
                tags.push(tag);
            }
        }
        tags.sort_by_key(|tag| tag.id);
        Ok(tags)
    }
}

fn accepts(config: &FiducialsConfig, tag: &Tag) -> bool {
    tag.hamming <= config.max_hamming
        && tag.decision_margin.is_finite()
        && tag.decision_margin >= config.min_decision_margin
        && (config.allowed_ids.is_empty() || config.allowed_ids.contains(&tag.id))
}

struct Batch {
    ptr: NonNull<ZArray>,
    free: Free,
}
impl Drop for Batch {
    fn drop(&mut self) {
        unsafe { (self.free)(self.ptr.as_ptr()) }
    }
}
impl Drop for Native {
    fn drop(&mut self) {
        unsafe {
            (self.symbols.destroy_detector)(self.detector.as_ptr());
            (self.symbols.destroy_family)(self.family.as_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quality_and_registry_filter() {
        let mut config = FiducialsConfig::default();
        config.allowed_ids.push(100);
        let tag = Tag {
            family: "tag36h11".into(),
            id: 100,
            corners_px: [[0.0; 2]; 4],
            hamming: 0,
            decision_margin: 40.0,
        };
        assert!(accepts(&config, &tag));
        assert!(!accepts(
            &config,
            &Tag {
                id: 101,
                ..tag.clone()
            }
        ));
        assert!(!accepts(
            &config,
            &Tag {
                decision_margin: 10.0,
                ..tag
            }
        ));
    }
}
