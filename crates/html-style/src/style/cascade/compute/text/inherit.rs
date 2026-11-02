//! Наследование текстовых свойств и разрешение оформления.
use super::*;
pub(in crate::style::cascade::compute) fn inherit_text(
    parent: &Computed,
    own: &Computed,
    c: &mut Computed,
) {
    c.inh.text_ink.text_emphasis = own.inh.text_ink.text_emphasis.clone().or(parent
        .inh
        .text_ink
        .text_emphasis
        .clone());
    c.inh.text_ink.emphasis_auto_shape = if own.inh.text_ink.text_emphasis.is_some() {
        own.inh.text_ink.emphasis_auto_shape
    } else {
        parent.inh.text_ink.emphasis_auto_shape
    };
    c.inh.text_ink.emphasis_position_auto = own
        .inh
        .text_ink
        .emphasis_position_auto
        .or(parent.inh.text_ink.emphasis_position_auto);
    let position = if own.inh.text_ink.emphasis_position_auto.is_some() {
        own
    } else {
        parent
    };
    c.inh.text_ink.emphasis_under = position.inh.text_ink.emphasis_under;
    c.inh.text_ink.emphasis_left = position.inh.text_ink.emphasis_left;
    c.inh.text_ink.emphasis_color = own
        .inh
        .text_ink
        .emphasis_color
        .clone()
        .or_else(|| parent.inh.text_ink.emphasis_color.clone());
    // css-ruby-1 §4.1/§4.3: оба свойства наследуемые.
    c.inh.text_flow.ruby_under = own
        .inh
        .text_flow
        .ruby_under
        .or(parent.inh.text_flow.ruby_under);
    c.inh.text_flow.ruby_align = own
        .inh
        .text_flow
        .ruby_align
        .or(parent.inh.text_flow.ruby_align);
    c.inh.text_flow.ruby_overhang = own
        .inh
        .text_flow
        .ruby_overhang
        .or(parent.inh.text_flow.ruby_overhang);
    c.inh.text_flow.ruby_merge = own
        .inh
        .text_flow
        .ruby_merge
        .or(parent.inh.text_flow.ruby_merge);
    // `image-orientation` наследуется (css-images-3 §5.4, «Inherited: yes»):
    // в наборе его ставят на `body`, а действует он на каждой картинке.
    c.inh.ui.image_orient_none = own
        .inh
        .ui
        .image_orient_none
        .or(parent.inh.ui.image_orient_none);
    c.inh.font.font_oblique_only = own
        .inh
        .font
        .font_oblique_only
        .or(parent.inh.font.font_oblique_only);
    c.inh.font.font_synth = (
        own.inh.font.font_synth.0.or(parent.inh.font.font_synth.0),
        own.inh.font.font_synth.1.or(parent.inh.font.font_synth.1),
        own.inh.font.font_synth.2.or(parent.inh.font.font_synth.2),
    );
    c.inh.text_flow.keep_all = own.inh.text_flow.keep_all.or(parent.inh.text_flow.keep_all);
    c.inh.text_flow.hyphens_auto = own
        .inh
        .text_flow
        .hyphens_auto
        .or(parent.inh.text_flow.hyphens_auto);
    c.inh.text_flow.hyphen_char = own
        .inh
        .text_flow
        .hyphen_char
        .clone()
        .or_else(|| parent.inh.text_flow.hyphen_char.clone());
    c.inh.text_flow.hyphen_limit_chars = own
        .inh
        .text_flow
        .hyphen_limit_chars
        .or(parent.inh.text_flow.hyphen_limit_chars);
    c.inh.text_flow.hyphen_limit_lines = own
        .inh
        .text_flow
        .hyphen_limit_lines
        .or(parent.inh.text_flow.hyphen_limit_lines);
    c.inh.text_flow.lang = own
        .inh
        .text_flow
        .lang
        .clone()
        .or(parent.inh.text_flow.lang.clone());
    c.inh.text_flow.break_after_spaces = own
        .inh
        .text_flow
        .break_after_spaces
        .or(parent.inh.text_flow.break_after_spaces);
    c.inh.text_flow.hyphenate = own
        .inh
        .text_flow
        .hyphenate
        .or(parent.inh.text_flow.hyphenate);
    c.inh.text_flow.tab_size = if own.inh.text_flow.tab_size_len.is_some() {
        None
    } else {
        own.inh.text_flow.tab_size.or(parent.inh.text_flow.tab_size)
    };
    c.inh.table_list.list_style_type = own
        .inh
        .table_list
        .list_style_type
        .clone()
        .or_else(|| parent.inh.table_list.list_style_type.clone());
    c.inh.table_list.list_style_inside = own
        .inh
        .table_list
        .list_style_inside
        .or(parent.inh.table_list.list_style_inside);
    c.inh.table_list.list_style_image = own
        .inh
        .table_list
        .list_style_image
        .clone()
        .or_else(|| parent.inh.table_list.list_style_image.clone());
    // `text-fit` наследуется (css-text-5 §text-fit).
    c.inh.text_flow.text_fit = own.inh.text_flow.text_fit.or(parent.inh.text_flow.text_fit);
    c.inh.text_flow.vertical_align = own
        .inh
        .text_flow
        .vertical_align
        .or(parent.inh.text_flow.vertical_align);
    c.inh.font.font_stretch = own.inh.font.font_stretch.or(parent.inh.font.font_stretch);
    // `font-size-adjust` наследуется значением; подгонку каждый элемент
    // считает сам, по СВОЕМУ шрифту (`Computed::resolve_em`).
    c.inh.font.font_size_adjust = own
        .inh
        .font
        .font_size_adjust
        .or(parent.inh.font.font_size_adjust);
    c.inh.ui.pointer_events_none = own
        .inh
        .ui
        .pointer_events_none
        .or(parent.inh.ui.pointer_events_none);
    // `block-ellipsis` наследуется (css-overflow-4 §block-ellipsis).
    c.inh.text_flow.clamp_mark = own
        .inh
        .text_flow
        .clamp_mark
        .clone()
        .or_else(|| parent.inh.text_flow.clamp_mark.clone());
    c.inh.text_flow.clamp_legacy = own
        .inh
        .text_flow
        .clamp_legacy
        .or(parent.inh.text_flow.clamp_legacy);
    // Гейтовый флаг `webkit_box` НЕ наследуется: пара display+orient
    // обязана стоять на самом элементе.
    c.inh.text_flow.webkit_box_vertical = own
        .inh
        .text_flow
        .webkit_box_vertical
        .or(parent.inh.text_flow.webkit_box_vertical);
    // Каждое `font-variant-*` наследуется само по себе (css-fonts-4 §6):
    // незаданная своя группа тегов берётся у родителя, заданная (в том числе
    // `normal` и сброс сокращением `font`) — своя.
    for (group, tags) in crate::style::computed::FONT_VARIANT_GROUPS
        .iter()
        .enumerate()
    {
        if own.reset_cascade.font_variant_set & (1 << group) == 0 {
            c.inh.font.font_features.extend(
                parent
                    .inh
                    .font
                    .font_features
                    .iter()
                    .filter(|(t, _)| tags.contains(&t.as_str()))
                    .cloned(),
            );
        }
    }
    c.inh.font.font_variant_emoji = own
        .inh
        .font
        .font_variant_emoji
        .or(parent.inh.font.font_variant_emoji);
    c.inh.font.font_synth_position = own
        .inh
        .font
        .font_synth_position
        .or(parent.inh.font.font_synth_position);
    c.inh.font.font_kerning = own.inh.font.font_kerning.or(parent.inh.font.font_kerning);
    c.inh.font.font_alternates = own
        .inh
        .font
        .font_alternates
        .clone()
        .or_else(|| parent.inh.font.font_alternates.clone());
    // `font-feature-settings` наследуется своим значением независимо от
    // `font-variant-*` ребёнка (css-fonts-4 §6.12: `font-variant: none` «does
    // not reset … font-feature-settings»).
    c.inh.font.font_settings = own
        .inh
        .font
        .font_settings
        .clone()
        .or_else(|| parent.inh.font.font_settings.clone());
    c.inh.text_ink.text_shadow = if own.reset_text_box.text_shadow_none {
        None
    } else {
        own.inh
            .text_ink
            .text_shadow
            .clone()
            .or_else(|| parent.inh.text_ink.text_shadow.clone())
    };
    // Хвост списка идёт вместе с первой тенью: своя запись — свой хвост,
    // унаследованная — хвост родителя (css-text-decor-3: `text-shadow`
    // наследуется списком целиком).
    if own.reset_text_box.text_shadow_none {
        c.inh.text_ink.text_shadow_rest.clear();
    } else if own.inh.text_ink.text_shadow.is_none() {
        c.inh.text_ink.text_shadow_rest = parent.inh.text_ink.text_shadow_rest.clone();
    }
    c.inh.writing.rtl = own.inh.writing.rtl.or(parent.inh.writing.rtl);
    // `text-align: start|end` — края СТРОКИ, и разворачиваются они в момент
    // ОТРИСОВКИ, а не здесь: иначе левый край, вычисленный для тела страницы,
    // достаётся по наследству и вложенному блоку справа налево (`physical`
    // зовёт `lines::align_for`). Умолчание CSS — `start`.
    c.inh.text_flow.text_align = Some(c.inh.text_flow.text_align.unwrap_or(TextAlign::Start));
}
