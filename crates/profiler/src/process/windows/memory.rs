//! Process usage on the renderer's exact adapter; no polling or PDH query.

use windows::{
    Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_MEMORY_SEGMENT_GROUP, DXGI_MEMORY_SEGMENT_GROUP_LOCAL,
        DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL, DXGI_QUERY_VIDEO_MEMORY_INFO, IDXGIAdapter3,
        IDXGIFactory1,
    },
    core::Interface,
};

pub struct Memory(IDXGIAdapter3);

impl Memory {
    pub fn open(high: i32, low: u32) -> windows::core::Result<Self> {
        // DXGI owns its COM reference; enumeration ends with DXGI_ERROR_NOT_FOUND.
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
            for index in 0.. {
                let adapter = factory.EnumAdapters1(index)?;
                let desc = adapter.GetDesc1()?;
                if desc.AdapterLuid.HighPart == high && desc.AdapterLuid.LowPart == low {
                    return Ok(Self(adapter.cast()?));
                }
            }
        }
        unreachable!("DXGI enumeration returns an adapter or DXGI_ERROR_NOT_FOUND")
    }

    pub(super) fn segment(&self, segment: DXGI_MEMORY_SEGMENT_GROUP) -> windows::core::Result<u64> {
        let mut info = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
        // Node zero of the selected adapter. CurrentUsage is this process's
        // usage, distinct from the adapter budget and logical engine owners.
        unsafe { self.0.QueryVideoMemoryInfo(0, segment, &mut info)? };
        Ok(info.CurrentUsage)
    }

    pub fn read(&self) -> windows::core::Result<(u64, u64)> {
        Ok((
            self.segment(DXGI_MEMORY_SEGMENT_GROUP_LOCAL)?,
            self.segment(DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL)?,
        ))
    }
}
