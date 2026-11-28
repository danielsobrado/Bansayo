//! Process resource counters, in platform-independent units.
#[derive(Debug, Clone, Copy, Default)]
pub struct Counters {
    pub cpu_seconds: Option<f64>,
    pub resident_bytes: Option<u64>,
}

#[cfg(target_os = "linux")]
pub fn read() -> Counters {
    let resident_bytes = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|line| line.starts_with("VmRSS:"))?
                .split_whitespace()
                .nth(1)?
                .parse::<u64>()
                .ok()
        })
        .and_then(|kb| kb.checked_mul(1024));
    Counters {
        cpu_seconds: process_cpu(),
        resident_bytes,
    }
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
fn process_cpu() -> Option<f64> {
    #[repr(C)]
    struct Timespec {
        sec: i64,
        nsec: i64,
    }
    unsafe extern "C" {
        fn clock_gettime(clock: i32, value: *mut Timespec) -> i32;
    }
    const CLOCK_PROCESS_CPUTIME_ID: i32 = 2;
    let mut value = Timespec { sec: 0, nsec: 0 };
    // SAFETY: the Linux 64-bit timespec layout matches the writable struct.
    let result = unsafe { clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &mut value) };
    (result == 0 && value.sec >= 0 && (0..1_000_000_000).contains(&value.nsec))
        .then(|| value.sec as f64 + value.nsec as f64 / 1e9)
}

#[cfg(all(target_os = "linux", not(target_pointer_width = "64")))]
fn process_cpu() -> Option<f64> {
    None
}

#[cfg(windows)]
pub fn read() -> Counters {
    use std::ffi::c_void;
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    impl FileTime {
        fn seconds(&self) -> f64 {
            ((u64::from(self.high) << 32) | u64::from(self.low)) as f64 / 1e7
        }
    }
    #[repr(C)]
    #[derive(Default)]
    struct MemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn GetProcessTimes(
            process: *mut c_void,
            creation: *mut FileTime,
            exit: *mut FileTime,
            kernel: *mut FileTime,
            user: *mut FileTime,
        ) -> i32;
        fn K32GetProcessMemoryInfo(
            process: *mut c_void,
            counters: *mut MemoryCounters,
            size: u32,
        ) -> i32;
    }
    let mut creation = FileTime::default();
    let mut exit = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();
    let size = std::mem::size_of::<MemoryCounters>() as u32;
    let mut memory = MemoryCounters {
        cb: size,
        ..Default::default()
    };
    // SAFETY: the pseudo-handle identifies this process; all output pointers are valid,
    // correctly sized Windows ABI structures, and no handle ownership is transferred.
    unsafe {
        let handle = GetCurrentProcess();
        let time = GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user);
        let mem = K32GetProcessMemoryInfo(handle, &mut memory, size);
        Counters {
            cpu_seconds: (time != 0).then(|| kernel.seconds() + user.seconds()),
            resident_bytes: (mem != 0).then_some(memory.working_set_size as u64),
        }
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn read() -> Counters {
    Counters::default()
}
