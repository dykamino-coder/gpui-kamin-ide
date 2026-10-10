//! inherit_stage, этап шрифта и строки: кегль и высота строки при font-size-adjust, начертание, выравнивание текста, письмо, сетка родителя, интервалы, text-transform/indent/box, разрывы.

use super::*;

pub(super) fn inherit_fonts(parent: &Computed, own: &Computed, c: &mut Computed) {
    // `c` — клон `own`, но `lh` в собственном кегле уже решён выше от строки
    // РОДИТЕЛЯ (css-values-4 §6.1.1, `from_parent`). Брать `own` здесь значило
    // вернуть сырое `Len::Lh` — кегль терялся (`lh-unit-002`).
    // Детям уходит ВЫЧИСЛЕННЫЙ кегль, а не подогнанный `font-size-adjust`:
    // «child elements inherit the computed font-size value (otherwise, the
    // effect of font-size-adjust would compound)» (css-fonts-4 §2.5).
    c.font_size = c.font_size.or(match parent.font_adjust_base {
        Some((px, _)) => Some(Len::Px(px)),
        None => parent.font_size,
    });
    // Кегль НОЛЬ вешает набор намертво (DirectWrite-цикл: `font: 0 Ahem` из
    // vars-font-shorthand-001 замораживал страницу навсегда) — клэмп к
    // микроскопическому: визуально то же «ничего», формулы живы.
    if let Some(Len::Px(v)) = c.font_size
        && v <= 0.0
    {
        c.font_size = Some(Len::Px(0.01));
    }
    crate::style::computed::font_weight::inherit(c, parent, own);
    c.italic = own.italic.or(parent.italic);
    c.oblique = own.oblique.or(parent.oblique);
    c.underline = own.underline.or(parent.underline);
    c.line_through = own.line_through.or(parent.line_through);
    // То же для высоты строки: `line-height: 2lh` уже переведён в точки от
    // строки родителя; сырое `Lh` уходило в `apply.rs` как `relative(2)` от
    // СВОЕГО кегля (`lh-unit-001`: 84 вместо 100).
    // У подогнанного родителя числовой `line-height` переведён в точки ЕГО
    // вычисленным кеглем; ребёнок наследует сам множитель.
    c.line_height = c.line_height.or(match parent.font_adjust_base {
        Some((_, lh)) => lh,
        None => parent.line_height,
    });
    c.text_align = own.text_align.or(parent.text_align);
    c.no_justify = own.no_justify.or(parent.no_justify);
    c.ruby_justify = own.ruby_justify.or(parent.ruby_justify);
    c.justify_chars = own.justify_chars.or(parent.justify_chars);
    c.ruby_unit = own.ruby_unit || parent.ruby_unit;
    c.text_align_last = own.text_align_last.or(parent.text_align_last);
    match_parent_align(parent, own, c);
    c.hanging = own.hanging.or(parent.hanging);
    c.monospace = own.monospace.or(parent.monospace);
    crate::style::computed::font_family::inherit(c, own, parent);
    c.nowrap = own.nowrap.or(parent.nowrap);
    c.orphans = own.orphans.or(parent.orphans);
    c.widows = own.widows.or(parent.widows);
    // Направление письма наследуется: `writing-mode` ставят на `body`, а ось
    // потока обязана смениться у КАЖДОГО вложенного блока — иначе вертикально
    // становится только сам `body`, а его дети снова текут вниз.
    c.vertical = own.vertical.or(parent.vertical);
    c.vertical_rl = own.vertical_rl.or(parent.vertical_rl);
    c.sideways = own.sideways.or(parent.sideways);
    c.combine_upright = own.combine_upright.or(parent.combine_upright);
    c.rotated_line = own.rotated_line.or(parent.rotated_line);
    c.ortho_limit = own.ortho_limit.or(parent.ortho_limit);
    c.orthogonal_scrollport = parent.orthogonal_scrollport;
    // An absolutely positioned box sizes its inline axis against its own
    // containing block (CSS 2.1 §10.3.7 / §10.6.4 shrink-to-fit), never with
    // the in-flow inline measure of the paragraph it was written in.
    c.orthogonal_inline = if matches!(
        own.position,
        Some(crate::style::computed::Position::Absolute)
            | Some(crate::style::computed::Position::Fixed)
    ) {
        None
    } else {
        parent.orthogonal_inline
    };
    c.wrap_anywhere = own.wrap_anywhere.or(parent.wrap_anywhere);
    c.word_space_char = own.word_space_char.or(parent.word_space_char);
    c.autospace_alpha = own.autospace_alpha.or(parent.autospace_alpha);
    c.autospace_numeric = own.autospace_numeric.or(parent.autospace_numeric);
    // Сдвиг НЕ наследуется: он принадлежит своему куску, иначе надстрочный
    // знак поднимал бы весь текст после себя.
    c.vertical_shift = own.vertical_shift;
    c.vertical_shift_px = own.vertical_shift_px;
    c.vertical_shift_len = own.vertical_shift_len;
    c.vertical_align_text = own.vertical_align_text;
    // Кегль родителя нужен `text-top`/`text-bottom`: край куска равняется по
    // ЕГО текстовой области.
    c.vertical_align_base = match parent.font_size {
        Some(crate::style::values::value::Len::Px(v)) => Some(v),
        _ => own.vertical_align_base,
    };
    c.upright = own.upright.or(parent.upright);
    c.text_sideways = own.text_sideways.or(parent.text_sideways);
    // Сетка-родитель и её письмо: у вертикальной сетки оси выравнивания
    // элемента переставляются (`apply.rs`), а по оси x идут группы базовых.
    c.parent_grid = match parent.display {
        Some(crate::style::computed::Display::Grid)
        | Some(crate::style::computed::Display::InlineGrid) => {
            match (
                parent.vertical == Some(true),
                parent.vertical_rl == Some(true),
            ) {
                (false, _) => 1,
                (true, false) => 2,
                (true, true) => 3,
            }
        }
        _ => 0,
    };
    c.parent_lanes = parent.display == Some(crate::style::computed::Display::GridLanes);
    c.parent_subgrid = c.parent_grid != 0 && (parent.subgrid_cols || parent.subgrid_rows);
    c.parent_flex_grid = matches!(
        parent.display,
        Some(crate::style::computed::Display::Flex)
            | Some(crate::style::computed::Display::InlineFlex)
            | Some(crate::style::computed::Display::Grid)
            | Some(crate::style::computed::Display::InlineGrid)
            | Some(crate::style::computed::Display::GridLanes)
    );
    // Наследуемые текстовые свойства из второй волны разбора. Без них
    // `text-transform` на контейнере не доходил до вложенного текста —
    // а в разметке его ставят именно на контейнер.
    c.letter_spacing = own.letter_spacing.or(parent.letter_spacing);
    c.word_spacing = own.word_spacing.or(parent.word_spacing);
    // Сторона подписи таблицы наследуется (CSS 2.1: caption-side inherited) —
    // читается потом С САМОГО заголовка (caption-side-applies-to-012..015:
    // значение на ряде до заголовка не доходит).
    c.caption_bottom = own.caption_bottom.or(parent.caption_bottom);
    c.text_transform = own.text_transform.or(parent.text_transform);
    // Добавки — часть того же значения: своё объявление заменяет их целиком.
    c.text_transform_flags = if own.text_transform.is_some() {
        own.text_transform_flags
    } else {
        parent.text_transform_flags
    };
    c.text_indent = own.text_indent.or(parent.text_indent);
    // `text-box-edge` наследуется (css-inline-3 §text-box-edge, Inherited:
    // yes); срез берёт край у корневой строчной коробки СТРОКИ, то есть у
    // блока, которому она принадлежит (`text-box-trim-accumulation-001…003`).
    if !own.text_box_edge_set {
        c.text_box_over = parent.text_box_over;
        c.text_box_under = parent.text_box_under;
        c.text_box_edge_set = parent.text_box_edge_set;
    }
    c.text_indent_each_line = own.text_indent_each_line.or(parent.text_indent_each_line);
    c.text_indent_hanging = own.text_indent_hanging.or(parent.text_indent_hanging);
    c.break_anywhere = own.break_anywhere.or(parent.break_anywhere);
    c.break_word = own.break_word.or(parent.break_word);
    c.balance_lines = own.balance_lines.or(parent.balance_lines);
    c.break_anywhere_strict = own.break_anywhere_strict.or(parent.break_anywhere_strict);
    c.line_break_loose = own.line_break_loose.or(parent.line_break_loose);
}

/// `match-parent`: «the inherited value of start or end is interpreted against the parent's
/// direction» (css-text-3 §text-align). У корня родителя нет — там это `start` по своему письму.
/// Последняя строка родителя с `auto` следует его `text-align-all` (`justify` → `start`).
fn match_parent_align(parent: &Computed, own: &Computed, c: &mut Computed) {
    if own.text_align_match_parent == 0 || (parent.rtl.is_none() && parent.text_align.is_none()) {
        return;
    }
    let rtl = parent.rtl == Some(true);
    let resolve = |a: Option<TextAlign>| {
        Some(match a.unwrap_or(TextAlign::Start) {
            TextAlign::Start if rtl => TextAlign::Right,
            TextAlign::Start => TextAlign::Left,
            TextAlign::End if rtl => TextAlign::Left,
            TextAlign::End => TextAlign::Right,
            other => other,
        })
    };
    if own.text_align_match_parent & 1 != 0 {
        c.text_align = resolve(parent.text_align);
    }
    if own.text_align_match_parent & 2 != 0 {
        let last = parent.text_align_last.or(match parent.text_align {
            Some(TextAlign::Justify) => Some(TextAlign::Start),
            a => a,
        });
        c.text_align_last = resolve(last);
    }
}
