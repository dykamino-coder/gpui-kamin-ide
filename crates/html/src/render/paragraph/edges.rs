//! Краевые inline pieces и определение текстового потока.

use crate::dom::Node;
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::inline;
use crate::text::text_box::normal_fraction;

/// Свой кегль абзаца в точках — точка отсчёта для строки-опоры и для долей.
///
/// Отсчёт идёт от СВОЕГО кегля, а не от базового кегля документа: у коробки с
/// `font-size: 10px` строка обязана быть в 10 точек, а базовый (16) держал её
/// вдвое выше.
pub(crate) fn own_size(inherited: &Computed, opts: &RenderOpts) -> f32 {
    match inherited.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) | Some(Len::Pct(k)) => k * opts.base_size(),
        _ => opts.base_size(),
    }
}

/// Есть ли в абзаце текст ПОМИМО внепоточных кусков.
///
/// Это ровно условие, при котором абзацу доступен ТЕКСТОВЫЙ путь: текст для
/// него собирает `inline::text_and_runs` из кусков `Piece::Text`, а
/// внепоточный кусок в него не входит (`inline.rs:2539`), и на пустом тексте
/// путь закрыт (`inline.rs:2543`). Внепоточный — и абсолют на статической
/// позиции, и абсолют с краями: оба уходят `Piece::Overlay`, оба своего
/// текста в строку не отдают.
pub(crate) fn has_flow_text(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Text(t) => !t.trim().is_empty(),
        Node::Element(e) => {
            !matches!(
                e.style.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            ) && has_flow_text(&e.children)
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
    if inherited.vertical == Some(true) {
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
        let out_of_flow = style.float.is_some_and(|f| f != 0)
            || matches!(
                style.position,
                Some(crate::style::computed::Position::Absolute)
                    | Some(crate::style::computed::Position::Fixed)
            );
        if let Some(top) = top
            && style.vertical_align != inherited.vertical_align
            && !out_of_flow
            && !text.is_empty()
        {
            let size = match style.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * opts.base_size(),
                _ => own_size(inherited, opts),
            };
            let h = match style.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
                _ => size * normal_fraction(style, opts),
            };
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
