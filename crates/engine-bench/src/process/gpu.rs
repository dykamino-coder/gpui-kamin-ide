//! Память DXGI текущего процесса на выбранном адаптере; LOCAL и NON_LOCAL отдельно.

use windows::{
    Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL,
        DXGI_QUERY_VIDEO_MEMORY_INFO, IDXGIAdapter3, IDXGIFactory1,
    },
    core::Interface,
};

pub(crate) struct Memory(IDXGIAdapter3);
impl Memory {
    pub(crate) fn open(high: i32, low: u32) -> windows::core::Result<Self> {
        // DXGI удерживает COM-ссылку; сравниваем LUID реально выбранного GPU.
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1()?;
            let mut index = 0;
            loop {
                let adapter = factory.EnumAdapters1(index)?;
                let desc = adapter.GetDesc1()?;
                if desc.AdapterLuid.HighPart == high && desc.AdapterLuid.LowPart == low {
                    return Ok(Self(adapter.cast()?));
                }
                index += 1;
            }
        }
    }
    pub(crate) fn read(&self) -> windows::core::Result<(u64, u64)> {
        // CurrentUsage относится к процессу; Budget не выдаётся за потребление.
        unsafe {
            let mut local = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
            let mut shared = DXGI_QUERY_VIDEO_MEMORY_INFO::default();
            self.0
                .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_LOCAL, &mut local)?;
            self.0
                .QueryVideoMemoryInfo(0, DXGI_MEMORY_SEGMENT_GROUP_NON_LOCAL, &mut shared)?;
            Ok((local.CurrentUsage, shared.CurrentUsage))
        }
    }
}
