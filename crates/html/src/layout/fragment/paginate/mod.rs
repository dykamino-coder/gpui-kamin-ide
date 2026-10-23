//! Пагинация одного ребёнка контекста фрагментации по его раскладке (первый шаг перехода к
//! break tokens). Геометрия фрагментаинера (блочный размер `H`, смещение `y0` ребёнка в его
//! первом фрагментаинере) становится входом раскладки ребёнка. Решения о разрывах
//! принимаются по РАЗЛОЖЕННОЙ геометрии копии, а не по оценке `shape_full` на DOM: монолит,
//! пересекающий границу фрагментаинера, или коробка с принудительным разрывом перед ней
//! получает распорку пагинации, и копия раскладывается заново. Вызванный этим рост разносят
//! обычные алгоритмы раскладки (flex-ряд растёт до сдвинутого элемента, растянутые соседи
//! следуют, следующие строки сдвигаются) — расширение строки css-flexbox-1 §12 / Blink
//! `flex_layout_algorithm.cc`. Итоговая копия — поток, выровненный по сетке фрагментаинеров,
//! и режется по краям колонок. Здесь: кандидаты разрыва (`Cand`, `candidates`) и `Paginate`
//! с перестроением копий (`Rebuild`). Подмодули: гейт (`gate`), распорки (`apply`),
//! решатель (`solve`), наблюдение (`watch`).

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::style::computed::Display;
use crate::style::values::value::Len;
mod candidates;
mod gate;
pub(crate) use gate::gate;
mod apply;
pub(crate) use apply::{Strut, Struts, apply};
pub(crate) mod solve;
pub(crate) mod watch;

/// A break opportunity whose position is decided on the laid-out copy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Cand {
    /// Monolithic box (css-break-4 §4.1); `soft` — only `break-inside: avoid`,
    /// which may still break when it does not fit a whole fragmentainer.
    Mono { soft: bool },
    /// Float margin box recorded by BandFlow; its strut moves only the float.
    Float { soft: bool },
    /// Forced break before the box (css-break-4 §3.1); `mt` — its margin,
    /// preserved after a forced break (§5.2).
    Forced { mt: f32 },
    /// Item of a column flex container: never breaks by itself; its main
    /// size is fixed before fragmentation (css-flexbox-1 §12, Blink
    /// `GiveItemsFinalPositionAndSizeForFragmentation`), so a strut among
    /// its siblings must not shrink or grow it.
    Item,
    /// Item of a wrapping flex row whose rows are separated by `row-gap`: a
    /// gap adjoining a break is truncated (Blink
    /// `UpdateOffsetAdjustmentForSuppressedRowGap`).
    Gap { gap: f32, parent: u64 },
    /// `break-before: avoid` on this box (or `break-after: avoid` on `prev`):
    /// a fragmentainer boundary right at this edge violates it, and the
    /// earlier breakpoint before `prev` has the better appeal (css-break-3
    /// §4.4 rule 3; Blink `EarlyBreak`, `kBreakAppealViolatingBreakAvoid`).
    Avoid { prev: u64 },
}

fn row_flex(s: &crate::style::computed::Computed) -> bool {
    use crate::style::computed::FlexDir;
    matches!(s.display, Some(Display::Flex) | Some(Display::InlineFlex))
        && matches!(
            s.flex_dir,
            None | Some(FlexDir::Row) | Some(FlexDir::RowReverse)
        )
}

/// The px `row-gap` of a wrapping flex row.
pub(crate) fn row_gap(s: &crate::style::computed::Computed) -> Option<f32> {
    match s.gap {
        Some((Some(Len::Px(g)), _))
            if g > 0.0
                && row_flex(s)
                && s.flex_wrap == Some(true)
                && s.flex_wrap_reverse != Some(true) =>
        {
            Some(g)
        }
        _ => None,
    }
}

fn column_flex(s: &crate::style::computed::Computed) -> bool {
    use crate::style::computed::FlexDir;
    matches!(s.display, Some(Display::Flex) | Some(Display::InlineFlex))
        && matches!(s.flex_dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse))
}

fn layout_parent(s: &crate::style::computed::Computed) -> bool {
    matches!(
        s.display,
        Some(Display::Flex) | Some(Display::Grid) | Some(Display::InlineFlex)
    ) || s.webkit_box == Some(true)
}

/// Collect the candidates of `c`'s subtree, or `None` when the subtree holds
/// content whose measured geometry is unavailable (tables or nested
/// fragmentation contexts). Lines remain in the measured flow geometry.
pub(crate) fn candidates(c: &Element, vcx: ValueCx<'_>) -> Option<Vec<(u64, Cand)>> {
    let mut out = Vec::new();
    candidates::walk(c, 8, &mut out, vcx)?;
    Some(out)
}

/// A single-line flex row with a `break-inside: avoid` item: breaking inside
/// the item breaks inside the row, and the earlier breakpoint before the row
/// has the better appeal (css-break-3 §4.4 rule 3; Blink break appeal
/// `kBreakAppealViolatingBreakAvoid` vs the row's `EarlyBreak`), so the row
/// is avoided as a whole.
pub(crate) fn avoid_row(p: &Element) -> bool {
    row_flex(&p.style.used())
        && p.style.used().flex_wrap != Some(true)
        && p.children.iter().any(|n| {
            matches!(n, Node::Element(k)
                if !out_of_flow(&k.style.used())
                    && k.style.used().break_inside_avoid
                    && !crate::layout::fragment::probe::size_monolith(k)
                    && crate::layout::page::paged::visible_overflow(&k.style.used()))
        })
}

/// Rebuild of a paginated child's copies with a strut map: `(main, extra
/// copies)` for `n` copies in all, or `None` when the struts cannot be
/// expressed.
pub(crate) type Rebuild =
    dyn Fn(&Struts, usize) -> Option<(gpui::AnyElement, Vec<gpui::AnyElement>)>;

/// A stack child paginated by its own layout (see the module docs).
pub struct Paginate {
    /// The child itself is an avoided row (`avoid_row`): it is placed as a
    /// `break-inside: avoid` box by the stack.
    pub(crate) avoid_root: bool,
    pub(crate) cands: Vec<(u64, Cand)>,
    pub(crate) rebuild: Box<Rebuild>,
    /// Recorder of the candidates' layout nodes in the rebuilt copies.
    pub(crate) watched: std::rc::Rc<watch::Watched>,
}
