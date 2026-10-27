//! Чтение отладочного выбора устройства на границе host.
//!
//! Native acceptance проверяет WARP и настоящее удаление устройства без
//! системных настроек DirectX. Debug-сборка может запросить WARP либо D3D11On12,
//! чей D3D12 device приложение удаляет самостоятельно. Release-сборка не читает
//! окружение: этот путь исключён при компиляции и проверяется тестом.
//! Файл не зависит от крейтов, поэтому CI компилирует его отдельно с
//! `-C debug-assertions=no`.

/// Переключатель окружения, который читают только debug-сборки.
#[cfg(any(debug_assertions, test))]
pub(crate) const ENV: &str = "KAMIN_DEBUG_D3D_DEVICE";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(not(debug_assertions), allow(dead_code))]
pub(crate) enum DeviceOverride {
    /// Обычное перечисление адаптеров; единственный вариант в release.
    #[default]
    None,
    /// Устройство D3D11 на WARP-адаптере фабрики (`EnumWarpAdapter`).
    Warp,
    /// D3D11On12 на первом аппаратном адаптере: `ID3D12Device5::RemoveDevice`
    /// переводит его в настоящее состояние device-removed.
    D3D11On12,
}

impl DeviceOverride {
    #[allow(
        clippy::disallowed_methods,
        reason = "host launch reads process configuration"
    )]
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
