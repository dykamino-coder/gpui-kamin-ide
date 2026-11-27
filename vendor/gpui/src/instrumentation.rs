//! KaminIDE patch: типизированные настройки и измерения, принадлежащие приложению.

use crate::{batch_cost::BatchCost, prepaint_prof::ElementProfile};
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
    /// Measure the self-time of request_layout by element type and the
    /// layout inputs boxes keep across passes (`Instrumentation::layout_inputs`).
    pub request_layout: bool,
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
    pub prepaint: ElementProfile,
    /// Self-time of request_layout by element type.
    pub request_layout: ElementProfile,
    /// Frame-tree boxes laid out with the inputs of the previous pass; kept
    /// with `request_layout` (the layout journal).
    pub layout_inputs: Arc<LayoutInputs>,
}

/// Boxes of the window's frame tree by whether their layout inputs (definite
/// sizes, available space, percentage base, sizing mode) are those of the same
/// box in the previous layout pass (`TaffyLayoutEngine::clear`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutInputCounts {
    /// Layout passes noted.
    pub passes: u64,
    /// Boxes with a full layout in their pass.
    pub boxes: u64,
    /// Of them, boxes whose inputs are those of the previous pass.
    pub same: u64,
    /// Of them, boxes whose whole subtree kept its inputs: what skipping
    /// unchanged subtrees whole would not lay out again.
    pub whole: u64,
    /// The largest such subtrees (their parent changed): how many skips.
    pub skips: u64,
}

/// Counts of [`LayoutInputCounts`] since the last `take`; disabled, nothing is walked.
#[derive(Debug, Default)]
pub struct LayoutInputs {
    enabled: bool,
    counts: std::sync::Mutex<LayoutInputCounts>,
}

impl LayoutInputs {
    /// Whether layout passes note their inputs.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn add(&self, pass: LayoutInputCounts) {
        if let Ok(mut counts) = self.counts.lock() {
            counts.passes += pass.passes;
            counts.boxes += pass.boxes;
            counts.same += pass.same;
            counts.whole += pass.whole;
            counts.skips += pass.skips;
        }
    }

    /// The counts since the last call.
    pub fn take(&self) -> LayoutInputCounts {
        self.counts
            .lock()
            .map(|mut counts| std::mem::take(&mut *counts))
            .unwrap_or_default()
    }
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
            prepaint: ElementProfile::new(config.prepaint),
            request_layout: ElementProfile::new(config.request_layout),
            layout_inputs: Arc::new(LayoutInputs {
                enabled: config.request_layout,
                ..Default::default()
            }),
        }
    }

    /// Снимок неизменяемых настроек.
    pub fn config(&self) -> InstrumentationConfig {
        self.config
    }
}
