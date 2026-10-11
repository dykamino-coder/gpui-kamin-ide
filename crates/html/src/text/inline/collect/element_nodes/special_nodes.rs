//! Special nodes for element_nodes; split out to keep the owning module within 250 lines.

use super::collect_with_empty_metrics;
use crate::dom::Element;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::text::inline::*;

pub(super) fn special_node(
    e: &Element,
    inherited: &Computed,
    atom: &mut dyn FnMut(&Element) -> Option<Piece>,
    has_text: bool,
    case: &mut text_case::Context,
    out: &mut Vec<Piece>,
) -> bool {
    // `display: contents` on `<br>`/`<wbr>` behaves as
    // `display: none` (css-display-3 §B «Unusual Elements»): no
    // line break, no break opportunity
    // (`display-contents-sharing-001`).
    if matches!(e.tag.as_str(), "br" | "wbr")
        && e.style.display == Some(crate::style::computed::Display::Contents)
    {
        return true;
    }
    if e.tag == "br" {
        case.boundary();
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
        style.word_space_char = e.style.word_space_char.or(style.word_space_char);
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
    // текстовые свойства наследованием. Блочный путь это знает
    // (`render.rs:4358`), а строчный сбор вёл такой элемент обычным
    // `<span>`, и рамка рисовалась прогоном
    // (`display-contents-inline-001` «красное видно»).
    if e.style.display == Some(crate::style::computed::Display::Contents) {
        let merged = inherit(inherited, &e.style);
        out.extend(collect_with_empty_metrics(
            &e.children,
            &merged,
            atom,
            has_text,
            case,
        ));
        return true;
    }
    false
}
