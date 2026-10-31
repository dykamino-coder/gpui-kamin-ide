//! KaminIDE patch: публичные снимки фактического GPU платформы.

use super::*;

impl WindowsPlatform {
    /// KaminIDE patch: identity of the live renderer adapter; unavailable until the platform attaches its GPU.
    pub fn gpu_adapter_info(&self) -> Result<GpuAdapterInfo> {
        let devices = self.inner.state.directx_devices.borrow();
        let device = &devices.as_ref().context("GPU has not been attached")?.device;
        let dxgi: windows::Win32::Graphics::Dxgi::IDXGIDevice = device.cast()?;
        GpuAdapterInfo::read(&unsafe { dxgi.GetAdapter()?.cast()? })
    }

    /// KaminIDE patch: generation snapshot queried from the renderer device at creation.
    pub fn gpu_device_context(&self) -> Result<gpui::frame_capture::GpuDeviceContext> {
        Ok(self.inner.state.directx_devices.borrow().as_ref().context("GPU has not been attached")?.context.clone())
    }

}
