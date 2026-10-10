//! Overlays for collect; split out to keep the owning module within 250 lines.

use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::inline::*;
use gpui::{AnyElement, ParentElement, Styled};

/// Кусок вне потока в РЯДУ: сам абзац его разместить не может (ряд собирает
/// раскладка), поэтому работает прежний обход — нулевая распорка на месте
/// куска и сам элемент в позднем слое, который рисуется от её угла.
pub(crate) fn overlay_in_row(el: AnyElement) -> AnyElement {
    let spot: crate::layout::positioned::containing_block::SpotCell = Default::default();
    let probe = crate::layout::positioned::containing_block::spot_probe(spot.clone(), false);
    match crate::layout::positioned::containing_block::late_push(spot, el) {
        None => probe,
        Some(kept) => {
            let mut hole = gpui::div().relative().w_0().h_0().flex_shrink_0();
            hole.style().align_self = Some(gpui::AlignItems::FlexStart);
            hole.child(kept).into_any_element()
        }
    }
}

/// Относительный сдвиг коробки в точках: `left - right`, `top - bottom`
/// (CSS 2.1 §9.4.3). Нулевой, если элемент не относительный.
pub(super) fn relative_inset(style: &Computed) -> (f32, f32) {
    if style.position != Some(crate::style::computed::Position::Relative) {
        return (0.0, 0.0);
    }
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    (
        px_of(style.inset.left) - px_of(style.inset.right),
        px_of(style.inset.top) - px_of(style.inset.bottom),
    )
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
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let (dx, dy) = (
        px_of(style.inset.left) - px_of(style.inset.right),
        px_of(style.inset.top) - px_of(style.inset.bottom),
    );
    if dx == 0.0 && dy == 0.0 {
        return pieces;
    }
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
            Piece::Overlay(el, at) if rotated => Piece::Overlay(
                el,
                OverlayAt {
                    rot_dx: at.rot_dx + dx,
                    rot_dy: at.rot_dy + dy,
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
