//! inherit_stage, этап текста и выравнивания: align-self normal у элементов flex/grid, emphasis, ruby, переносы, списки, line-clamp, -webkit-box, инлайн-оформление, font-feature-*, тени текста, rtl, text-align.

use super::*;

pub(super) fn inherit_text_layout(parent: &Computed, own: &Computed, c: &mut Computed) {
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): снимать `align-self` у коробки, чей
    // родитель не гибкий контейнер и не сетка (css-align-3 §6.2 «does not
    // apply to block-level boxes»). По спеке верно, но наш блок собран
    // колонкой flex, и на `align_self` держатся собственные приёмы сборки:
    // соотношение сторон блока (`blocks()` ставит `Align::Start`), обтекание,
    // сжатие стола. Узкий срез выравнивания (2661 пара) дал +8/−1, а ПОЛНЫЙ
    // свод v18 -> v19 — минус ~50: вся семья `float-applies-to-*` (0.00 ->
    // 3.84), `floats-002/025/147`, `clear-float-001/003`,
    // `block-aspect-ratio-002/015/016/018/043/047`, девять
    // `shape-outside-*-border-radius-*`, `absolute-replaced-width-020/034`.
    // Возвращать вместе с признаком «значение авторское», чтобы приёмы сборки
    // гейт не задевал (`self-align-start-end-flex-001` — цель правки).
    // `normal` у элемента ГИБКОГО контейнера = `stretch` (§6.2), а не
    // «пусто»: пустое значение брало `align-items` родителя
    // (`self-align-normal-flex`).
    if own.align_self_normal
        && matches!(
            parent.display,
            Some(crate::style::computed::Display::Flex)
                | Some(crate::style::computed::Display::InlineFlex)
        )
    {
        c.align_self = Some(crate::style::computed::Align::Stretch);
    }
    // `self-start`/`self-end` меряются по письму САМОГО элемента (css-align-3
    // §6.2). Значение уже физическое (начало = левый край при ltr), поэтому
    // зеркалим ровно тогда, когда строчная ось элемента смотрит в другую
    // сторону, чем у родителя: `flexbox-align-self-vert-002` даёт элементам
    // `direction: rtl` внутри ltr-колонки и ждёт `self-start` СПРАВА.
    // Вертикальное письмо здесь НЕ зеркалим намеренно: там ось строки уже
    // переставлена поворотом — это территория wm-скаута.
    // Письмо элемента — ДЕЙСТВУЮЩЕЕ (своё или унаследованное): незаданное
    // `direction` у элемента в rtl-колонке — тоже rtl, и зеркалить нечего
    // (поперечную ось rtl-колонки разворачивает сама раскладка,
    // `apply.rs`: `flex_cross_reverse` / `flip`).
    if c.parent_grid == 0
        && own.align_self_own_axis
        && own.rtl.or(parent.rtl).unwrap_or(false) != parent.rtl.unwrap_or(false)
    {
        c.align_self = match c.align_self {
            Some(crate::style::computed::Align::Start) => Some(crate::style::computed::Align::End),
            Some(crate::style::computed::Align::End) => Some(crate::style::computed::Align::Start),
            other => other,
        };
    }
    // `start`/`end` (и `self-*`) меряются по ПИСЬМУ, а раскладка знает только
    // гибкие концы (`apply::to_items` → `FlexStart`/`FlexEnd`), которые
    // АВТОРСКИЙ `wrap-reverse` родителя переворачивает (css-align-3 §6.1,
    // css-flexbox-1 §5.2). Концы письма меняются местами ровно тогда: taffy
    // развернёт их обратно. Переворот поперёк письма (`apply.rs`: `flip`
    // через тот же `WrapReverse`) здесь не участвует — он виден раскладке и
    // для `flex-*`, и для `start` одинаково (`self-align-start-end-flex-001`).
    if !own.align_self_flex_kw
        && parent.flex_wrap_reverse == Some(true)
        && matches!(
            parent.display,
            Some(crate::style::computed::Display::Flex)
                | Some(crate::style::computed::Display::InlineFlex)
        )
    {
        c.align_self = match c.align_self {
            Some(crate::style::computed::Align::Start) => Some(crate::style::computed::Align::End),
            Some(crate::style::computed::Align::End) => Some(crate::style::computed::Align::Start),
            other => other,
        };
    }
    c.text_emphasis = own.text_emphasis.clone().or(parent.text_emphasis.clone());
    c.emphasis_under = own.emphasis_under || parent.emphasis_under;
    c.emphasis_color = own.emphasis_color.or(parent.emphasis_color);
    // css-ruby-1 §4.1/§4.3: оба свойства наследуемые.
    c.ruby_under = own.ruby_under.or(parent.ruby_under);
    c.ruby_align = own.ruby_align.or(parent.ruby_align);
    c.ruby_overhang = own.ruby_overhang.or(parent.ruby_overhang);
    c.ruby_merge = own.ruby_merge.or(parent.ruby_merge);
    // `image-orientation` наследуется (css-images-3 §5.4, «Inherited: yes»):
    // в наборе его ставят на `body`, а действует он на каждой картинке.
    c.image_orient_none = own.image_orient_none.or(parent.image_orient_none);
    c.font_synth = (
        own.font_synth.0.or(parent.font_synth.0),
        own.font_synth.1.or(parent.font_synth.1),
        own.font_synth.2.or(parent.font_synth.2),
    );
    c.keep_all = own.keep_all.or(parent.keep_all);
    c.hyphens_auto = own.hyphens_auto.or(parent.hyphens_auto);
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
    c.vertical_align = own.vertical_align.or(parent.vertical_align);
    c.font_stretch = own.font_stretch.or(parent.font_stretch);
    // `font-size-adjust` наследуется значением; подгонку каждый элемент
    // считает сам, по СВОЕМУ шрифту (`Computed::resolve_em`).
    c.font_size_adjust = own.font_size_adjust.or(parent.font_size_adjust);
    c.no_select = own.no_select.or(parent.no_select);
    c.pointer_events_none = own.pointer_events_none.or(parent.pointer_events_none);
    c.line_clamp = own.line_clamp.or(parent.line_clamp);
    // `block-ellipsis` наследуется (css-overflow-4 §block-ellipsis).
    c.clamp_mark = own.clamp_mark.clone().or_else(|| parent.clamp_mark.clone());
    c.clamp_legacy = own.clamp_legacy.or(parent.clamp_legacy);
    // Гейтовые флаги -webkit-box НЕ наследуются: пара display+orient
    // обязана стоять на самом элементе.
    c.webkit_box = own.webkit_box;
    c.webkit_box_vertical = own.webkit_box_vertical.or(parent.webkit_box_vertical);
    // Фон строчного бокса идёт вниз как текстовое свойство: он принадлежит
    // строке, а не коробке, и вложенный `<b>` внутри подсветки обязан его
    // сохранить.
    c.inline_bg = own.inline_bg.or(parent.inline_bg);
    c.inline_border = own.inline_border.or(parent.inline_border);
    c.inline_pad = own.inline_pad.or(parent.inline_pad);
    c.inline_radius = own.inline_radius.or(parent.inline_radius);
    if c.font_features.is_empty() {
        c.font_features = parent.font_features.clone();
    }
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
        own.text_shadow.or(parent.text_shadow)
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
