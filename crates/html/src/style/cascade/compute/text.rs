//! Стадия вычисленных значений, текст: emphasis, ruby, переносы, списки, block-ellipsis,
//! font-feature-*, тени текста, rtl, text-align; пробелы, курсор, таблицы и цвета интерфейса
//! (`inherit_ui`); вычисленные значения оформления текста (`inherit_decoration_values`).

use super::*;

pub(super) fn inherit_text(parent: &Computed, own: &Computed, c: &mut Computed) {
    c.text_emphasis = own.text_emphasis.clone().or(parent.text_emphasis.clone());
    c.emphasis_auto_shape = if own.text_emphasis.is_some() {
        own.emphasis_auto_shape
    } else {
        parent.emphasis_auto_shape
    };
    c.emphasis_position_auto = own.emphasis_position_auto.or(parent.emphasis_position_auto);
    let position = if own.emphasis_position_auto.is_some() {
        own
    } else {
        parent
    };
    c.emphasis_under = position.emphasis_under;
    c.emphasis_left = position.emphasis_left;
    c.emphasis_color = own
        .emphasis_color
        .clone()
        .or_else(|| parent.emphasis_color.clone());
    // css-ruby-1 §4.1/§4.3: оба свойства наследуемые.
    c.ruby_under = own.ruby_under.or(parent.ruby_under);
    c.ruby_align = own.ruby_align.or(parent.ruby_align);
    c.ruby_overhang = own.ruby_overhang.or(parent.ruby_overhang);
    c.ruby_merge = own.ruby_merge.or(parent.ruby_merge);
    // `image-orientation` наследуется (css-images-3 §5.4, «Inherited: yes»):
    // в наборе его ставят на `body`, а действует он на каждой картинке.
    c.image_orient_none = own.image_orient_none.or(parent.image_orient_none);
    c.font_oblique_only = own.font_oblique_only.or(parent.font_oblique_only);
    c.font_synth = (
        own.font_synth.0.or(parent.font_synth.0),
        own.font_synth.1.or(parent.font_synth.1),
        own.font_synth.2.or(parent.font_synth.2),
    );
    c.keep_all = own.keep_all.or(parent.keep_all);
    c.hyphens_auto = own.hyphens_auto.or(parent.hyphens_auto);
    c.hyphen_char = own
        .hyphen_char
        .clone()
        .or_else(|| parent.hyphen_char.clone());
    c.hyphen_limit_chars = own.hyphen_limit_chars.or(parent.hyphen_limit_chars);
    c.hyphen_limit_lines = own.hyphen_limit_lines.or(parent.hyphen_limit_lines);
    c.lang = own.lang.clone().or(parent.lang.clone());
    c.break_after_spaces = own.break_after_spaces.or(parent.break_after_spaces);
    c.hyphenate = own.hyphenate.or(parent.hyphenate);
    c.tab_size = if own.tab_size_len.is_some() {
        None
    } else {
        own.tab_size.or(parent.tab_size)
    };
    c.list_style_type = own
        .list_style_type
        .clone()
        .or_else(|| parent.list_style_type.clone());
    c.list_style_inside = own.list_style_inside.or(parent.list_style_inside);
    c.list_style_image = own
        .list_style_image
        .clone()
        .or_else(|| parent.list_style_image.clone());
    // `text-fit` наследуется (css-text-5 §text-fit).
    c.text_fit = own.text_fit.or(parent.text_fit);
    c.vertical_align = own.vertical_align.or(parent.vertical_align);
    c.font_stretch = own.font_stretch.or(parent.font_stretch);
    // `font-size-adjust` наследуется значением; подгонку каждый элемент
    // считает сам, по СВОЕМУ шрифту (`Computed::resolve_em`).
    c.font_size_adjust = own.font_size_adjust.or(parent.font_size_adjust);
    c.no_select = own.no_select.or(parent.no_select);
    c.pointer_events_none = own.pointer_events_none.or(parent.pointer_events_none);
    // `block-ellipsis` наследуется (css-overflow-4 §block-ellipsis).
    c.clamp_mark = own.clamp_mark.clone().or_else(|| parent.clamp_mark.clone());
    c.clamp_legacy = own.clamp_legacy.or(parent.clamp_legacy);
    // Гейтовый флаг `webkit_box` НЕ наследуется: пара display+orient
    // обязана стоять на самом элементе.
    c.webkit_box_vertical = own.webkit_box_vertical.or(parent.webkit_box_vertical);
    // Каждое `font-variant-*` наследуется само по себе (css-fonts-4 §6):
    // незаданная своя группа тегов берётся у родителя, заданная (в том числе
    // `normal` и сброс сокращением `font`) — своя.
    for (group, tags) in crate::style::computed::FONT_VARIANT_GROUPS
        .iter()
        .enumerate()
    {
        if own.font_variant_set & (1 << group) == 0 {
            c.font_features.extend(
                parent
                    .font_features
                    .iter()
                    .filter(|(t, _)| tags.contains(&t.as_str()))
                    .cloned(),
            );
        }
    }
    c.font_variant_emoji = own.font_variant_emoji.or(parent.font_variant_emoji);
    c.font_synth_position = own.font_synth_position.or(parent.font_synth_position);
    c.font_kerning = own.font_kerning.or(parent.font_kerning);
    c.font_alternates = own
        .font_alternates
        .clone()
        .or_else(|| parent.font_alternates.clone());
    // `font-feature-settings` наследуется своим значением независимо от
    // `font-variant-*` ребёнка (css-fonts-4 §6.12: `font-variant: none` «does
    // not reset … font-feature-settings»).
    c.font_settings = own
        .font_settings
        .clone()
        .or_else(|| parent.font_settings.clone());
    c.text_shadow = if own.text_shadow_none {
        None
    } else {
        own.text_shadow
            .clone()
            .or_else(|| parent.text_shadow.clone())
    };
    // Хвост списка идёт вместе с первой тенью: своя запись — свой хвост,
    // унаследованная — хвост родителя (css-text-decor-3: `text-shadow`
    // наследуется списком целиком).
    if own.text_shadow_none {
        c.text_shadow_rest.clear();
    } else if own.text_shadow.is_none() {
        c.text_shadow_rest = parent.text_shadow_rest.clone();
    }
    c.rtl = own.rtl.or(parent.rtl);
    // `text-align: start|end` — края СТРОКИ, и разворачиваются они в момент
    // ОТРИСОВКИ, а не здесь: иначе левый край, вычисленный для тела страницы,
    // достаётся по наследству и вложенному блоку справа налево (`physical`
    // зовёт `lines::align_for`). Умолчание CSS — `start`.
    c.text_align = Some(c.text_align.unwrap_or(TextAlign::Start));
}

