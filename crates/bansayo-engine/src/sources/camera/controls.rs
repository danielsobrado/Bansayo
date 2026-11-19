//! Manual exposure and gain on a system camera (martial-arts 08 §3.2, ADR-020).
//!
//! `nokhwa` 0.10 cannot switch a Media Foundation control from automatic to manual: its
//! `set_camera_control` keeps the control's current flag. This is ADR-020's fallback, a thin Media
//! Foundation wrapper inside the camera module: the device's media source is activated a second
//! time and its `IAMCameraControl` / `IAMVideoProcAmp` are set to manual. UVC controls belong to
//! the device, so they hold for the stream nokhwa reads.

/// Values in effect after [`apply`], read back from the device.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Applied {
    /// Seconds, from the device's log2 exposure value.
    pub exposure_s: Option<f64>,
    /// Device units of `VideoProcAmp_Gain`.
    pub gain: Option<f64>,
}

/// Media Foundation exposure is `log2(seconds)` in whole steps (`-7` = 1/128 s).
pub fn exposure_step(exposure_s: f64) -> i32 {
    exposure_s.log2().round() as i32
}

pub fn exposure_seconds(step: i32) -> f64 {
    2f64.powi(step)
}

/// The device's media source, activated beside the stream nokhwa holds.
#[cfg(windows)]
fn media_source(
    index: u32,
) -> Result<windows::Win32::Media::MediaFoundation::IMFMediaSource, String> {
    use windows::Win32::Media::MediaFoundation::{
        IMFActivate, IMFAttributes, IMFMediaSource, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
        MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID, MF_VERSION, MFCreateAttributes,
        MFEnumDeviceSources, MFSTARTUP_FULL, MFStartup,
    };
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoTaskMemFree};
    // SAFETY: plain COM and Media Foundation calls; every out-pointer is a local and the
    // activation array is freed below.
    unsafe {
        // Already initialised on this thread by nokhwa is fine: both calls are reference counted.
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        MFStartup(MF_VERSION, MFSTARTUP_FULL).map_err(|e| format!("MFStartup: {e}"))?;
        let mut attributes: Option<IMFAttributes> = None;
        MFCreateAttributes(&mut attributes, 1).map_err(|e| e.to_string())?;
        let attributes = attributes.ok_or("MFCreateAttributes returned nothing")?;
        attributes
            .SetGUID(
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE,
                &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
            )
            .map_err(|e| e.to_string())?;
        let mut list: *mut Option<IMFActivate> = std::ptr::null_mut();
        let mut count = 0u32;
        MFEnumDeviceSources(&attributes, &mut list, &mut count).map_err(|e| e.to_string())?;
        let devices: Vec<Option<IMFActivate>> = if list.is_null() {
            vec![]
        } else {
            let v = (0..count as usize).map(|i| (*list.add(i)).take()).collect();
            CoTaskMemFree(Some(list.cast()));
            v
        };
        // Same enumeration and order as nokhwa's device indices.
        let device = devices
            .get(index as usize)
            .and_then(Option::as_ref)
            .ok_or_else(|| format!("no system camera {index}"))?;
        device
            .ActivateObject::<IMFMediaSource>()
            .map_err(|e| format!("activate: {e}"))
    }
}

/// Hands exposure back to the camera's automatic control (undoes [`apply`]).
#[cfg(windows)]
pub fn automatic_exposure(index: u32) -> Result<(), String> {
    use windows::Win32::Media::DirectShow::{
        CameraControl_Exposure, CameraControl_Flags_Auto, IAMCameraControl,
    };
    use windows::core::Interface;
    let source = media_source(index)?;
    // SAFETY: COM calls on a live interface with local out-pointers.
    unsafe {
        let control: IAMCameraControl = source.cast().map_err(|e| e.to_string())?;
        let property = CameraControl_Exposure.0;
        let (mut min, mut max, mut step, mut default, mut caps) = (0, 0, 0, 0, 0);
        control
            .GetRange(
                property,
                &mut min,
                &mut max,
                &mut step,
                &mut default,
                &mut caps,
            )
            .map_err(|e| e.to_string())?;
        control
            .Set(property, default, CameraControl_Flags_Auto.0)
            .map_err(|e| e.to_string())?;
        let _ = source.Shutdown();
    }
    Ok(())
}

