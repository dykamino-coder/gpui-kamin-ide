//! KaminIDE patch: явный выбор DXGI-адаптера и сведения о реальном устройстве.

use anyhow::{Context, Result, bail};
use windows::{
    Win32::Graphics::Dxgi::{
        CreateDXGIFactory2, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_CREATE_FACTORY_FLAGS, DXGI_ERROR_NOT_FOUND,
        DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE, DXGI_GPU_PREFERENCE_MINIMUM_POWER,
        IDXGIAdapter1, IDXGIDevice, IDXGIFactory6,
    },
    core::Interface,
};

/// Device creation policy. `Default` retains the original EnumAdapters order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum GpuAdapterPreference {
    #[default]
    Default,
    HighPerformance,
    MinimumPower,
    ByName(String),
    ByLuid { high_part: i32, low_part: u32 },
}

impl GpuAdapterPreference {
    /// Reserved power policies or a case-insensitive device-name substring.
    pub fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "default" => Self::Default,
            "high-performance" => Self::HighPerformance,
            "minimum-power" => Self::MinimumPower,
            value if !value.trim().is_empty() => Self::ByName(value.to_lowercase()),
            _ => bail!("GPU name substring must not be empty"),
        })
    }
}

/// Identity queried from an actual DXGI adapter, rather than the requested policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GpuAdapterInfo {
    pub device_name: String,
    pub luid_high_part: i32,
    pub luid_low_part: u32,
    pub driver_version: String,
    pub vendor_id: u32,
    pub device_id: u32,
}

impl GpuAdapterInfo {
    pub(crate) fn read(adapter: &IDXGIAdapter1) -> Result<Self> {
        // The COM adapter is alive; both calls return owned values.
        let desc = unsafe { adapter.GetDesc1()? };
        Ok(Self {
            device_name: String::from_utf16_lossy(&desc.Description)
                .trim_end_matches('\0').to_owned(),
            luid_high_part: desc.AdapterLuid.HighPart,
            luid_low_part: desc.AdapterLuid.LowPart,
            driver_version: driver_version(adapter)?,
            vendor_id: desc.VendorId,
            device_id: desc.DeviceId,
        })
    }
}

/// Enumerates hardware identities without creating an application or D3D device.
pub fn gpu_adapters(preference: &GpuAdapterPreference) -> Result<Vec<GpuAdapterInfo>> {
    let factory: IDXGIFactory6 = unsafe { CreateDXGIFactory2(DXGI_CREATE_FACTORY_FLAGS::default())? };
    adapters(&factory, preference)?.iter()
        .filter(|adapter| unsafe { adapter.GetDesc1().is_ok_and(|desc| desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 == 0) })
        .map(GpuAdapterInfo::read).collect()
}

pub(crate) fn adapters(
    factory: &IDXGIFactory6,
    preference: &GpuAdapterPreference,
) -> Result<Vec<IDXGIAdapter1>> {
    let power = match preference {
        GpuAdapterPreference::HighPerformance => Some(DXGI_GPU_PREFERENCE_HIGH_PERFORMANCE),
        GpuAdapterPreference::MinimumPower => Some(DXGI_GPU_PREFERENCE_MINIMUM_POWER),
        _ => None,
    };
    let mut found = Vec::new();
    for index in 0.. {
        let adapter = if let Some(power) = power {
            unsafe { factory.EnumAdapterByGpuPreference::<IDXGIAdapter1>(index, power) }
        } else {
            unsafe { factory.EnumAdapters(index).and_then(|adapter| adapter.cast()) }
        };
        match adapter {
            Ok(adapter) => found.push(adapter),
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(error) if power.is_some() => {
                log::warn!("DXGI GPU preference enumeration failed ({error}); using EnumAdapters");
                return adapters(factory, &GpuAdapterPreference::Default);
            }
            Err(error) => return Err(error.into()),
        }
    }
    if matches!(preference, GpuAdapterPreference::ByName(_) | GpuAdapterPreference::ByLuid { .. }) {
        let mut matching = Vec::new();
        for adapter in found {
            let desc = unsafe { adapter.GetDesc1()? };
            let matches = match preference {
                GpuAdapterPreference::ByName(name) => String::from_utf16_lossy(&desc.Description)
                    .to_lowercase().contains(&name.to_lowercase()),
                GpuAdapterPreference::ByLuid { high_part, low_part } =>
                    desc.AdapterLuid.HighPart == *high_part && desc.AdapterLuid.LowPart == *low_part,
                _ => unreachable!(),
            };
            if matches { matching.push(adapter); }
        }
        if matching.len() != 1 {
            bail!("GPU selector {preference:?} matched {} adapters; expected exactly one", matching.len());
        }
        return Ok(matching);
    }
    Ok(found)
}

pub(crate) fn selected_adapter(
    factory: &IDXGIFactory6,
    preference: &GpuAdapterPreference,
) -> Result<IDXGIAdapter1> {
    adapters(factory, preference)?.into_iter().next().context("No matching DXGI adapter")
}


// KaminIDE patch: shared DXGI driver identity, moved unchanged from the renderer.
pub(crate) fn driver_version(adapter: &IDXGIAdapter1) -> anyhow::Result<String> {
    let number = unsafe { adapter.CheckInterfaceSupport(&IDXGIDevice::IID as _) }?;
    Ok(format!(
        "{}.{}.{}.{}",
        number >> 48,
        (number >> 32) & 0xFFFF,
        (number >> 16) & 0xFFFF,
        number & 0xFFFF
    ))
}
