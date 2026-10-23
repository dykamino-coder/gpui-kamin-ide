//! Стадия вычисленных значений, каскадное `inherit` ненаследуемых длинных свойств реестра
//! (css-cascade-5 §7.3.1): каждое берёт у родителя своё ВЫЧИСЛЕННОЕ значение — стороны полей,
//! отступов и вставок, размеры (с `calc-size()`), толщину, стиль и цвет сторон рамки (цвет —
//! ключевым словом `currentcolor`, его решает цвет ребёнка: css-color-4 §6.4), части контура,
//! длинные свойства фона, выравнивания и свойства с полями в реестре `inherited_value`.

use crate::document::state::ValueCx;
use crate::style::computed::{BgListProp, Computed, Sides};
use crate::style::css::{BoxGroup as G, LonghandId as L, Slot};
use crate::style::values::value::Len;

/// Скопировать в `c` вычисленные значения родителя `p` у свойств множества
/// `c.inherit_longhands`.
pub(super) fn inherit_longhands(c: &mut Computed, p: &Computed, vcx: ValueCx<'_>) {
    for id in c.inherit_longhands.iter() {
        match id.slot() {
            Slot::Physical(G::Margin, i) => *side(&mut c.margin, i) = of(&p.margin, i),
            Slot::Physical(G::Padding, i) => *side(&mut c.padding, i) = of(&p.padding, i),
            Slot::Physical(G::Inset, i) => *side(&mut c.inset, i) = of(&p.inset, i),
            Slot::Physical(G::Size, i) => size(c, p, i),
            Slot::Physical(G::BorderWidth, i) => {
                *side(&mut c.border_width, i) = of(&p.border_width, i)
            }
            Slot::Physical(G::BorderStyle, i) => {
                c.border_visible[i] = p.border_visible[i];
                c.border_side_styles[i] = p.border_side_styles[i];
                c.sync_border_pattern();
            }
            Slot::Physical(G::BorderColor, i) => c.border_colors[i] = p.border_colors[i].clone(),
            // Логическое свойство ложится на физическое по письму
            // (`resolve_logical`) вместе со своим `inherit`.
            Slot::Logical(..) => {}
            Slot::Single => single(c, p, id, vcx),
        }
    }
}

/// Сторона `i` в порядке верх, право, низ, лево.
fn of(s: &Sides, i: usize) -> Option<Len> {
    [s.top, s.right, s.bottom, s.left][i]
}

fn side(s: &mut Sides, i: usize) -> &mut Option<Len> {
    match i {
        0 => &mut s.top,
        1 => &mut s.right,
        2 => &mut s.bottom,
        _ => &mut s.left,
    }
}

/// Размер `i` (width, height, min-width, min-height, max-width, max-height)
/// вместе с выражением `calc-size()` и его основой.
fn size(c: &mut Computed, p: &Computed, i: usize) {
    c.calc_size_basis[i] = p.calc_size_basis[i];
    if i < 4 {
        c.calc_size[i] = p.calc_size[i];
    } else {
        c.calc_size_max[i - 4] = p.calc_size_max[i - 4];
    }
    match i {
        0 => c.width = p.width,
        1 => c.height = p.height,
        2 => c.min_width = p.min_width,
        3 => c.min_height = p.min_height,
        4 => c.max_width = p.max_width,
        _ => c.max_height = p.max_height,
    }
}

fn single(c: &mut Computed, p: &Computed, id: L, vcx: ValueCx<'_>) {
    let bg_list = |c: &mut Computed, prop: BgListProp| {
        c.bg_lists.remove(prop);
        if let Some(layers) = p.bg_lists.get(prop) {
            c.bg_lists.set_layers(prop, layers.to_vec());
        }
    };
    match id {
        L::OutlineWidth | L::OutlineStyle | L::OutlineColor | L::OutlineOffset => {
            let from = p.outline.clone().unwrap_or_default();
            let mut o = c.outline.clone().unwrap_or_default();
            match id {
                L::OutlineWidth => o.width = from.width,
                L::OutlineStyle => o.style = from.style,
                // `currentColor` вычисляется В СЕБЯ (css-color-4 §resolving)
                // и наследуется словом (`outline-019`).
                L::OutlineColor => o.color = from.color,
                // Метка `inset` — часть значения сдвига.
                _ => (o.offset, o.inset) = (from.offset, from.inset),
            }
            c.outline = Some(o);
        }
        L::BackgroundColor => c.background_color = p.background_color.clone(),
        L::BackgroundImage => {
            c.bg_image = p.bg_image.clone();
            c.gradient = p.gradient.clone();
            c.gradient_raw = p.gradient_raw.clone();
            c.gradient_current = p.gradient_current.clone();
            bg_list(c, BgListProp::Image);
        }
        L::BackgroundPositionX => {
            c.bg_pos.x = p.bg_pos.x;
            c.bg_position_math[0] = p.bg_position_math[0].clone();
            bg_list(c, BgListProp::PositionX);
        }
        L::BackgroundPositionY => {
            c.bg_pos.y = p.bg_pos.y;
            c.bg_position_math[1] = p.bg_position_math[1].clone();
            bg_list(c, BgListProp::PositionY);
        }
        L::BackgroundRepeat => {
            c.bg_repeat = p.bg_repeat;
            bg_list(c, BgListProp::Repeat);
        }
        L::BackgroundOrigin => {
            c.bg_origin = p.bg_origin;
            bg_list(c, BgListProp::Origin);
        }
        L::BackgroundClip => {
            c.bg_clip = p.bg_clip;
            bg_list(c, BgListProp::Clip);
        }
        L::BackgroundSize => {
            c.bg_size = p.bg_size;
            bg_list(c, BgListProp::Size);
        }
        L::BackgroundAttachment => {
            c.bg_fixed = p.bg_fixed;
            bg_list(c, BgListProp::Attachment);
        }
        // Значение родителя целиком, с его `safe`/`last` (`place-items:
        // inherit` во вложенной сетке —
        // `grid-self-alignment-baseline-with-grid-001`).
        L::AlignItems => {
            c.align_items = p.align_items;
            c.align_items_safe = p.align_items_safe;
            c.align_items_last = p.align_items_last;
        }
        L::JustifyItems => {
            c.justify_items = p.justify_items;
            c.justify_items_safe = p.justify_items_safe;
            c.justify_items_last = p.justify_items_last;
        }
        L::AlignContent => {
            c.align_content = p.align_content;
            c.align_content_baseline = p.align_content_baseline;
            c.align_content_safe = p.align_content_safe;
            c.align_content_unsafe = p.align_content_unsafe;
            c.align_content_block = p.align_content_block;
        }
        L::JustifyContent => {
            c.justify_content = p.justify_content;
            c.justify_content_safe = p.justify_content_safe;
        }
        L::JustifySelf => {
            c.justify_self = p.justify_self;
            c.justify_self_safe = p.justify_self_safe;
            c.justify_self_last = p.justify_self_last;
            c.justify_self_physical = p.justify_self_physical;
            c.justify_self_normal = p.justify_self_normal;
            c.justify_self_own_axis = p.justify_self_own_axis;
        }
        // `align-self: inherit` решает проход дерева по объявленному значению
        // родителя (`dom::fixup_tree::alignment`): там же гашение `align-self`
        // у коробки вне flex/grid.
        L::AlignSelf => {}
        _ => {
            for property in crate::style::cascade::inherited_value::of_longhand(id) {
                property.copy(p, c, vcx);
            }
        }
    }
}