/// Наследуемые по CSS свойства пробелов, видимости, курсора, таблиц и цвета
/// интерфейса и каскадное `inherit` у `background-blend-mode`.
pub(super) fn inherit_ui(parent: &Computed, own: &Computed, c: &mut Computed) {
    use crate::style::computed::{BgListProp, BlendMode};
    if c.background_blend == Some(BlendMode::Inherit) {
        c.background_blend = parent.background_blend;
        c.bg_lists.remove(BgListProp::BlendMode);
        if let Some(list) = parent.bg_lists.get(BgListProp::BlendMode) {
            c.bg_lists.set_layers(BgListProp::BlendMode, list.to_vec());
        }
    }
    // Без них `white-space: pre` на контейнере не доходил до вложенного
    // текста, а курсор и зазор ячеек не доставались детям.
    c.preserve_newlines = own.preserve_newlines.or(parent.preserve_newlines);
    c.keep_spaces = own.keep_spaces.or(parent.keep_spaces);
    c.hidden = own.hidden.or(parent.hidden);
    c.cursor = own.cursor.clone().or(parent.cursor.clone());
    c.border_collapse = own.border_collapse.or(parent.border_collapse);
    c.empty_cells_hide = own.empty_cells_hide.or(parent.empty_cells_hide);
    c.border_spacing = own.border_spacing.or(parent.border_spacing);
    c.caret_color = own
        .caret_color
        .clone()
        .or_else(|| parent.caret_color.clone());
    c.accent_color = own
        .accent_color
        .clone()
        .or_else(|| parent.accent_color.clone());
}

/// Наследуемые свойства оформления текста (css-text-decor-4): смещение
/// подчёркивания — вычисленной длиной от своего кегля (доля остаётся
/// долей), его положение и пропуски. Сами линии не наследуются, а
/// распространяются по дереву коробок (`inherit::decorate`).
pub(super) fn inherit_decoration_values(
    parent: &Computed,
    own: &Computed,
    c: &mut Computed,
    own_px: f32,
    vcx: crate::document::state::ValueCx<'_>,
) {
    let family = decor_family(c, vcx);
    c.underline_offset = own
        .underline_offset
        .map(|l| resolve_decor_len(l, &family, own_px, vcx))
        .or(parent.underline_offset);
    c.underline_pos = own.underline_pos.or(parent.underline_pos);
    c.skip_ink = own.skip_ink.or(parent.skip_ink);
    c.skip_spaces = own.skip_spaces.or(parent.skip_spaces);
}

/// Семейство для `ch`/`ex` в длинах оформления — как у `resolve_em`:
/// родовое `monospace` имени не даёт, а меряться должно тем шрифтом,
/// которым набран текст.
pub(in crate::style::cascade) fn decor_family(
    c: &Computed,
    vcx: crate::document::state::ValueCx<'_>,
) -> String {
    c.font_family.clone().unwrap_or_else(|| {
        if c.monospace == Some(true) {
            crate::text::metrics::mono_family_for(c.lang.as_deref(), vcx).to_string()
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
    vcx: crate::document::state::ValueCx<'_>,
) -> crate::style::computed::DecorLen {
    use crate::style::computed::{DecorLen, FontCx};
    match l {
        DecorLen::Raw(raw) => raw
            .to_px(None, FontCx::new(family, own_px), vcx)
            .map_or(DecorLen::Auto, DecorLen::Px),
        other => other,
    }
}
