//! Краевые inline pieces и определение текстового потока.

use crate::dom::Node;
use crate::layout::options::RenderOpts;
use crate::style::computed::Computed;
use crate::text::inline;
use crate::text::text_box::normal_fraction;

pub(crate) use crate::layout::options::own_size;

/// Есть ли в абзаце текст ПОМИМО внепоточных кусков.
///
/// По этому признаку `atom_piece` решает, идёт ли внепоточный кусок
/// горизонтального rtl-абзаца общим текстовым путём (`plain_flow`); абзац из
/// одних внепоточных кусков остаётся фантомным `Paragraph`. Внепоточный — и
/// абсолют на статической позиции, и абсолют с краями: оба уходят
/// `Piece::Overlay`, оба своего текста в строку не отдают.
pub(crate) fn has_flow_text(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Text(t) => !t.trim().is_empty(),
        Node::Element(e) => {
            !crate::style::flow::absolutely_positioned(e.style.used().position)
                && has_flow_text(&e.children)
        }
    })
}

/// Куски текста с `vertical-align: top`/`bottom` (CSS 2.1 §10.8.1): отрезок
/// байт в тексте абзаца, край (`true` — верх) и высота строчной коробки
/// куска — его `line-height`.
///
/// `vertical-align` у нас наследуется (ради ячеек таблицы), поэтому краевым
/// считается только кусок, чьё значение ОТЛИЧАЕТСЯ от значения абзаца: иначе
/// каждый абзац ячейки с `vertical-align: top` прижимался бы весь.
pub(crate) fn edge_pieces(
    pieces: &[inline::Piece],
    inherited: &Computed,
    opts: &RenderOpts,
) -> Vec<(std::ops::Range<usize>, bool, f32)> {
    use crate::style::computed::Align;
    let mut out: Vec<(std::ops::Range<usize>, bool, f32)> = Vec::new();
    // A rotated vertical paragraph is laid out in its pre-rotation frame,
    // whose top is the line-over side (css-writing-modes-4 §line-relative
    // directions), so `top`/`bottom` keep their meaning there.
    if inherited.wm().vertical() {
        return out;
    }
    let mut at = 0usize;
    for p in pieces {
        let inline::Piece::Text { text, style } = p else {
            continue;
        };
        let end = at + text.len();
        let top = match style.vertical_align {
            Some(Align::Start) => Some(true),
            Some(Align::End) => Some(false),
            _ => None,
        };
        let out_of_flow = crate::style::flow::out_of_flow(style);
        if let Some(top) = top
            && style.vertical_align != inherited.vertical_align
            && !out_of_flow
            && !text.is_empty()
        {
            let size = style.used_font(opts.base_size(), opts.cx.vcx()).size_px;
            let h = style.numeric_line_px(opts.base_size()).unwrap_or_else(|| {
                crate::text::metrics::normal_px(normal_fraction(style, opts), size, opts.cx.vcx())
            });
            match out.last_mut() {
                // Соседние куски одного края — одна коробка (`<span>` с
                // вложенными кусками).
                Some((r, t, hh)) if r.end == at && *t == top => {
                    r.end = end;
                    *hh = hh.max(h);
                }
                _ => out.push((at..end, top, h)),
            }
        }
        at = end;
    }
    out
}

/// Лежит ли отрезок внутри краевого куска.
pub(crate) fn in_edge(
    edges: &[(std::ops::Range<usize>, bool, f32)],
    r: &std::ops::Range<usize>,
) -> bool {
    edges
        .iter()
        .any(|(e, _, _)| e.start < r.end.max(r.start + 1) && r.start < e.end)
}
