//! Наследование текстовых свойств и разрешение оформления.
use super::*;
/// Наследуемые по CSS свойства пробелов, видимости, курсора, таблиц и цвета
/// интерфейса и каскадное `inherit` у `background-blend-mode`.
pub(in crate::style::cascade::compute) fn inherit_ui(
    parent: &Computed,
    own: &Computed,
    c: &mut Computed,
) {
    use crate::style::computed::{BgListProp, BlendMode};
    if c.reset_background.background_blend == Some(BlendMode::Inherit) {
        c.reset_background.background_blend = parent.reset_background.background_blend;
        c.reset_background.bg_lists.remove(BgListProp::BlendMode);
        if let Some(list) = parent.reset_background.bg_lists.get(BgListProp::BlendMode) {
            c.reset_background
                .bg_lists
                .set_layers(BgListProp::BlendMode, list.to_vec());
        }
    }
    // Без них `white-space: pre` на контейнере не доходил до вложенного
    // текста, а курсор и зазор ячеек не доставались детям.
    c.inh.text_flow.preserve_newlines = own
        .inh
        .text_flow
        .preserve_newlines
        .or(parent.inh.text_flow.preserve_newlines);
    c.inh.text_flow.keep_spaces = own
        .inh
        .text_flow
        .keep_spaces
        .or(parent.inh.text_flow.keep_spaces);
    c.inh.ui.hidden = own.inh.ui.hidden.or(parent.inh.ui.hidden);
    c.inh.ui.cursor = own.inh.ui.cursor.clone().or(parent.inh.ui.cursor.clone());
    c.inh.ui.color_scheme = own
        .inh
        .ui
        .color_scheme
        .clone()
        .or_else(|| parent.inh.ui.color_scheme.clone());
    c.inh.table_list.border_collapse = own
        .inh
        .table_list
        .border_collapse
        .or(parent.inh.table_list.border_collapse);
    c.inh.table_list.empty_cells_hide = own
        .inh
        .table_list
        .empty_cells_hide
        .or(parent.inh.table_list.empty_cells_hide);
    c.inh.table_list.border_spacing = own
        .inh
        .table_list
        .border_spacing
        .or(parent.inh.table_list.border_spacing);
    c.inh.ui.caret_color = own
        .inh
        .ui
        .caret_color
        .clone()
        .or_else(|| parent.inh.ui.caret_color.clone());
    c.inh.ui.accent_color = own
        .inh
        .ui
        .accent_color
        .clone()
        .or_else(|| parent.inh.ui.accent_color.clone());
}

/// Наследуемые свойства оформления текста (css-text-decor-4): смещение
/// подчёркивания — вычисленной длиной от своего кегля (доля остаётся
/// долей), его положение и пропуски. Сами линии не наследуются, а
/// распространяются по дереву коробок (`inherit::decorate`).
pub(in crate::style::cascade::compute) fn inherit_decoration_values(
    parent: &Computed,
    own: &Computed,
    c: &mut Computed,
    own_px: f32,
    vcx: crate::values::ValueCx<'_>,
) {
    use crate::style::computed::DecorLen;
    let family = decor_family(c, vcx);
    let vp = c.viewport_cx(vcx.viewport);
    c.inh.text_ink.underline_offset = own
        .inh
        .text_ink
        .underline_offset
        .map(|l| match l {
            DecorLen::Raw(raw) => DecorLen::Raw(raw.fold_container(vp, vcx)),
            other => other,
        })
        .map(|l| resolve_decor_len(l, &family, own_px, vcx))
        .or(parent.inh.text_ink.underline_offset);
    c.inh.text_ink.underline_pos = own
        .inh
        .text_ink
        .underline_pos
        .or(parent.inh.text_ink.underline_pos);
    c.inh.text_ink.skip_ink = own.inh.text_ink.skip_ink.or(parent.inh.text_ink.skip_ink);
    c.inh.text_ink.skip_spaces = own
        .inh
        .text_ink
        .skip_spaces
        .or(parent.inh.text_ink.skip_spaces);
}

/// Семейство для `ch`/`ex` в длинах оформления — как у `resolve_em`:
/// родовое `monospace` имени не даёт, а меряться должно тем шрифтом,
/// которым набран текст.
pub(in crate::style::cascade) fn decor_family(
    c: &Computed,
    vcx: crate::values::ValueCx<'_>,
) -> String {
    c.inh.font.font_family.clone().unwrap_or_else(|| {
        if c.inh.font.monospace == Some(true) {
            vcx.doc
                .font_snapshot()
                .mono_family(c.inh.text_flow.lang.as_deref())
                .to_string()
        } else {
            String::new()
        }
    })
}

/// Длина оформления в единицах шрифта — в точки собственным кеглем.
pub(in crate::style::cascade) fn resolve_decor_len(
    l: crate::style::computed::DecorLen,
    family: &str,
    own_px: f32,
    vcx: crate::values::ValueCx<'_>,
) -> crate::style::computed::DecorLen {
    use crate::style::computed::{DecorLen, FontCx};
    match l {
        DecorLen::Raw(raw) => raw
            .to_px(None, FontCx::new(family, own_px), vcx)
            .map_or(DecorLen::Auto, DecorLen::Px),
        other => other,
    }
}
