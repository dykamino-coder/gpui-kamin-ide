//! CPU time and memory of the whole viewer process, read from Windows counters.

#[derive(Clone, Copy, Default)]
pub struct Sample {
    pub cpu_100ns: u64,
    pub private: u64,
    pub working: u64,
    pub peak_working: u64,
}

#[cfg(windows)]
pub fn sample() -> Option<Sample> {
    use windows::Win32::{
        Foundation::FILETIME,
        System::{
            ProcessStatus::{
                GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
            },
            Threading::{GetCurrentProcess, GetProcessTimes},
        },
    };
    let ticks =
        |time: FILETIME| (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime);
    let (mut created, mut exited) = (FILETIME::default(), FILETIME::default());
    let (mut kernel, mut user) = (FILETIME::default(), FILETIME::default());
    // Pseudo handle of this process; the out values live until the call returns.
    unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut created,
            &mut exited,
            &mut kernel,
            &mut user,
        )
    }
    .ok()?;
    let mut counters = PROCESS_MEMORY_COUNTERS_EX {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        ..Default::default()
    };
    // The extended counters start as the basic ones; the buffer is this call's.
    let basic = &mut counters as *mut PROCESS_MEMORY_COUNTERS_EX as *mut PROCESS_MEMORY_COUNTERS;
    unsafe { GetProcessMemoryInfo(GetCurrentProcess(), basic, counters.cb) }.ok()?;
    Some(Sample {
        cpu_100ns: ticks(kernel) + ticks(user),
        private: counters.PrivateUsage as u64,
        working: counters.WorkingSetSize as u64,
        peak_working: counters.PeakWorkingSetSize as u64,
    })
}

#[cfg(not(windows))]
pub fn sample() -> Option<Sample> {
    None
}
