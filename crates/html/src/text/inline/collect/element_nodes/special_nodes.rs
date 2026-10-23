//! Элементы без собственной строчной коробки при сборе кусков: `<br>` — принудительный перевод
//! строки (подавляется в колонках руби, как Blink `inline_items_builder.cc`), `<wbr>` — нулевой
//! пробел как точка переноса (HTML §4.5.28), `display: contents` — только дети с наследованием
//! (css-display-3 §2.5; на `<br>`/`<wbr>` — как `none`, §B). Возвращает, обработан ли узел.

use super::collect_with_empty_metrics;
use crate::dom::Element;
use crate::style::cascade::materialize::box_style;
use crate::style::computed::BoxStyle;
use crate::text::inline::*;

pub(super) fn special_node(
    e: &Element,
    inherited: &BoxStyle,
    atom: &mut dyn FnMut(&Element, &crate::text::inline::atom_parent::AtomCx) -> Option<Piece>,
    has_text: bool,
    cx: &mut CollectCx,
    out: &mut Vec<Piece>,
) -> bool {
    let doc = cx.layout.doc().clone();
    let vcx = doc.frame_values();
    // `display: contents` on `<br>`/`<wbr>` behaves as
    // `display: none` (css-display-3 §B «Unusual Elements»): no
    // line break, no break opportunity
    // (`display-contents-sharing-001`).
    if matches!(e.tag.as_str(), "br" | "wbr")
        && e.style.computed().display == Some(crate::style::computed::Display::Contents)
    {
        return true;
    }
    if e.tag == "br" {
        // Blink inline_items_builder.cc:58-62 suppresses forced breaks
        // in ruby columns, including bases, until they can be fragmented.
        if inherited.ruby_suppress_breaks {
            return true;
        }
        cx.case.boundary();
        out.push(Piece::Text {
            text: "\n".into(),
            style: inherited.clone(),
        });
        return true;
    }
    // `<wbr>` — точка переноса без знака, то есть ровно нулевой
    // пробел (HTML §4.5.28). Раньше тег не давал НИЧЕГО, и
    // разрешённого переноса в этом месте не было.
    if e.tag == "wbr" {
        // Замена точки переноса пробелом — по стилю САМОГО `<wbr>`
        // (css-text-4 §word-space-transform: свойство наследуемое,
        // и у элемента своё значение).
        let mut style = inherited.clone();
        style.word_space_char = e.style.cascaded().word_space_char.or(style.word_space_char);
        out.push(Piece::Text {
            text: "\u{200b}".into(),
            style,
        });
        return true;
    }
    // `display: contents` — коробки нет (css-display-3 §2.5 «does
    // not generate any boxes, but its children … still generate
    // boxes and text runs as normal»): ни рамки, ни полей, ни
    // отступов, ни знаков направления — детям уходят только
    // текстовые свойства наследованием. Блочный путь это знает,
    // а строчный сбор вёл такой элемент обычным
    // `<span>`, и рамка рисовалась прогоном
    // (`display-contents-inline-001` «красное видно»).
    if e.style.computed().display == Some(crate::style::computed::Display::Contents) {
        let merged = box_style(inherited, &e.style, vcx);
        out.extend(collect_with_empty_metrics(
            &e.children,
            &merged,
            atom,
            has_text,
            cx,
        ));
        return true;
    }
    false
}