#[cfg(windows)]
pub fn apply(index: u32, exposure_s: Option<f64>, gain: Option<f64>) -> Result<Applied, String> {
    use windows::Win32::Media::DirectShow::{
        CameraControl_Exposure, CameraControl_Flags_Manual, IAMCameraControl, IAMVideoProcAmp,
        VideoProcAmp_Flags_Manual, VideoProcAmp_Gain,
    };
    use windows::core::Interface;
    let source = media_source(index)?;
    // SAFETY: COM calls on live interfaces with local out-pointers.
    unsafe {
        let mut applied = Applied::default();
        if let Some(seconds) = exposure_s {
            let control: IAMCameraControl = source
                .cast()
                .map_err(|_| "exposure_s: the device has no camera control interface")?;
            let property = CameraControl_Exposure.0;
            let (mut min, mut max, mut step, mut default, mut caps) = (0, 0, 0, 0, 0);
            control
                .GetRange(
                    property,
                    &mut min,
                    &mut max,
                    &mut step,
                    &mut default,
                    &mut caps,
                )
                .map_err(|e| format!("exposure_s: the device reports no exposure range ({e})"))?;
            if caps & CameraControl_Flags_Manual.0 == 0 {
                return Err("exposure_s: the device offers no manual exposure".into());
            }
            let wanted = exposure_step(seconds).clamp(min, max);
            control
                .Set(property, wanted, CameraControl_Flags_Manual.0)
                .map_err(|e| format!("exposure_s: the device refused {wanted} ({e})"))?;
            let (mut value, mut flags) = (0, 0);
            control
                .Get(property, &mut value, &mut flags)
                .map_err(|e| format!("exposure_s: could not be read back ({e})"))?;
            if flags & CameraControl_Flags_Manual.0 == 0 {
                return Err("exposure_s: the device stayed on automatic exposure".into());
            }
            applied.exposure_s = Some(exposure_seconds(value));
        }
        if let Some(g) = gain {
            let amp: IAMVideoProcAmp = source
                .cast()
                .map_err(|_| "gain: the device has no video processing interface")?;
            let property = VideoProcAmp_Gain.0;
            let (mut min, mut max, mut step, mut default, mut caps) = (0, 0, 0, 0, 0);
            amp.GetRange(
                property,
                &mut min,
                &mut max,
                &mut step,
                &mut default,
                &mut caps,
            )
            .map_err(|e| format!("gain: the device reports no gain range ({e})"))?;
            let wanted = (g.round() as i32).clamp(min, max);
            amp.Set(property, wanted, VideoProcAmp_Flags_Manual.0)
                .map_err(|e| format!("gain: the device refused {wanted} ({e})"))?;
            let (mut value, mut flags) = (0, 0);
            amp.Get(property, &mut value, &mut flags)
                .map_err(|e| format!("gain: could not be read back ({e})"))?;
            applied.gain = Some(f64::from(value));
        }
        let _ = source.Shutdown();
        Ok(applied)
    }
}

#[cfg(not(windows))]
pub fn automatic_exposure(_: u32) -> Result<(), String> {
    Ok(())
}

#[cfg(not(windows))]
pub fn apply(_: u32, exposure_s: Option<f64>, gain: Option<f64>) -> Result<Applied, String> {
    match (exposure_s, gain) {
        (None, None) => Ok(Applied::default()),
        (Some(_), _) => Err("exposure_s: manual exposure is implemented on Windows only".into()),
        _ => Err("gain: manual gain is implemented on Windows only".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposure_steps_are_whole_log2_seconds() {
        assert_eq!(exposure_step(1.0 / 128.0), -7);
        assert_eq!(exposure_step(0.002), -9); // 1/512 s is the nearest step to 1/500 s
        assert_eq!(exposure_seconds(-9), 1.0 / 512.0);
        assert_eq!(exposure_step(1.0 / 30.0), -5);
    }

    /// Needs a camera: `BANSAYO_CAMERA=0 BANSAYO_EXPOSURE_S=0.004 cargo test -p bansayo-engine
    /// -- --ignored --nocapture manual_exposure`.
    #[test]
    #[ignore]
    fn manual_exposure_on_a_physical_camera() {
        use crate::sources::camera::capture::{Backend, open};
        let index: u32 = std::env::var("BANSAYO_CAMERA").unwrap().parse().unwrap();
        let exposure: f64 =
            std::env::var("BANSAYO_EXPOSURE_S").map_or(0.004, |s| s.parse().unwrap());
        let selector = crate::sources::camera::identity::Selector {
            index: Some(index),
            ..Default::default()
        };
        let mut c = open(&Backend::Device(selector), [1280, 720], 30.0).unwrap();
        c.next().unwrap();
        let applied = apply(index, Some(exposure), None).unwrap();
        eprintln!("requested {exposure} s, in effect {applied:?}");
        for _ in 0..5 {
            c.next().unwrap();
        }
        let t = std::time::Instant::now();
        for _ in 0..60 {
            c.next().unwrap();
        }
        eprintln!("{:.1} frames/s", 60.0 / t.elapsed().as_secs_f64());
        automatic_exposure(index).unwrap();
    }
}
