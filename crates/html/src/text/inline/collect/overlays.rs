//! Static-position probes and relative shifts for overlays in inline rows.

use crate::style::computed::Computed;
use crate::style::values::value::px_lossy_zero;
use crate::text::inline::*;
use gpui::{AnyElement, ParentElement, Styled};

/// Кусок вне потока в РЯДУ: сам абзац его разместить не может (ряд собирает
/// раскладка), поэтому работает прежний обход — нулевая распорка на месте
/// куска и сам элемент в позднем слое, который рисуется от её угла.
///
/// `turned` — ряд лежит в вертикальной строке (`Some(vertical-rl)`): блочная
/// ось там физическая горизонталь, и гипотетическая коробка §10.3.7 начинается
/// у блочного начала строки — при `vertical-rl` это ПРАВЫЙ край
/// (css-writing-modes-4 §7.1; Blink `static_position` с `InsetBias::kEnd`
/// по физической оси x), а не левый, как у горизонтальной строки.
pub(crate) fn overlay_in_row(
    el: AnyElement,
    turned: Option<bool>,
    cx: &crate::layout::cx::LayoutCx,
) -> AnyElement {
    let spot: crate::layout::positioned::containing_block::SpotCell = Default::default();
    if let Some(rl) = turned {
        spot.set(crate::layout::positioned::containing_block::Spot {
            vertical: true,
            vertical_rl: rl,
            ..Default::default()
        });
    }
    let probe =
        crate::layout::positioned::containing_block::spot_probe(spot.clone(), false, None, cx);
    match cx.positioning.late_push(spot, el) {
        None => probe,
        Some(kept) => {
            let mut hole = gpui::div().relative().w_0().h_0().flex_shrink_0();
            hole.style().align_self = Some(gpui::AlignItems::FlexStart);
            hole.child(kept).into_any_element()
        }
    }
}

/// Сдвинуть куски вне потока на относительный сдвиг их строчного предка.
///
/// `rotated` — абзац повёрнутого вертикального письма. Там до-поворотная
/// ось y — БЛОЧНАЯ ось экрана, и сдвиг по ней обязан округляться так же, как
/// глифы соседнего текста: `paint_glyph` кладёт глиф `floor` от физической
/// точки, а раскладка ставит край отбивки `round` (`taffy.rs: layout_bounds`).
/// На 2px при масштабе 1.25 (2.5 точки) коробка вставала на точку дальше
/// текста, и столбец красного в одну точку проступал у всех `cb`-случаев
/// `css-position/static-position/v{lr,rl}-*`. Поэтому сдвиг по y не идёт
/// отбивкой, а копится в `OverlayAt::rot_dy` и прикладывается `lines.rs`
/// одним округлением вместе с местом в строке.
pub(super) fn shift_overlays(pieces: Vec<Piece>, style: &Computed, rotated: bool) -> Vec<Piece> {
    if style.position != Some(crate::style::computed::Position::Relative) {
        return pieces;
    }
    let px_of = px_lossy_zero;
    let (dx, dy) = (
        px_of(style.inset.left) - px_of(style.inset.right),
        px_of(style.inset.top) - px_of(style.inset.bottom),
    );
    if dx == 0.0 && dy == 0.0 {
        return pieces;
    }
    // Rotated paragraphs are typeset horizontally and turned afterwards: express
    // the physical relative offset in the pre-rotation frame, as the text runs do.
    let (rx, ry) = if rotated {
        to_rotated_frame(style, (dx, dy))
    } else {
        (dx, dy)
    };
    pieces
        .into_iter()
        .map(|p| match p {
            // Абсолют от строчного содержащего блока: сдвигается сам блок, и
            // обёртка-отбивка встала бы его родителем (`lines.rs`).
            Piece::Overlay(el, at) if at.cb.is_some() => Piece::Overlay(
                el,
                OverlayAt {
                    cb: at.cb.map(|c| InlineCb {
                        shift: (c.shift.0 + dx, c.shift.1 + dy),
                        ..c
                    }),
                    ..at
                },
            ),
            // Метка строчной коробки-якоря копит сдвиг в `rot_dx/rot_dy`:
            // рамку считает абзац по месту меток (`spacers::overlays`).
            Piece::Overlay(el, at) if at.box_marker.is_some() => Piece::Overlay(
                el,
                OverlayAt {
                    rot_dx: at.rot_dx + rx,
                    rot_dy: at.rot_dy + ry,
                    ..at
                },
            ),
            Piece::Overlay(el, at) if rotated => Piece::Overlay(
                el,
                OverlayAt {
                    rot_dx: at.rot_dx + rx,
                    rot_dy: at.rot_dy + ry,
                    ..at
                },
            ),
            Piece::Overlay(el, at) => Piece::Overlay(
                gpui::div()
                    .pl(gpui::px(dx))
                    .pt(gpui::px(dy))
                    .child(el)
                    .into_any_element(),
                at,
            ),
            other => other,
        })
        .collect()
}

/// Physical offset -> pre-rotation frame of a rotated vertical line: a clockwise
/// turn sends the line axis (+x) to screen-down and the block axis (+y) to
/// screen-left; `sideways-lr` turns the other way (css-writing-modes-4 §7.1).
pub(crate) fn to_rotated_frame(style: &Computed, (dx, dy): (f32, f32)) -> (f32, f32) {
    use crate::style::computed::writing_mode::Turn;
    if style.wm().line().line_turn() == Turn::CounterClockwise {
        (-dy, dx)
    } else {
        (dy, -dx)
    }
}
