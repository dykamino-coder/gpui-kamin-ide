//! KaminIDE patch: фактический D3D device context и проверка строгого выбора.

use anyhow::{Result, bail};
use gpui::frame_capture::GpuDeviceContext;
use windows::{Win32::Graphics::{Direct3D11::ID3D11Device, Dxgi::{IDXGIAdapter1, IDXGIDevice}}, core::Interface};
use crate::{GpuAdapterInfo, GpuAdapterPreference};

pub(crate) fn validate_mode(preference: &GpuAdapterPreference, mode: gpui::DebugDevice) -> Result<()> {
    if *preference != GpuAdapterPreference::Default && mode != gpui::DebugDevice::Default {
        bail!("Explicit hardware GPU selection conflicts with {mode:?}; select default for this backend");
    }
    Ok(())
}

pub(crate) fn read(device: &ID3D11Device, generation: u64, factory_debug: bool, mode: gpui::DebugDevice) -> Result<GpuDeviceContext> {
    let dxgi: IDXGIDevice = device.cast()?;
    let adapter: IDXGIAdapter1 = unsafe { dxgi.GetAdapter()?.cast()? };
    let info = GpuAdapterInfo::read(&adapter)?;
    let desc = unsafe { adapter.GetDesc1()? };
    Ok(GpuDeviceContext {
        luid_high_part: info.luid_high_part, luid_low_part: info.luid_low_part,
        device_name: info.device_name, vendor_id: info.vendor_id, device_id: info.device_id,
        subsystem_id: desc.SubSysId, revision: desc.Revision, adapter_flags: desc.Flags,
        driver_version: info.driver_version, feature_level: unsafe { device.GetFeatureLevel().0 as u32 },
        creation_flags: unsafe { device.GetCreationFlags() }, factory_debug,
        backend: match mode { gpui::DebugDevice::Default => "d3d11", gpui::DebugDevice::Warp => "warp", gpui::DebugDevice::D3D11On12 => "d3d11on12" }.into(),
        generation, queried_from_device: true,
    })
}

pub(crate) fn pin(preference: &GpuAdapterPreference, context: &GpuDeviceContext) -> Result<GpuAdapterPreference> {
    if *preference == GpuAdapterPreference::Default { return Ok(preference.clone()); }
    if context.adapter_flags & 2 != 0 { bail!("Explicit GPU selection produced a software device"); }
    let actual = GpuAdapterPreference::ByLuid { high_part: context.luid_high_part, low_part: context.luid_low_part };
    if matches!(preference, GpuAdapterPreference::ByLuid { .. }) && *preference != actual {
        bail!("Requested GPU {preference:?} differs from actual device {actual:?}");
    }
    Ok(actual)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_conflicting_backend_before_device_creation() {
        for mode in [gpui::DebugDevice::Warp, gpui::DebugDevice::D3D11On12] {
            assert!(validate_mode(&GpuAdapterPreference::MinimumPower, mode).is_err());
            assert!(validate_mode(&GpuAdapterPreference::Default, mode).is_ok());
        }
    }
    #[test]
    fn parses_luid_without_losing_signed_high_part() {
        assert_eq!(GpuAdapterPreference::parse("luid:FFFFFFFF:000150BD").unwrap(), GpuAdapterPreference::ByLuid { high_part: -1, low_part: 86205 });
        for bad in ["luid:", "luid:x:0", "luid:0:100000000", ""] { assert!(GpuAdapterPreference::parse(bad).is_err()); }
    }
    fn hardware() -> GpuDeviceContext {
        GpuDeviceContext { luid_high_part: 0, luid_low_part: 17, device_name: "GPU".into(),
            vendor_id: 0x8086, device_id: 1, subsystem_id: 0, revision: 0, adapter_flags: 0,
            driver_version: "1.2.3.4".into(), feature_level: 0xb100, creation_flags: 0x22,
            factory_debug: true, backend: "d3d11".into(), generation: 1, queried_from_device: true }
    }
    #[test]
    fn power_selection_pins_actual_hardware_for_recovery() {
        let mut context = hardware();
        let luid = GpuAdapterPreference::ByLuid { high_part: 0, low_part: 17 };
        assert_eq!(pin(&GpuAdapterPreference::MinimumPower, &context).unwrap(), luid);
        assert!(pin(&luid, &context).is_ok());
        let captured = context.clone();
        context.generation += 1;
        context.luid_low_part += 1;
        assert!(pin(&luid, &context).is_err());
        assert_eq!(captured.generation, 1);
        assert_eq!(captured.luid_low_part, 17);
        context.adapter_flags = 2;
        assert!(pin(&GpuAdapterPreference::MinimumPower, &context).is_err());
        assert_eq!(pin(&GpuAdapterPreference::Default, &context).unwrap(), GpuAdapterPreference::Default);
    }

}
