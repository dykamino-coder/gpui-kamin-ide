//! Нейтральные значения картинки: параметры декодирования, CPU-растр с явным форматом
//! точек, неизменяемый рисунок SVG, метаданные и типизированные ошибки. Ни GPUI, ни
//! фаз конвейера: эти значения читают и раскладка (размер, альфа), и покраска (точки).

use super::natural::Intrinsic;
use super::vector::VectorImage;
use std::sync::Arc;

/// Применять ли EXIF-ориентацию (css-images-3 §5.4 `image-orientation`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum OrientationMode {
    /// `from-image`: растр разворачивается по метке ДО вычисления природного размера.
    FromImage,
    /// `none`: точки как записаны в файле.
    None,
}

/// Что делать со вшитым цветовым профилем (css-color-4 §12, tagged images).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ColourPolicy {
    /// Перевести точки в sRGB, если профиль понятен (матричный RGB).
    ConvertToSrgb,
    /// Оставить точки в пространстве файла (профиль описан в метаданных).
    Preserve,
}

/// Параметры декодирования: всё, кроме байт, от чего зависят полученные точки.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct DecodeOptions {
    pub(crate) orientation: OrientationMode,
    pub(crate) colour: ColourPolicy,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            orientation: OrientationMode::FromImage,
            colour: ColourPolicy::ConvertToSrgb,
        }
    }
}

/// Порядок каналов точки растра.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum PixelFormat {
    /// Четыре байта на точку: R, G, B, A.
    Rgba8,
    /// Четыре байта на точку: B, G, R, A.
    Bgra8,
}

/// Как альфа связана с цветом.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AlphaMode {
    /// Цвет не умножен на альфу (так его отдают кодеки).
    Straight,
    /// Цвет умножен на альфу (выход resvg).
    Premultiplied,
}

/// В каком пространстве записаны цвета точек.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ColourSpace {
    Srgb,
    /// Пространство вшитого профиля, не переведённое (`ColourPolicy::Preserve` или
    /// профиль, который не удалось понять).
    Embedded(Arc<[u8]>),
}

/// Готовые точки картинки в памяти процесса. Неизменяемы после декодирования.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CpuRaster {
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Байт в строке точек.
    pub(crate) stride: usize,
    pub(crate) format: PixelFormat,
    pub(crate) alpha: AlphaMode,
    pub(crate) colour_space: ColourSpace,
    pub(crate) bytes: Arc<[u8]>,
}

impl CpuRaster {
    /// Точка `(x, y)` четырьмя байтами формата растра.
    pub(crate) fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        let at = y as usize * self.stride + x as usize * 4;
        (x < self.width && y < self.height).then(|| {
            let p = &self.bytes[at..at + 4];
            [p[0], p[1], p[2], p[3]]
        })
    }
}

/// Результат декодирования: точки растра или разметка рисунка, которую растрируют под
/// размер места (у рисунка своих точек нет).
#[derive(Clone, Debug)]
pub(crate) enum DecodedImage {
    Raster(Arc<CpuRaster>),
    Vector(Arc<VectorImage>),
}

impl DecodedImage {
    /// Природный размер (css-images-3 §4.1) после ориентации.
    pub(crate) fn natural(&self) -> Intrinsic {
        match self {
            DecodedImage::Raster(r) => {
                // Как у покраски фона: сторона растра не меньше точки.
                let (w, h) = ((r.width as f32).max(1.0), (r.height as f32).max(1.0));
                Intrinsic {
                    w: Some(w),
                    h: Some(h),
                    ratio: Some(w / h),
                }
            }
            DecodedImage::Vector(v) => v.natural,
        }
    }

    /// Байты, которые держит значение (точки или разметка).
    pub(crate) fn held_bytes(&self) -> usize {
        match self {
            DecodedImage::Raster(r) => r.bytes.len(),
            DecodedImage::Vector(v) => v.markup.len(),
        }
    }
}

/// Факты о картинке, доступные раскладке без точек.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ImageMetadata {
    pub(crate) natural: Intrinsic,
    /// Метка EXIF `Orientation`, которая была применена (`None` — не применялась).
    pub(crate) applied_orientation: Option<u16>,
    pub(crate) vector: bool,
}

/// Почему картинки нет.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ImageError {
    /// Бэкенда загрузки для схемы нет.
    UnsupportedScheme(String),
    /// Политика документа запрещает этот адрес.
    AccessDenied,
    /// Байты не получены (нет файла, ошибка чтения, битый `data:`).
    Io,
    /// Байты не являются картинкой известного формата или текст SVG не UTF-8.
    InvalidData,
    /// Кодек распознал формат, но данные повреждены.
    Decode,
    /// Размеры за пределами ограничений кодека.
    LimitExceeded,
}
