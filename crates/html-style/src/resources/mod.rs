//! Ресурсы документа: нейтральные байты и картинки, затем CSS-операнды.
//!
//! `loader` и `images` не импортируют фазы конвейера или GPUI. `images` владеет
//! CPU-декодированием, метаданными и SVG-растеризацией с внешней базой шрифтов.
//! `css_images` разрешает вычисленные CSS-источники через контекст документа:
//! зависит от style, но не от layout, paint, render или GPUI.

pub mod css_images;
pub mod images;
pub mod loader;
pub mod source;
pub mod svg_document;
pub mod url_loader;
