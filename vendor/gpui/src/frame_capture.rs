//! KaminIDE patch: оконный получатель завершённого BGRA readback.
//!
//! Backend владеет единственным sink. Подписки, sequence и выбор кадра
//! принадлежат host; production offscreen scene не зависит от этого hook.

use std::time::Duration;

/// Завершённая копия клиентского GPU-кадра; строки BGRA идут сверху вниз.
#[derive(Clone)]
pub struct CapturedFrame {
    /// Ширина в device pixels.
    pub width: u32,
    /// Высота в device pixels.
    pub height: u32,
    /// Четыре байта BGRA на пиксель, без padding строк.
    pub bgra: Vec<u8>,
    /// Снимок устройства и цели в момент readback; тестовая платформа может его не иметь.
    pub context: Option<FrameContext>,
}

/// KaminIDE patch: идентичность запрашивается у самого D3D device, а не у policy.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GpuDeviceContext {
    /// Старшие знаковые биты локального идентификатора адаптера.
    pub luid_high_part: i32,
    /// Младшие биты локального идентификатора адаптера.
    pub luid_low_part: u32,
    /// Описание фактического адаптера устройства.
    pub device_name: String,
    /// PCI vendor ID.
    pub vendor_id: u32,
    /// PCI device ID.
    pub device_id: u32,
    /// PCI subsystem ID.
    pub subsystem_id: u32,
    /// Ревизия адаптера.
    pub revision: u32,
    /// Флаги DXGI adapter description.
    pub adapter_flags: u32,
    /// Версия UMD из CheckInterfaceSupport.
    pub driver_version: String,
    /// Фактический D3D feature level.
    pub feature_level: u32,
    /// Фактические флаги GetCreationFlags.
    pub creation_flags: u32,
    /// Фабрика DXGI создана с debug flag.
    pub factory_debug: bool,
    /// Backend создания устройства.
    pub backend: String,
    /// Монотонный номер устройства в процессе.
    pub generation: u64,
    /// Идентичность прочитана у устройства после создания.
    pub queried_from_device: bool,
}

/// Контекст принадлежит завершённому кадру и не меняется после device recovery.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FrameContext {
    /// Устройство, нарисовавшее этот кадр.
    pub device: GpuDeviceContext,
    /// DXGI format исходной render target.
    pub target_format: i32,
    /// Число samples исходной render target.
    pub sample_count: u32,
    /// Качество multisampling исходной render target.
    pub sample_quality: u32,
    /// SHA-256 исполняемых DXBC chunks используемых pipelines.
    pub shader_executables: Vec<String>,
    /// Gamma constants фактического text pipeline.
    pub gamma_ratios: [f32; 4],
    /// Contrast constant фактического grayscale pipeline.
    pub grayscale_enhanced_contrast: f32,
}

/// Эксклюзивный оконный endpoint, перемещаемый в backend без клонирования.
/// Drop endpoint означает завершение/замену capture для этого окна.
pub trait FrameCaptureSink {
    /// Доставить завершённый readback и его длительность.
    fn completed(&mut self, frame: CapturedFrame, readback: Duration);
}
