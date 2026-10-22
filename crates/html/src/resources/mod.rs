//! Ресурсы документа: нейтральные байты и картинки, затем CSS-операнды.
//!
//! `loader` и `images` не импортируют фазы конвейера или GPUI. `images` владеет
//! CPU-декодированием, метаданными и SVG-растеризацией с внешней базой шрифтов.
//! `css_images` разрешает вычисленные CSS-источники через контекст документа:
//! зависит от style, но не от layout, paint, render или GPUI.

pub(crate) mod css_images;
pub(crate) mod images;
pub(crate) mod loader;
