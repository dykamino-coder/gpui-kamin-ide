//! Блочные дети вложенного многоколоночника, которые режутся по рядам как строки.

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::style::computed::{Display, Position};
use crate::style::values::value::Len;

/// Блочный ребёнок вложенного многоколоночника, чья мера — его собственная
/// коробка: высота в точках (у `relative` из одних абсолютов — и `auto`, ноль), без полей и `overflow`, без потоковых потомков
/// (нечему переполнять коробку). Допустимы лишь абсолютные дети: коробка
/// `position: relative` — их содержащий блок, и `shape_full` уже дотягивает
/// меру до их низа (css-position-3 §abspos-breaking: абсолют продолжается в
/// следующих фрагментаинерах; Blink `column_layout_algorithm.cc` добавляет
/// колонки под OOF). Такая коробка режется по рядам так же, как строки
/// (css-break-3 §4.4, разрыв класса C внутри блока). Монолит выше ряда
/// `nested_rows_shape` отклоняет: Blink переносит его иначе, чем наша укладка.
pub(super) fn plain_block_kid(k: &Element) -> bool {
    let s = k.style.cascaded();
    let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
    let unset = |l: &Option<Len>| matches!(l, None | Some(Len::Auto));
    matches!(s.display, None | Some(Display::Block))
        && !k.inline
        && s.float.unwrap_or(0) == 0
        && (matches!(s.height, Some(Len::Px(h)) if h > 0.0)
            || (s.position == Some(Position::Relative) && unset(&s.height)))
        && s.min_height.is_none()
        && s.max_height.is_none()
        && zero(&s.margin.top)
        && zero(&s.margin.bottom)
        && !s.break_inside_avoid
        && crate::layout::page::paged::visible_overflow(s)
        && match s.position {
            None => k.children.iter().all(is_blank),
            Some(Position::Relative) => {
                [&s.inset.top, &s.inset.right, &s.inset.bottom, &s.inset.left]
                    .into_iter()
                    .all(unset)
                    && k.children.iter().all(|n| match n {
                        // Абсолют внутри абсолюта — тоже в этой коробке:
                        // его дотяг `shape_full` складывает цепочкой.
                        Node::Element(a) => a.style.cascaded().position == Some(Position::Absolute),
                        other => is_blank(other),
                    })
            }
            _ => false,
        }
}

/// Все внепоточные потомки многоколоночника — абсолютные дети его прямых
/// детей вида `plain_block_kid`: их содержащий блок внутри одной коробки
/// потока, и рядами выражаются и он, и дотяг (`nested_rows_shape`).
pub(super) fn oof_in_plain_kids(c: &Element) -> bool {
    c.children.iter().all(|n| match n {
        Node::Element(k) => {
            if out_of_flow(k.style.cascaded()) {
                false
            } else if super::nested::oof_descendant(k) {
                plain_block_kid(k)
            } else {
                true
            }
        }
        _ => true,
    })
}

/// Длина в единицах раскладки — 1/64 точки (Blink `LayoutUnit`): разрешённая
/// доля не тащит хвост погрешности `f32` в края колонок.
pub(crate) fn layout_unit(v: f32) -> f32 {
    (v * 64.0).round() / 64.0
}
