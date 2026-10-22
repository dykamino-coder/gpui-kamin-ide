//! CSS-фон корня SVG; декодирование изображений выполняет сервис ресурсов.
mod svg_metadata;
#[cfg(test)]
pub(super) use crate::resources::images::vector::svg_size;
#[cfg(test)]
pub(super) use crate::resources::loader::data_url::base64_decode;
pub(crate) use svg_metadata::svg_root_background;
