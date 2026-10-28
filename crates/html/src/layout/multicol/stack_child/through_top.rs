//! Снятие поля, схлопнутого сквозь верх корня копии ребёнка стопки.

use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::layout::fragment::table_bands::table_box;
use crate::layout::multicol::spanner::multicol_container;
use crate::style::computed::Display;
use crate::style::values::value::Len;

/// Поле, схлопнутое СКВОЗЬ верх коробки (CSS 2.1 §8.3.1: у первого поточного
/// блока без верхней отбивки и рамки при высоте `auto` оно становится полем
/// коробки, `fragment_margins`). Стопка кладёт это поле сама — `lead` в
/// `fill_at`, с усечением на разрыве (css-break-4 `margin-break: auto`: «any
/// margins adjoining the break … are truncated to zero after the break»). Копия
/// же ставится КОРНЕМ
/// (`layout_as_root`), а у корня taffy поля с детьми не схлопывает
/// (`vendor/taffy/src/compute/block.rs:186-192`, `vertical_margins_are_collapsible`
/// у корня ложно): поле внука вставало ВТОРОЙ раз внутри копии. На разрыве это
/// сдвигало содержимое на всё поле вниз: `margin-at-break-001/002` — `margin-top:
/// 60px` у первого внука, в колонке 2 зелёное 110..160 вместо 50..100 (по
/// снимку). Снимается ровно цепочка схлопывания:
/// первый непустой ребёнок — поточный блок, у коробки нет верхней отбивки/рамки и
/// высота `auto`. Сетка, гибкий контейнер, таблица и многоколоночник поля детей
/// сквозь себя не пропускают; внепоточный первым цепочку не начинает.
pub(super) fn strip_through_top(c: &mut Element, depth: u8) {
    if depth == 0
        || c.style.used().height.is_some()
        || table_box(c)
        || multicol_container(&c.style.used())
        || c.style.used().webkit_box == Some(true)
        || !matches!(
            c.style.used().display,
            None | Some(Display::Block) | Some(Display::ListItem)
        )
    {
        return;
    }
    let px = |l: &Option<Len>| crate::style::values::value::px_lossy_zero(*l);
    let b = c.style.used().borders();
    if px(&c.style.used().padding.top) + px(&b.top) != 0.0 {
        return;
    }
    let Some(Node::Element(k)) = c.children.iter_mut().find(|n| !is_blank(n)) else {
        return;
    };
    if k.inline
        || out_of_flow(&k.style.used())
        || !matches!(
            k.style.used().position,
            None | Some(crate::style::computed::Position::Relative)
        )
    {
        return;
    }
    k.style.used_mut().margin.top = None;
    strip_through_top(k, depth - 1);
}
