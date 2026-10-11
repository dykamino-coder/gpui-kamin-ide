//! KaminIDE debug-only acceptance device override.
//!
//! Native acceptance must exercise WARP and real device removal without any
//! system-wide DirectX setting (no D3DConfig). Debug builds may therefore ask
//! GPUI to create its own D3D11 device on the WARP adapter, or as a D3D11On12
//! device whose D3D12 device the application can remove itself. Release builds
//! never read the environment: the reader below is compiled out and tested.
//! This file has no crate dependencies so CI can compile it alone with
//! `-C debug-assertions=no`.

/// Environment switch read only by debug builds.
pub(crate) const ENV: &str = "KAMIN_DEBUG_D3D_DEVICE";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(not(debug_assertions), allow(dead_code))]
pub(crate) enum DeviceOverride {
    /// Regular adapter enumeration (the only release behavior).
    #[default]
    None,
    /// D3D11 device on the factory's WARP adapter (`EnumWarpAdapter`).
    Warp,
    /// D3D11On12 device over a D3D12 device on the first hardware adapter, so
    /// `ID3D12Device5::RemoveDevice` produces a genuine device-removed state.
    D3D11On12,
}

impl DeviceOverride {
    pub(crate) fn from_env() -> Self {
        Self::read(|name| std::env::var(name).ok())
    }

    fn read(_env: impl Fn(&str) -> Option<String>) -> Self {
        #[cfg(debug_assertions)]
        return match _env(ENV).as_deref() {
            Some("warp") => Self::Warp,
            Some("d3d11on12") => Self::D3D11On12,
            _ => Self::None,
        };
        #[cfg(not(debug_assertions))]
        Self::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_never_reads_device_override() {
        let reads = std::cell::Cell::new(0);
        let read = |value: &'static str| {
            DeviceOverride::read(|name| {
                assert_eq!(name, ENV);
                reads.set(reads.get() + 1);
                Some(value.into())
            })
        };
        let warp = read("warp");
        let on12 = read("d3d11on12");
        let other = read("WARP");
        assert_eq!(reads.get(), if cfg!(debug_assertions) { 3 } else { 0 });
        if cfg!(debug_assertions) {
            assert_eq!(warp, DeviceOverride::Warp);
            assert_eq!(on12, DeviceOverride::D3D11On12);
        } else {
            assert_eq!(warp, DeviceOverride::None);
            assert_eq!(on12, DeviceOverride::None);
        }
        assert_eq!(other, DeviceOverride::None);
        assert_eq!(DeviceOverride::read(|_| None), DeviceOverride::None);
        let _ = DeviceOverride::from_env();
    }
}
