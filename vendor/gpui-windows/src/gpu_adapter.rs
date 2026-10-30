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
            value if value.starts_with("luid:") => {
                let (high, low) = value[5..].split_once(':').context("Expected luid:HHHHHHHH:LLLLLLLL")?;
                Self::ByLuid {
                    high_part: u32::from_str_radix(high, 16).context("Invalid LUID high part")? as i32,
                    low_part: u32::from_str_radix(low, 16).context("Invalid LUID low part")?,
                }
            }
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
    adapters(&factory, preference)?.iter().map(GpuAdapterInfo::read).collect()
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
    let enumerated = enumerate(|index| {
        if let Some(power) = power {
            unsafe { factory.EnumAdapterByGpuPreference::<IDXGIAdapter1>(index, power) }
        } else {
            unsafe { factory.EnumAdapters(index).and_then(|adapter| adapter.cast()) }
        }
    })?;
    let mut found = Vec::new();
    for adapter in enumerated {
        let desc = unsafe { adapter.GetDesc1()? };
        if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 == 0 { found.push(adapter); }
    }
    if matches!(preference, GpuAdapterPreference::ByName(_) | GpuAdapterPreference::ByLuid { .. }) {
        let mut matching = Vec::new();
        for adapter in found {
            let matches = matches_identity(preference, &GpuAdapterInfo::read(&adapter)?);
            if matches { matching.push(adapter); }
        }
        require_unique(preference, matching.len())?;
        return Ok(matching);
    }
    Ok(found)
}

// KaminIDE patch: только NOT_FOUND завершает список; ошибка power enumeration не включает fallback.
fn enumerate<T>(mut next: impl FnMut(u32) -> windows::core::Result<T>) -> Result<Vec<T>> {
    let mut found = Vec::new();
    for index in 0.. {
        match next(index) {
            Ok(adapter) => found.push(adapter),
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => return Ok(found),
            Err(error) => return Err(error.into()),
        }
    }
    unreachable!("DXGI adapter index exhausted")
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

fn matches_identity(preference: &GpuAdapterPreference, info: &GpuAdapterInfo) -> bool {
    match preference {
        GpuAdapterPreference::ByName(name) => info.device_name.to_lowercase().contains(&name.to_lowercase()),
        GpuAdapterPreference::ByLuid { high_part, low_part } => info.luid_high_part == *high_part && info.luid_low_part == *low_part,
        _ => false,
    }
}

fn require_unique(preference: &GpuAdapterPreference, count: usize) -> Result<()> {
    if count != 1 { bail!("GPU selector {preference:?} matched {count} adapters; expected exactly one"); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn enumeration_errors_do_not_become_partial_success_or_fallback() {
        let mut calls = 0;
        let result = enumerate(|index| {
            calls += 1;
            if index == 0 { Ok(42) } else { Err(windows::core::Error::from_hresult(windows::Win32::Foundation::E_FAIL)) }
        });
        assert!(result.is_err());
        assert_eq!(calls, 2);
        let result = enumerate::<u32>(|_| Err(windows::core::Error::from_hresult(DXGI_ERROR_NOT_FOUND)));
        assert!(result.unwrap().is_empty());
    }
    #[test]
    fn selectors_are_exact_and_ambiguous_names_fail() {
        let info = GpuAdapterInfo { device_name: "Intel GPU".into(), luid_high_part: -1,
            luid_low_part: 42, driver_version: "1".into(), vendor_id: 1, device_id: 2 };
        assert!(matches_identity(&GpuAdapterPreference::ByName("INTEL".into()), &info));
        assert!(matches_identity(&GpuAdapterPreference::ByLuid { high_part: -1, low_part: 42 }, &info));
        assert!(!matches_identity(&GpuAdapterPreference::ByLuid { high_part: 0, low_part: 42 }, &info));
        for count in [0, 2] { assert!(require_unique(&GpuAdapterPreference::ByName("GPU".into()), count).is_err()); }
        assert!(require_unique(&GpuAdapterPreference::MinimumPower, 1).is_ok());
    }
}
