//! KaminIDE patch: типизированные настройки и измерения, принадлежащие приложению.

use crate::{batch_cost::BatchCost, prepaint_prof::PrepaintProfile};
use std::sync::Arc;

/// Выбор устройства для native acceptance; release использует Default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DebugDevice {
    /// Обычное перечисление адаптеров.
    #[default]
    Default,
    /// Программный адаптер WARP.
    Warp,
    /// D3D11On12 с поддержкой принудительного device removal.
    D3D11On12,
}

/// Настройки задаёт host до запуска приложения, без чтения окружения toolkit.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct InstrumentationConfig {
    /// Измерять self-time prepaint по типам элементов.
    pub prepaint: bool,
    /// Создавать timestamp-запросы GPU.
    pub gpu_timing: bool,
    /// Подменить DPI монитора для воспроизводимого raster capture.
    pub scale_factor: Option<f32>,
    /// Отладочное устройство; backend игнорирует override в release.
    pub debug_device: DebugDevice,
}

/// Явный общий экземпляр измерений приложения и его окон.
#[derive(Clone)]
pub struct Instrumentation {
    config: InstrumentationConfig,
    /// Счётчики пачек, включая завершённые GPU timestamps.
    pub batches: Arc<BatchCost>,
    /// Профиль элементов UI-потока.
    pub prepaint: PrepaintProfile,
}

impl Default for Instrumentation {
    fn default() -> Self {
        Self::new(InstrumentationConfig::default())
    }
}

impl Instrumentation {
    /// Создать независимый экземпляр; clone разделяет только его измерения.
    pub fn new(config: InstrumentationConfig) -> Self {
        Self {
            config,
            batches: Arc::default(),
            prepaint: PrepaintProfile::new(config.prepaint),
        }
    }

    /// Снимок неизменяемых настроек.
    pub fn config(&self) -> InstrumentationConfig {
        self.config
    }
}
