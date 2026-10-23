//! Стадия вычисленных значений: цвет (с функциями от `currentcolor`), единицы `lh`/`rlh`, каскадное
//! `inherit` у теней, рамок, отступов и выравниваний.

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
    // `inherit` по сторонам и по частям рамки (`border: inherit` раскрыт
    // разбором в длинные свойства): вычисленное значение родителя.
    for i in 0..4 {
        if own.border_inherit_w[i] {
            match i {
                0 => c.border_width.top = parent.border_width.top,
                1 => c.border_width.right = parent.border_width.right,
                2 => c.border_width.bottom = parent.border_width.bottom,
                _ => c.border_width.left = parent.border_width.left,
            }
        }
        if own.border_inherit_s[i] {
            c.border_visible[i] = parent.border_visible[i];
            c.border_side_styles[i] = parent.border_side_styles[i];
            c.sync_border_pattern();
        }
        if own.border_inherit_c[i] {
            // Начальное значение `border-color` — `currentColor`, и по
            // css-color-4 §6.4 наследуется оно КЛЮЧЕВЫМ СЛОВОМ: решает его
            // цвет РЕБЁНКА (`border-color-011`).
            c.border_colors[i] = parent.border_colors[i].clone();
        }
    }
    if own.padding_inherit {
        c.padding = parent.padding;
    }
    // `inherit` у ненаследуемых выравниваний (css-cascade-4 §7.3.1): значение
    // родителя целиком, с его `safe`/`last` (`place-items: inherit` во
    // вложенной сетке — `grid-self-alignment-baseline-with-grid-001`).
    if own.align_inherit != 0 {
        use crate::style::computed::ainh;
        let on = |b: u8| own.align_inherit & b != 0;
        if on(ainh::ALIGN_ITEMS) {
            c.align_items = parent.align_items;
            c.align_items_safe = parent.align_items_safe;
            c.align_items_last = parent.align_items_last;
        }
        if on(ainh::JUSTIFY_ITEMS) {
            c.justify_items = parent.justify_items;
            c.justify_items_safe = parent.justify_items_safe;
            c.justify_items_last = parent.justify_items_last;
        }
        if on(ainh::ALIGN_CONTENT) {
            c.align_content = parent.align_content;
            c.align_content_baseline = parent.align_content_baseline;
            c.align_content_safe = parent.align_content_safe;
            c.align_content_unsafe = parent.align_content_unsafe;
            c.align_content_block = parent.align_content_block;
        }
        if on(ainh::JUSTIFY_CONTENT) {
            c.justify_content = parent.justify_content;
            c.justify_content_safe = parent.justify_content_safe;
        }
        if on(ainh::JUSTIFY_SELF) {
            c.justify_self = parent.justify_self;
            c.justify_self_safe = parent.justify_self_safe;
            c.justify_self_last = parent.justify_self_last;
            c.justify_self_physical = parent.justify_self_physical;
            c.justify_self_normal = parent.justify_self_normal;
            c.justify_self_own_axis = parent.justify_self_own_axis;
        }
    }
}
