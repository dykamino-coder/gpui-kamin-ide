//! Стадия вычисленных значений, шрифт и строка: кегль и высота строки при font-size-adjust,
//! начертание, выравнивание текста (`match-parent`), письмо, интервалы, text-transform/indent/box,
//! разрывы.

use super::*;

pub(super) fn inherit_fonts(parent: &Computed, own: &Computed, c: &mut Computed) {
    // `c` — клон `own`, но `lh` в собственном кегле уже решён выше от строки
    // РОДИТЕЛЯ (css-values-4 §6.1.1, `from_parent`). Брать `own` здесь значило
    // вернуть сырое `Len::Lh` — кегль терялся (`lh-unit-002`).
    // Детям уходит ВЫЧИСЛЕННЫЙ кегль, а не подогнанный `font-size-adjust`:
    // «child elements inherit the computed font-size value (otherwise, the
    // effect of font-size-adjust would compound)» (css-fonts-4 §2.5).
    c.inh.font.font_size = c.inh.font.font_size.or(parent.inh.font.font_size);
    // Кегль НОЛЬ вешает набор намертво (DirectWrite-цикл: `font: 0 Ahem` из
    // vars-font-shorthand-001 замораживал страницу навсегда) — клэмп к
    // микроскопическому: визуально то же «ничего», формулы живы.
    // TRANSITIONAL(cascade-7b-R11): Клэмп font-size:0 до 0.01 меняет computed значение, em и наследование ради text backend; удалить после безопасного shaping нулевого размера на границе backend.
    if let Some(Len::Px(v)) = c.inh.font.font_size
        && v <= 0.0
    {
        c.inh.font.font_size = Some(Len::Px(0.01));
    }
    crate::style::computed::font_weight::inherit(c, parent, own);
    c.inh.font.italic = own.inh.font.italic.or(parent.inh.font.italic);
    c.inh.font.oblique = own.inh.font.oblique.or(parent.inh.font.oblique);
    c.inh.font.font_oblique_angle = if own.inh.font.oblique.is_some() {
        own.inh.font.font_oblique_angle
    } else {
        parent.inh.font.font_oblique_angle
    };
    // То же для высоты строки: `line-height: 2lh` уже переведён в точки от
    // строки родителя; сырое `Lh` уходило в `paint/apply/` как `relative(2)` от
    // СВОЕГО кегля (`lh-unit-001`: 84 вместо 100).
    c.inh.font.line_height = c.inh.font.line_height.or(parent.inh.font.line_height);
    c.inh.text_flow.text_align = own
        .inh
        .text_flow
        .text_align
        .or(parent.inh.text_flow.text_align);
    c.inh.text_flow.no_justify = own
        .inh
        .text_flow
        .no_justify
        .or(parent.inh.text_flow.no_justify);
    c.inh.text_flow.ruby_justify = own
        .inh
        .text_flow
        .ruby_justify
        .or(parent.inh.text_flow.ruby_justify);
    c.inh.text_flow.justify_chars = own
        .inh
        .text_flow
        .justify_chars
        .or(parent.inh.text_flow.justify_chars);
    c.inh.text_flow.text_align_last = own
        .inh
        .text_flow
        .text_align_last
        .or(parent.inh.text_flow.text_align_last);
    c.inh.ui.image_pixelated = own.inh.ui.image_pixelated.or(parent.inh.ui.image_pixelated);
    match_parent_align(parent, own, c);
    c.inh.text_flow.hanging = own.inh.text_flow.hanging.or(parent.inh.text_flow.hanging);
    c.inh.font.monospace = own.inh.font.monospace.or(parent.inh.font.monospace);
    crate::style::computed::font_family::inherit(c, own, parent);
    c.inh.text_flow.nowrap = own.inh.text_flow.nowrap.or(parent.inh.text_flow.nowrap);
    c.inh.text_flow.orphans = own.inh.text_flow.orphans.or(parent.inh.text_flow.orphans);
    c.inh.text_flow.widows = own.inh.text_flow.widows.or(parent.inh.text_flow.widows);
    c.inh.writing.combine_upright = own
        .inh
        .writing
        .combine_upright
        .or(parent.inh.writing.combine_upright);
    c.inh.text_flow.wrap_anywhere = own
        .inh
        .text_flow
        .wrap_anywhere
        .or(parent.inh.text_flow.wrap_anywhere);
    c.inh.text_flow.word_space_char = own
        .inh
        .text_flow
        .word_space_char
        .or(parent.inh.text_flow.word_space_char);
    c.inh.text_flow.autospace_alpha = own
        .inh
        .text_flow
        .autospace_alpha
        .or(parent.inh.text_flow.autospace_alpha);
    c.inh.text_flow.autospace_numeric = own
        .inh
        .text_flow
        .autospace_numeric
        .or(parent.inh.text_flow.autospace_numeric);
    // Кегль родителя нужен `text-top`/`text-bottom`: край куска равняется по
    // ЕГО текстовой области.
    c.reset_text_box.vertical_align_base = match parent.inh.font.font_size {
        Some(crate::style::values::value::Len::Px(v)) => Some(v),
        _ => own.reset_text_box.vertical_align_base,
    };
    // Наследуемые текстовые свойства из второй волны разбора. Без них
    // `text-transform` на контейнере не доходил до вложенного текста —
    // а в разметке его ставят именно на контейнер.
    c.inh.text_flow.letter_spacing = own
        .inh
        .text_flow
        .letter_spacing
        .or(parent.inh.text_flow.letter_spacing);
    c.inh.text_flow.word_spacing = own
        .inh
        .text_flow
        .word_spacing
        .or(parent.inh.text_flow.word_spacing);
    // Сторона подписи таблицы наследуется (CSS 2.1: caption-side inherited) —
    // читается потом С САМОГО заголовка (caption-side-applies-to-012..015:
    // значение на ряде до заголовка не доходит).
    c.inh.table_list.caption_bottom = own
        .inh
        .table_list
        .caption_bottom
        .or(parent.inh.table_list.caption_bottom);
    c.inh.text_flow.text_transform = own
        .inh
        .text_flow
        .text_transform
        .or(parent.inh.text_flow.text_transform);
    // Добавки — часть того же значения: своё объявление заменяет их целиком.
    c.inh.text_flow.text_transform_flags = if own.inh.text_flow.text_transform.is_some() {
        own.inh.text_flow.text_transform_flags
    } else {
        parent.inh.text_flow.text_transform_flags
    };
    c.inh.text_flow.text_indent = own
        .inh
        .text_flow
        .text_indent
        .or(parent.inh.text_flow.text_indent);
    // `text-box-edge` наследуется (css-inline-3 §text-box-edge, Inherited:
    // yes); срез берёт край у корневой строчной коробки СТРОКИ, то есть у
    // блока, которому она принадлежит (`text-box-trim-accumulation-001…003`).
    if !own.inh.text_flow.text_box_edge_set {
        c.inh.text_flow.text_box_over = parent.inh.text_flow.text_box_over;
        c.inh.text_flow.text_box_under = parent.inh.text_flow.text_box_under;
        c.inh.text_flow.text_box_edge_set = parent.inh.text_flow.text_box_edge_set;
    }
    c.inh.text_flow.text_indent_each_line = own
        .inh
        .text_flow
        .text_indent_each_line
        .or(parent.inh.text_flow.text_indent_each_line);
    c.inh.text_flow.text_indent_hanging = own
        .inh
        .text_flow
        .text_indent_hanging
        .or(parent.inh.text_flow.text_indent_hanging);
    c.inh.text_flow.break_anywhere = own
        .inh
        .text_flow
        .break_anywhere
        .or(parent.inh.text_flow.break_anywhere);
    c.inh.text_flow.break_word = own
        .inh
        .text_flow
        .break_word
        .or(parent.inh.text_flow.break_word);
    c.inh.text_flow.word_break_word = own
        .inh
        .text_flow
        .word_break_word
        .or(parent.inh.text_flow.word_break_word);
    c.inh.text_flow.auto_phrase = own
        .inh
        .text_flow
        .auto_phrase
        .or(parent.inh.text_flow.auto_phrase);
    c.inh.text_flow.balance_lines = own
        .inh
        .text_flow
        .balance_lines
        .or(parent.inh.text_flow.balance_lines);
    c.inh.text_flow.break_anywhere_strict = own
        .inh
        .text_flow
        .break_anywhere_strict
        .or(parent.inh.text_flow.break_anywhere_strict);
    c.inh.text_flow.line_break_loose = own
        .inh
        .text_flow
        .line_break_loose
        .or(parent.inh.text_flow.line_break_loose);
}

