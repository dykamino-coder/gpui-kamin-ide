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
    /// Взять все группы `other` вместо своих; собственные поля элемента остаются.
    pub(crate) fn share_groups_from(&mut self, other: &Computed) {
        self.inh = other.inh.clone();
        self.reset_text_box = other.reset_text_box.clone();
        self.reset_cascade = other.reset_cascade.clone();
        self.reset_box_layout = other.reset_box_layout.clone();
        self.reset_flex = other.reset_flex.clone();
        self.reset_border = other.reset_border.clone();
        self.reset_grid = other.reset_grid.clone();
        self.reset_context = other.reset_context.clone();
        self.reset_generated = other.reset_generated.clone();
        self.reset_svg_math = other.reset_svg_math.clone();
        self.reset_positioned = other.reset_positioned.clone();
        self.reset_effects = other.reset_effects.clone();
        self.reset_background = other.reset_background.clone();
        self.reset_mask = other.reset_mask.clone();
        self.reset_shapes = other.reset_shapes.clone();
        self.reset_fragmentation = other.reset_fragmentation.clone();
        self.reset_transforms = other.reset_transforms.clone();
    }

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
