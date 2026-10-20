//! Stable paragraph identity for selection memory.

/// Устойчивый номер абзаца для памяти выделения.
///
/// Абзац не элемент документа, своего номера у него нет; берём отпечаток его
/// текста — от кадра к кадру он не меняется, а разные абзацы почти всегда
/// различаются.
pub(crate) fn text_id(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}