/// Письмо потребителя нужно до вычисления cqi/cqb, в том числе в кегле.
pub(super) fn inherit_writing_mode(parent: &Computed, own: &Computed, c: &mut Computed) {
    let wm = crate::style::computed::writing_mode::WritingProps::of(own).inherit(
        crate::style::computed::writing_mode::WritingProps::of(parent),
    );
    (
        c.inh.writing.writing_mode,
        c.inh.writing.flat_mode,
        c.inh.writing.text_orientation,
    ) = (wm.writing_mode, wm.flat_mode, wm.text_orientation);
}

/// `match-parent`: «the inherited value of start or end is interpreted against the parent's
/// direction» (css-text-3 §text-align). У корня родителя нет — там это `start` по своему письму.
/// Последняя строка родителя с `auto` следует его `text-align-all` (`justify` → `start`).
fn match_parent_align(parent: &Computed, own: &Computed, c: &mut Computed) {
    if own.reset_text_box.text_align_match_parent == 0
        || (parent.inh.writing.rtl.is_none() && parent.inh.text_flow.text_align.is_none())
    {
        return;
    }
    let rtl = parent.wm().rtl();
    let resolve = |a: Option<TextAlign>| {
        Some(match a.unwrap_or(TextAlign::Start) {
            TextAlign::Start if rtl => TextAlign::Right,
            TextAlign::Start => TextAlign::Left,
            TextAlign::End if rtl => TextAlign::Left,
            TextAlign::End => TextAlign::Right,
            other => other,
        })
    };
    if own.reset_text_box.text_align_match_parent & 1 != 0 {
        c.inh.text_flow.text_align = resolve(parent.inh.text_flow.text_align);
    }
    if own.reset_text_box.text_align_match_parent & 2 != 0 {
        let last = parent.inh.text_flow.text_align_last.flatten().or(
            match parent.inh.text_flow.text_align {
                Some(TextAlign::Justify) => Some(TextAlign::Start),
                a => a,
            },
        );
        c.inh.text_flow.text_align_last = Some(resolve(last));
    }
}
