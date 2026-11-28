//! Windows process and selected-adapter counters, shared by all engine clients.

use super::{Adapter, Field, Gpu, Sample};
use windows::Win32::{
    Foundation::FILETIME,
    Graphics::Dxgi::{DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL},
    System::{
        ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
        },
        Threading::{GetCurrentProcess, GetProcessTimes},
        WindowsProgramming::QueryProcessCycleTime,
    },
};

mod memory;
mod utilization;
pub use memory::Memory;

fn field<T>(result: windows::core::Result<T>) -> Field<T> {
    match result {
        Ok(value) => Field::available(value),
        Err(error) => Field::unavailable(error.to_string()),
    }
}

pub(super) fn sample() -> Sample {
    let mut sample = super::unsupported();
    sample.gpu = Gpu::unavailable(None, "renderer adapter not supplied");
    let (mut created, mut exited, mut kernel, mut user) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // Pseudo handle of this process; out values live through the calls.
    sample.cpu_100ns = field(
        unsafe {
            GetProcessTimes(
                GetCurrentProcess(),
                &mut created,
                &mut exited,
                &mut kernel,
                &mut user,
            )
        }
        .map(|()| {
            let ticks =
                |t: FILETIME| (u64::from(t.dwHighDateTime) << 32) | u64::from(t.dwLowDateTime);
            ticks(kernel) + ticks(user)
        }),
    );
    sample.cpu_cycles = cycles();
    let mut memory = PROCESS_MEMORY_COUNTERS_EX {
        cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        ..Default::default()
    };
    // EX starts with PROCESS_MEMORY_COUNTERS, and cb describes the full buffer.
    let result = unsafe {
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            (&raw mut memory).cast::<PROCESS_MEMORY_COUNTERS>(),
            memory.cb,
        )
    };
    match result {
        Ok(()) => {
            sample.private_commit_bytes = Field::available(memory.PrivateUsage as u64);
            sample.working_set_bytes = Field::available(memory.WorkingSetSize as u64);
            sample.lifetime_peak_private_commit_bytes =
                Field::available(memory.PeakPagefileUsage as u64);
            sample.lifetime_peak_working_set_bytes =
                Field::available(memory.PeakWorkingSetSize as u64);
        }
        Err(error) => {
            let reason = error.to_string();
            sample.private_commit_bytes = Field::unavailable(&reason);
            sample.working_set_bytes = Field::unavailable(&reason);
            sample.lifetime_peak_private_commit_bytes = Field::unavailable(&reason);
            sample.lifetime_peak_working_set_bytes = Field::unavailable(&reason);
        }
    }
    sample
}

pub(super) fn cycles() -> Field<u64> {
    let mut cycles = 0;
    // This process's pseudo handle; the output lives through the call.
    field(unsafe { QueryProcessCycleTime(GetCurrentProcess(), &mut cycles) }.map(|()| cycles))
}

pub(super) struct GpuCounters {
    adapter: Adapter,
    memory: windows::core::Result<Memory>,
    utilization: utilization::Counters,
}

impl GpuCounters {
    pub(super) fn new(adapter: Adapter) -> Self {
        Self {
            adapter,
            memory: Memory::open(adapter.luid_high, adapter.luid_low),
            utilization: utilization::Counters::new(adapter),
        }
    }

    pub(super) fn poll(&mut self) -> Gpu {
        let read = |segment| match &self.memory {
            Ok(memory) => field(memory.segment(segment)),
            Err(error) => Field::unavailable(error.to_string()),
        };
        Gpu {
            adapter: Some(self.adapter),
            node: 0,
            local_bytes: read(DXGI_MEMORY_SEGMENT_GROUP_LOCAL),
            non_local_bytes: read(DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL),
            utilization: self.utilization.poll(),
        }
    }
}
