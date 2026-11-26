//! Process CPU snapshots exclude startup and retain the Windows counter granularity.

#[derive(Clone, Copy)]
pub struct Cpu(Option<u64>);

impl Cpu {
    pub fn read() -> Self {
        #[cfg(windows)]
        {
            use windows::Win32::{
                Foundation::FILETIME,
                System::Threading::{GetCurrentProcess, GetProcessTimes},
            };
            let (mut created, mut exited, mut kernel, mut user) = (
                FILETIME::default(),
                FILETIME::default(),
                FILETIME::default(),
                FILETIME::default(),
            );
            // The pseudo handle stays valid; output buffers outlive this call.
            let result = unsafe {
                GetProcessTimes(
                    GetCurrentProcess(),
                    &mut created,
                    &mut exited,
                    &mut kernel,
                    &mut user,
                )
            };
            let ticks =
                |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
            Self(result.ok().map(|()| ticks(kernel) + ticks(user)))
        }
        #[cfg(not(windows))]
        Self(None)
    }

    pub fn elapsed_ms(self) -> Option<f64> {
        Some(Self::read().0?.checked_sub(self.0?)? as f64 / 10_000.)
    }
}
