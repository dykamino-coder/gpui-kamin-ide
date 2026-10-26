//! Использованный размер конкретного лица: отдельные стадии CSS Fonts 5 §3.4.

use super::SelectedFaceFacts;

/// Результат подгонки лица; не наследуется и не заменяет computed font-size.
#[derive(Clone, Debug)]
pub struct UsedFont {
    pub selected: Option<SelectedFaceFacts>,
    pub computed_px: f32,
    /// Дескриптор масштабирует контуры и метрики до свойства font-size-adjust.
    pub descriptor_scale: f32,
    /// Свойство нормализует уже масштабированную метрику лица.
    pub property_scale: f32,
    pub size_px: f32,
}

impl UsedFont {
    pub(crate) fn new(
        selected: Option<SelectedFaceFacts>,
        computed_px: f32,
        descriptor_scale: f32,
        adjustment: Option<(f32, f32)>,
    ) -> Self {
        let property_scale = adjustment
            .filter(|(want, have)| want.is_finite() && *have > 0.0)
            .map_or(1.0, |(want, have)| want / have);
        Self {
            selected,
            computed_px,
            descriptor_scale,
            property_scale,
            size_px: computed_px * descriptor_scale * property_scale,
        }
    }
    /// Сначала контуры масштабируются дескриптором и свойством; затем
    /// проценты overrides заменяют метрики от того же использованного em.
    pub(crate) fn override_metrics(
        size: f32,
        native: [f32; 3],
        overrides: [Option<f32>; 3],
    ) -> [f32; 3] {
        std::array::from_fn(|i| overrides[i].map_or(native[i], |ratio| ratio * size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_normalizes_descriptor_metrics_once() {
        let used = UsedFont::new(None, 20.0, 2.0, Some((0.5, 0.8)));
        assert_eq!(used.computed_px, 20.0);
        assert_eq!(used.property_scale, 0.625);
        assert_eq!(used.size_px, 25.0);
        assert_eq!(UsedFont::new(None, 0.0, 2.0, Some((0.5, 0.8))).size_px, 0.0);
    }
}
