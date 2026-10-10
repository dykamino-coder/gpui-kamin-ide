//! Element nodes for collect; split out to keep the owning module within 250 lines.

mod special_nodes;
use special_nodes::special_node;

mod blank_box;
use crate::text::inline::collect::element_nodes::blank_box::blank_box;

mod box_paint;
use crate::text::inline::collect::element_nodes::box_paint::element_style;

use super::collect_with_empty_metrics;
use crate::dom::Element;
use crate::style::computed::Computed;
use crate::text::inline::collect::boundary_spacing::boundary_gap_after_box;
use crate::text::inline::collect::boundary_spacing::set_boundary_spacing;
use crate::text::inline::collect::overlays::shift_overlays;
use crate::text::inline::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_element(
    e: &Element,
    inherited: &Computed,
    atom: &mut dyn FnMut(&Element) -> Option<Piece>,
    has_text: bool,
    case: &mut text_case::Context,
    out: &mut Vec<Piece>,
    gap_at: &mut Option<usize>,
    from: usize,
) {
    if special_node(e, inherited, atom, has_text, case, out) {
        return;
    }
    // Атому сообщают, лежит ли он в позиционированном строчном
    // (`take_atom_cb`); его собственное содержимое — уже вне его.
    let depth = INLINE_CB_DEPTH.with(|d| d.replace(0));
    ATOM_CB.with(|c| c.set(depth > 0));
    let built = atom(e);
    ATOM_CB.with(|c| c.set(false));
    INLINE_CB_DEPTH.with(|d| d.set(depth));
    if let Some(piece) = built {
        if matches!(piece, Piece::Atom(_)) {
            case.boundary();
        }
        out.push(piece);
        // Строчный `<span>`, ушедший в свою коробку (узорный фон,
        // `has_own_box`), атомом в CSS не является: зазор между его
        // последним знаком и следующим — ПРЕДКА (css-text-3 §8.2),
        // а внутренний абзац коробки последний знак не трекует.
        // Без распорки следующее слово вставало вплотную
        // (`letter-spacing-nesting-003`). Хвостовая распорка у
        // конца узла снимается ниже — за ней границы нет.
        if let Some(gap) = boundary_gap_after_box(e, inherited) {
            *gap_at = Some(out.len());
            out.push(Piece::Text {
                text: SPACER.into(),
                style: spacer_style(inherited, gap),
            });
        }
        return;
    }
    let (mut merged, painted_bg) = element_style(e, inherited);
    let atomic = matches!(
        e.style.display,
        Some(crate::style::computed::Display::InlineBlock)
            | Some(crate::style::computed::Display::InlineFlex)
            | Some(crate::style::computed::Display::InlineGrid)
            | Some(crate::style::computed::Display::InlineTable)
    ) && e.style.inline_display != Some(true);
    // Ограничитель атомарной коробки — служебный знак, а не текст
    // документа: замена нулевого пробела идеографическим его
    // касаться не должна, иначе вокруг `inline-block` появляется
    // полноширинный пробел, которого в разметке нет.
    let mut marker = merged.clone();
    marker.word_space_char = None;
    if atomic {
        out.push(Piece::Text {
            text: "\u{200b}".into(),
            style: marker.clone(),
        });
    }
    // Боковые поля, рамки и отступы СТРОЧНОЙ коробки: своей
    // коробки в раскладке у неё нет, поэтому место занимает
    // невидимый знак-распорка. Соединитель слов (U+FEFF) выбран
    // не случайно: точкой переноса он не является, а нулевой
    // пробел ею был бы — строка рвалась бы по краю `<span>`.
    // CSS Writing Modes 4 §2.4, bidi-fragment-boxes: physical edges
    // follow the parent's direction, not the inline's own embedding.
    // Spacers sit outside the inline's bidi controls, in that parent.
    let ((mut mlead, mut mtrail), (mut lead, mut trail)) =
        inline_spacing::inline_sides(e, &merged, inherited);
    if inherited.rtl == Some(true) {
        std::mem::swap(&mut lead, &mut trail);
        std::mem::swap(&mut mlead, &mut mtrail);
    }
    // CSS 2.1 sections 8.4 and 14.2: padding belongs to this
    // inline, even when a descendant supplies a different background.
    // Its advance is already carried by the edge spacers; paint those
    // advances instead of extending a descendant's text band into them.
    let painted_padding = merged.inline_bg.is_some() && merged.inline_border.is_none();
    if painted_padding && let Some(pad) = &mut merged.inline_pad {
        pad[1] = 0.0;
        pad[3] = 0.0;
    }
    // Box identity of the edge spacers: the line painter moves them
    // to the outermost visual fragments after bidi reordering.
    let parent_rtl = inherited.rtl == Some(true);
    let box_id = if mlead != 0.0 || lead != 0.0 || trail != 0.0 || mtrail != 0.0 {
        SPACER_BOX.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    } else {
        0
    };
    let edge = |mut style: Computed, leading: bool| {
        style.spacer_edge = Some((box_id, leading != parent_rtl, parent_rtl));
        style
    };
    if mlead != 0.0 {
        out.push(Piece::Text {
            text: SPACER.into(),
            style: edge(margin_spacer_style(&merged, inherited, mlead), true),
        });
    }
    let blank = blank_box(e, &merged, inherited, painted_bg, lead, trail, out);
    if lead != 0.0 {
        out.push(Piece::Text {
            text: SPACER.into(),
            style: edge(padding_spacer_style(&merged, lead, painted_padding), true),
        });
    }
    if blank
        && has_text
        && !atomic
        && lead == 0.0
        && trail == 0.0
        && empty_inline::different_metrics(&merged, inherited)
    {
        // CSS 2.1 section 10.8: an empty inline box contributes
        // its line height and font metrics even without glyphs.
        out.push(Piece::Text {
            text: SPACER.into(),
            style: spacer_style(&merged, 0.0),
        });
    }
    // Своя сторона письма у куска — это знаки управления по
    // Юникоду: разбор двунаправленности их и ждёт, а рисовать их
    // не надо, ширины у них нет.
    let (open, close) = bidi_marks(&e.style, &merged);
    // Zero-length markers bound the box content between its edge
    // spacers (`box_extents`); they add no text.
    let box_marker = |start: bool| Piece::Text {
        text: String::new(),
        style: Computed {
            spacer_edge: Some((box_id, start, parent_rtl)),
            ..merged.clone()
        },
    };
    if box_id != 0 {
        out.push(box_marker(true));
    }
    if let Some(mark) = open {
        out.push(Piece::Text {
            text: mark.to_string(),
            style: merged.clone(),
        });
    }
    // Относительный сдвиг строчного куска несёт и его потомков вне
    // потока: абсолютный элемент внутри `position: relative`
    // спана стоит от СДВИНУТОГО места (`static-position/htb-*`).
    let cb_here = establishes_cb(&e.style);
    if cb_here {
        INLINE_CB_DEPTH.with(|d| d.set(d.get() + 1));
    }
    let kids = collect_with_empty_metrics(&e.children, &merged, atom, has_text, case);
    let kids = if cb_here {
        INLINE_CB_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
        mark_inline_cb(kids, e)
    } else {
        kids
    };
    out.extend(shift_overlays(
        kids,
        &e.style,
        merged.rotated_line == Some(true),
    ));
    if let Some(mark) = close {
        out.push(Piece::Text {
            text: mark.to_string(),
            style: merged.clone(),
        });
    }
    if box_id != 0 {
        out.push(box_marker(false));
    }
    if trail != 0.0 {
        out.push(Piece::Text {
            text: SPACER.into(),
            style: edge(padding_spacer_style(&merged, trail, painted_padding), false),
        });
    }
    if mtrail != 0.0 {
        out.push(Piece::Text {
            text: SPACER.into(),
            style: edge(margin_spacer_style(&merged, inherited, mtrail), false),
        });
    }
    if atomic {
        out.push(Piece::Text {
            text: "\u{200b}".into(),
            style: marker,
        });
    }
    // Зазор между последним знаком этого элемента и первым знаком
    // того, что идёт следом, лежит ВНУТРИ текущего узла — значит,
    // и величина его отсюда (css-text-3 §8.2). Ставится всегда, а
    // снимается ниже с последнего куска: за ним границы уже нет.
    set_boundary_spacing(&mut out[from..], inherited.letter_spacing);
}
