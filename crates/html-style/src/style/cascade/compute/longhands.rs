//! Стадия вычисленных значений, каскадное `inherit` ненаследуемых длинных свойств реестра
//! (css-cascade-5 §7.3.1): каждое берёт у родителя своё ВЫЧИСЛЕННОЕ значение — стороны полей,
//! отступов и вставок, размеры (с `calc-size()`), толщину, стиль и цвет сторон рамки (цвет —
//! ключевым словом `currentcolor`, его решает цвет ребёнка: css-color-4 §6.4), части контура,
//! длинные свойства фона, выравнивания и свойства с полями в реестре `inherited_value`.

use crate::style::computed::{BgListProp, Computed, Sides};
use crate::style::css::{BoxGroup as G, LonghandId as L, Slot};
use crate::style::values::value::Len;
use crate::values::ValueCx;

/// Скопировать в `c` вычисленные значения родителя `p` у свойств множества
/// `c.inherit_longhands`.
pub(super) fn inherit_longhands(c: &mut Computed, p: &Computed, vcx: ValueCx<'_>) {
    for id in c.reset_cascade.inherit_longhands.iter() {
        match id.slot() {
            Slot::Physical(G::Margin, i) => {
                *side(&mut c.reset_box_layout.margin, i) = of(&p.reset_box_layout.margin, i)
            }
            Slot::Physical(G::Padding, i) => {
                *side(&mut c.reset_box_layout.padding, i) = of(&p.reset_box_layout.padding, i)
            }
            Slot::Physical(G::Inset, i) => {
                *side(&mut c.reset_positioned.inset, i) = of(&p.reset_positioned.inset, i)
            }
            Slot::Physical(G::Size, i) => size(c, p, i),
            Slot::Physical(G::BorderWidth, i) => {
                *side(&mut c.reset_border.border_width, i) = of(&p.reset_border.border_width, i)
            }
            Slot::Physical(G::BorderStyle, i) => {
                c.reset_border.border_visible[i] = p.reset_border.border_visible[i];
                c.reset_border.border_side_styles[i] = p.reset_border.border_side_styles[i];
                c.sync_border_pattern();
            }
            Slot::Physical(G::BorderColor, i) => {
                c.reset_border.border_colors[i] = p.reset_border.border_colors[i].clone()
            }
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
    c.reset_box_layout.calc_size_basis[i] = p.reset_box_layout.calc_size_basis[i];
    if i < 4 {
        c.reset_box_layout.calc_size[i] = p.reset_box_layout.calc_size[i];
    } else {
        c.reset_box_layout.calc_size_max[i - 4] = p.reset_box_layout.calc_size_max[i - 4];
    }
    match i {
        0 => c.reset_box_layout.width = p.reset_box_layout.width,
        1 => c.reset_box_layout.height = p.reset_box_layout.height,
        2 => c.reset_box_layout.min_width = p.reset_box_layout.min_width,
        3 => c.reset_box_layout.min_height = p.reset_box_layout.min_height,
        4 => c.reset_box_layout.max_width = p.reset_box_layout.max_width,
        _ => c.reset_box_layout.max_height = p.reset_box_layout.max_height,
    }
}

fn single(c: &mut Computed, p: &Computed, id: L, vcx: ValueCx<'_>) {
    let bg_list = |c: &mut Computed, prop: BgListProp| {
        c.reset_background.bg_lists.remove(prop);
        if let Some(layers) = p.reset_background.bg_lists.get(prop) {
            c.reset_background
                .bg_lists
                .set_layers(prop, layers.to_vec());
        }
    };
    match id {
        L::OutlineWidth | L::OutlineStyle | L::OutlineColor | L::OutlineOffset => {
            let from = p.reset_border.outline.clone().unwrap_or_default();
            let mut o = c.reset_border.outline.clone().unwrap_or_default();
            match id {
                L::OutlineWidth => o.width = from.width,
                L::OutlineStyle => o.style = from.style,
                // `currentColor` вычисляется В СЕБЯ (css-color-4 §resolving)
                // и наследуется словом (`outline-019`).
                L::OutlineColor => o.color = from.color,
                // Метка `inset` — часть значения сдвига.
                _ => (o.offset, o.inset) = (from.offset, from.inset),
            }
            c.reset_border.outline = Some(o);
        }
        L::BackgroundColor => {
            c.reset_background.background_color = p.reset_background.background_color.clone()
        }
        L::BackgroundImage => {
            c.reset_background.bg_image = p.reset_background.bg_image.clone();
            c.reset_background.gradient = p.reset_background.gradient.clone();
            c.reset_background.gradient_raw = p.reset_background.gradient_raw.clone();
            c.reset_background.gradient_current = p.reset_background.gradient_current.clone();
            bg_list(c, BgListProp::Image);
        }
        L::BackgroundPositionX => {
            c.reset_background.bg_pos.x = p.reset_background.bg_pos.x;
            c.reset_background.bg_position_math[0] = p.reset_background.bg_position_math[0].clone();
            bg_list(c, BgListProp::PositionX);
        }
        L::BackgroundPositionY => {
            c.reset_background.bg_pos.y = p.reset_background.bg_pos.y;
            c.reset_background.bg_position_math[1] = p.reset_background.bg_position_math[1].clone();
            bg_list(c, BgListProp::PositionY);
        }
        L::BackgroundRepeat => {
            c.reset_background.bg_repeat = p.reset_background.bg_repeat;
            bg_list(c, BgListProp::Repeat);
        }
        L::BackgroundOrigin => {
            c.reset_background.bg_origin = p.reset_background.bg_origin;
            bg_list(c, BgListProp::Origin);
        }
        L::BackgroundClip => {
            c.reset_background.bg_clip = p.reset_background.bg_clip;
            bg_list(c, BgListProp::Clip);
        }
        L::BackgroundSize => {
            c.reset_background.bg_size = p.reset_background.bg_size;
            bg_list(c, BgListProp::Size);
        }
        L::BackgroundAttachment => {
            c.reset_background.bg_fixed = p.reset_background.bg_fixed;
            bg_list(c, BgListProp::Attachment);
        }
        // Значение родителя целиком, с его `safe`/`last` (`place-items:
        // inherit` во вложенной сетке —
        // `grid-self-alignment-baseline-with-grid-001`).
        L::AlignItems => {
            c.reset_flex.align_items = p.reset_flex.align_items;
            c.reset_flex.align_items_safe = p.reset_flex.align_items_safe;
            c.reset_flex.align_items_last = p.reset_flex.align_items_last;
        }
        L::JustifyItems => {
            c.reset_flex.justify_items = p.reset_flex.justify_items;
            c.reset_flex.justify_items_safe = p.reset_flex.justify_items_safe;
            c.reset_flex.justify_items_last = p.reset_flex.justify_items_last;
        }
        L::AlignContent => {
            c.reset_flex.align_content = p.reset_flex.align_content;
            c.reset_flex.align_content_baseline = p.reset_flex.align_content_baseline;
            c.reset_flex.align_content_safe = p.reset_flex.align_content_safe;
            c.reset_flex.align_content_unsafe = p.reset_flex.align_content_unsafe;
            c.reset_flex.align_content_block = p.reset_flex.align_content_block;
        }
        L::JustifyContent => {
            c.reset_flex.justify_content = p.reset_flex.justify_content;
            c.reset_flex.justify_content_safe = p.reset_flex.justify_content_safe;
        }
        L::JustifySelf => {
            c.reset_flex.justify_self = p.reset_flex.justify_self;
            c.reset_flex.justify_self_safe = p.reset_flex.justify_self_safe;
            c.reset_flex.justify_self_last = p.reset_flex.justify_self_last;
            c.reset_flex.justify_self_physical = p.reset_flex.justify_self_physical;
            c.reset_flex.justify_self_normal = p.reset_flex.justify_self_normal;
            c.reset_flex.justify_self_own_axis = p.reset_flex.justify_self_own_axis;
        }
        // `align-self: inherit` (css-cascade-5 §7.3.1) — вычисленное значение
        // родителя: его авторское слово, даже когда у самого родителя оно
        // погашено как неприменимое (`dom::fixup_tree::alignment`: гашение —
        // про используемое значение). Ребёнку блочного контейнера свойство
        // неприменимо (css-align-3 §6.1) — то же гашение и здесь.
        L::AlignSelf => {
            let value = p
                .reset_flex
                .align_self_decl
                .unwrap_or(p.reset_flex.align_self);
            c.reset_flex.align_self = value;
            c.reset_flex.align_self_decl = Some(value);
            c.reset_flex.align_self_safe = p.reset_flex.align_self_safe;
            c.reset_flex.align_self_normal = p.reset_flex.align_self_normal;
            c.reset_flex.align_self_own_axis = p.reset_flex.align_self_own_axis;
            c.reset_flex.align_self_flex_kw = p.reset_flex.align_self_flex_kw;
            c.reset_flex.align_self_last = p.reset_flex.align_self_last;
            // TRANSITIONAL(cascade-7b-R06): Явный inherit align-self гасится для block parent в computed стадии; удалить гашение после сохранения computed значения и проверки применимости в used block layout.
            use crate::style::computed::{Display as D, Position as P};
            let block_parent = matches!(
                p.reset_box_layout.display,
                None | Some(D::Block)
                    | Some(D::ListItem)
                    | Some(D::InlineBlock)
                    | Some(D::TableCell)
            ) && p.reset_flex.webkit_box != Some(true);
            if block_parent
                && c.reset_flex.align_self.is_some()
                && !matches!(
                    c.reset_positioned.position,
                    Some(P::Absolute) | Some(P::Fixed)
                )
            {
                c.reset_flex.align_self = None;
                c.reset_flex.align_self_normal = false;
            }
        }
        // Цвет линейки родителя целиком, не `currentcolor` своего текста
        // (`multicol-rule-color-inherit-001/002`).
        L::ColumnRuleColor => {
            c.reset_fragmentation.column_rule_color =
                p.reset_fragmentation.column_rule_color.clone();
            c.reset_fragmentation.column_rule_colors =
                p.reset_fragmentation.column_rule_colors.clone();
        }
        L::RowRuleColor => {
            c.reset_fragmentation.row_rule_color = p.reset_fragmentation.row_rule_color.clone();
            c.reset_fragmentation.row_rule_colors = p.reset_fragmentation.row_rule_colors.clone();
        }
        L::GridTemplateAreas => c.reset_grid.grid_areas = p.reset_grid.grid_areas.clone(),
        _ => {
            for property in crate::style::cascade::inherited_value::of_longhand(id) {
                property.copy(p, c, vcx);
            }
        }
    }
}
