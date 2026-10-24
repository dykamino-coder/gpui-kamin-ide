//! Стадия вычисленных значений: цвет (с функциями от `currentcolor`), единицы `lh`/`rlh`, каскадное
//! `inherit` у `box-shadow`.

use super::*;
use crate::document::state::ValueCx;
mod lh_units;
use lh_units::inherit_lh_units;

pub(super) fn compute_sizes(parent: &Computed, own: &Computed, c: &mut Computed, vcx: ValueCx<'_>) {
    // `color` с функцией от `currentcolor` вычисляется от цвета родителя
    // (css-color-4 §6.4): дети наследуют уже абсолютный цвет.
    c.color = match &own.color_function {
        Some(function) => Some(
            function.resolve(
                parent
                    .color
                    .unwrap_or(crate::style::values::value::Color::BLACK),
            ),
        ),
        None => own.color.or(parent.color),
    };
    // Единица `lh` разрешается ЗДЕСЬ: высота строки известна после каскада.
    inherit_lh_units(parent, c, vcx);
    // `box-shadow: inherit` копирует ВЫЧИСЛЕННУЮ тень родителя — вместе с
    // нерешённым `currentcolor`: решает его отрисовка цветом СВОЕГО элемента.
    if own.shadow_inherit {
        c.shadows = parent.shadows.clone();
        c.inset_shadows = parent.inset_shadows.clone();
    }
}
