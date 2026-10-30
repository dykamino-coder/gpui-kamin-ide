//! KaminIDE patch: создание device на выбранном адаптере без скрытого fallback.

use super::*;

#[inline]
pub(super) fn get_adapter(
    dxgi_factory: &IDXGIFactory6,
    debug_layer_available: bool,
    preference: &crate::GpuAdapterPreference,
    debug_device: gpui::DebugDevice,
) -> Result<(
    IDXGIAdapter1,
    ID3D11Device,
    ID3D11DeviceContext,
    D3D_FEATURE_LEVEL,
)> {
    // KaminIDE patch: leave the entire default creation path unchanged.
    if *preference != crate::GpuAdapterPreference::Default {
        let adapter = crate::gpu_adapter::selected_adapter(dxgi_factory, preference)?;
        let mut context = None;
        let mut feature_level = D3D_FEATURE_LEVEL::default();
        let device = get_device(&adapter, Some(&mut context), Some(&mut feature_level),
            debug_layer_available).context("Creating device on the requested GPU")?;
        return Ok((adapter, device, context.context("Missing D3D device context")?, feature_level));
    }
    // В release сохраняется обычный выбор адаптера.
    let debug_device = if cfg!(debug_assertions) {
        debug_device
    } else {
        gpui::DebugDevice::Default
    };
    match debug_device {
        gpui::DebugDevice::Default => {}
        gpui::DebugDevice::Warp => {
            let adapter: IDXGIAdapter1 = unsafe { dxgi_factory.EnumWarpAdapter()? };
            log::info!("Debug acceptance override: Direct3D device on the WARP adapter");
            let mut context: Option<ID3D11DeviceContext> = None;
            let mut feature_level = D3D_FEATURE_LEVEL::default();
            let device = get_device(
                &adapter,
                Some(&mut context),
                Some(&mut feature_level),
                debug_layer_available,
            )?;
            return Ok((adapter, device, context.unwrap(), feature_level));
        }
        gpui::DebugDevice::D3D11On12 => {
            let adapter: IDXGIAdapter1 = unsafe { dxgi_factory.EnumAdapters(0)?.cast()? };
            log::info!("Debug acceptance override: D3D11On12 device on adapter 0");
            let (device, context, feature_level) =
                get_d3d11on12_device(&adapter, debug_layer_available)?;
            return Ok((adapter, device, context, feature_level));
        }
    }
    for adapter_index in 0.. {
        let adapter: IDXGIAdapter1 = unsafe { dxgi_factory.EnumAdapters(adapter_index)?.cast()? };
        if let Ok(desc) = unsafe { adapter.GetDesc1() } {
            let gpu_name = String::from_utf16_lossy(&desc.Description)
                .trim_matches(char::from(0))
                .to_string();
            log::info!("Using GPU: {}", gpu_name);
        }
        // Check to see whether the adapter supports Direct3D 11 and create
        // the device if it does.
        let mut context: Option<ID3D11DeviceContext> = None;
        let mut feature_level = D3D_FEATURE_LEVEL::default();
        if let Some(device) = get_device(
            &adapter,
            Some(&mut context),
            Some(&mut feature_level),
            debug_layer_available,
        )
        .log_err()
        {
            return Ok((adapter, device, context.unwrap(), feature_level));
        }
    }

    unreachable!()
}
