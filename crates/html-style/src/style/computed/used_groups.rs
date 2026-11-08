//! Правка used-стадии одной reset-группой через выведенные группы документа
//! (`DerivedGroups`): соседи с одной общей группой и одной правкой получают одну группу.

use super::*;

/// Reset-группа `T` стиля: адресует группу, которую заменяет [`Computed::derive_used`].
pub trait ResetGroup<T> {
    fn reset_group(&mut self) -> &mut StyleGroup<T>;
}

macro_rules! reset_groups {
    ($($field:ident: $group:ty),* $(,)?) => {$(
        impl ResetGroup<$group> for Computed {
            fn reset_group(&mut self) -> &mut StyleGroup<$group> {
                &mut self.$field
            }
        }
    )*};
}

reset_groups!(
    reset_text_box: ResetTextBox,
    reset_cascade: ResetCascade,
    reset_box_layout: ResetBoxLayout,
    reset_flex: ResetFlex,
    reset_border: ResetBorder,
    reset_grid: ResetGrid,
    reset_context: ResetContext,
    reset_generated: ResetGenerated,
    reset_svg_math: ResetSvgMath,
    reset_positioned: ResetPositioned,
    reset_effects: ResetEffects,
    reset_background: ResetBackground,
    reset_mask: ResetMask,
    reset_shapes: ResetShapes,
    reset_fragmentation: ResetFragmentation,
    reset_transforms: ResetTransforms,
);

impl Computed {
    /// Заменить reset-группу `T` её правкой `edit` вида `E` со значением `value` из кэша
    /// документа: правка зависит только от исходной группы и `value`.
    pub fn derive_used<E: 'static, T: Clone + 'static>(
        &mut self,
        derived: &DerivedGroups,
        value: u64,
        edit: impl FnOnce(&mut T),
    ) where
        Self: ResetGroup<T>,
    {
        let group = self.reset_group();
        *group = derived.derive::<E, T>(group, value, edit);
    }
}
