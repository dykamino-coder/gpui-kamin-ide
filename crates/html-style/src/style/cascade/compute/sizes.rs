//! Стадия вычисленных значений: цвет (с функциями от `currentcolor`), единицы `lh`/`rlh`, каскадное
//! `inherit` у `box-shadow`.

use super::*;
use crate::values::ValueCx;
mod lh_units;
use lh_units::inherit_lh_units;

pub(super) fn compute_sizes(parent: &Computed, own: &Computed, c: &mut Computed, vcx: ValueCx<'_>) {
    // `color` с функцией от `currentcolor` вычисляется от цвета родителя
    // (css-color-4 §6.4): дети наследуют уже абсолютный цвет.
    c.inh.ui.color = match &own.reset_cascade.color_function {
        Some(function) => Some(
            function.resolve(
                parent
                    .inh
                    .ui
                    .color
                    .unwrap_or(crate::style::values::value::Color::BLACK),
            ),
        ),
        None => own.inh.ui.color.or(parent.inh.ui.color),
    };
    // Единица `lh` разрешается ЗДЕСЬ: высота строки известна после каскада.
    inherit_lh_units(parent, c, vcx);
    // `box-shadow: inherit` копирует ВЫЧИСЛЕННУЮ тень родителя — вместе с
    // нерешённым `currentcolor`: решает его отрисовка цветом СВОЕГО элемента.
    if own.reset_effects.shadow_inherit {
        c.reset_effects.shadows = parent.reset_effects.shadows.clone();
        c.reset_effects.inset_shadows = parent.reset_effects.inset_shadows.clone();
    }
}
