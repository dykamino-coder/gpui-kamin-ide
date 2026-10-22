//! Пробы строк и коробок текущей раскладки, нумерация абзацев при сборке.

use crate::layout::cx::LayoutCx;
use gpui::{Bounds, Pixels};

/// Бюджет строк обрезки (`line-clamp`, css-overflow-3/4): точка среза —
/// низ N-й СЧИТАЕМОЙ строки. Строки потомков в собственном контексте
/// форматирования (BFC: overflow, флоат, корень потока) видимы, но НЕ
/// считаются; блок, пересекающий точку среза, прячется целиком — срез
/// поднимается к его верху. Точка меряется пробами локальной раскладки и
/// применяется потолком высоты итоговой сборки того же кадра.
#[derive(Clone, Debug)]
pub struct ClampEntry {
    pub bounds: Bounds<Pixels>,
    /// Высота строки в точках; 0 — блок без собственного текста.
    pub line: f32,
    /// Строки не считаются (элемент внутри вложенного BFC).
    pub skip_count: bool,
    /// Коробка с ЗАДАННОЙ высотой: фрагментировать нечего, пересечённая
    /// точкой среза она прячется целиком.
    pub fixed_height: bool,
    /// Нижние рамка и паддинг коробки в точках. Проба меряет ПАДДИНГ-БОКС,
    /// а фрагментированная коробка своих нижних рамки и паддинга не теряет
    /// (css-overflow-4 §5.3): на них укорачивается бюджет строк и на них же
    /// удлиняется итоговый срез. Для строчных проб — ноль.
    pub bp_after: f32,
    /// Bottom margin of a box probe in points: an ancestor fragmented at
    /// the clamp point keeps its end margin, collapsing with the end
    /// margins of nested ancestors unless border/padding separates them
    /// (css-overflow-4 §5.3; Blink `LineClampAncestorChain`).
    pub end_margin: f32,
    /// Minimum border-box height of a box probe (`min-height`) from its
    /// border-box top, 0 when none: fragmented at the clamp point it still
    /// ends no earlier (Blink clamps the chain size to min/max).
    pub min_end: f32,
    /// Box probe with a `max-height`: once its content overflows it, its
    /// size no longer depends on the content and it is as monolithic as a
    /// box with a definite height (`line-clamp-auto-044/063`).
    pub capped: bool,
    /// Node of a box probe (0 for paragraph probes).
    pub node: u64,
    /// Порядковый номер абзаца ВНУТРИ клэмп-контейнера, выданный при
    /// построении. `None` — проба коробки, а не абзаца: знак обрыва на
    /// неё не садится. Номер связывает абзац пробы с абзацем
    /// итоговой сборки независимо от изменения его геометрии.
    pub seq: Option<u32>,
    /// Сколько строк этому абзацу оставлено УЖЕ на этом кадре. Признак
    /// того, что абзац укорочен нами: его собственный низ за срез больше
    /// не выходит, и без этой защёлки «что-то срезано» пропало бы, а
    /// повторный локальный замер потерял бы признак обрезки
    /// (css-overflow-4 §5.3: вставка знака
    /// «must not cause a reevaluation of the effects of `continue`»).
    pub clamped: Option<usize>,
    /// Пустая поточная блочная коробка (без содержимого): сама по себе —
    /// возможная точка среза МЕЖДУ блоками (css-overflow-4 §5.3).
    pub empty: bool,
    /// Box probe: paragraph numbers `[first, end)` built inside the box
    /// (its descendants in box-tree order).
    pub span: Option<(u32, u32)>,
    /// Top border of a box probe: the probe measures the padding box, a
    /// cut above the box goes above its border.
    pub bt_before: f32,
    /// Absolutely positioned box: not hidden by the clamp (Blink keeps
    /// out-of-flow boxes painted), its overflow stays visible.
    pub positioned: bool,
}

/// End edges of a box probe beyond its padding box (see `ClampEntry`).
#[derive(Clone, Copy, Default)]
pub struct BoxEnds {
    pub margin: f32,
    pub min_end: f32,
    pub capped: bool,
    pub node: u64,
}

/// Выдать следующему абзацу клэмп-контейнера его номер. Счётчик сбрасывает
/// вход в сам контейнер (`ClampBuild::container`), поэтому нумерация одна и та
/// же в пробе и итоговой сборке, пока не меняется дерево.
pub fn clamp_next_seq(key: u64, cx: &LayoutCx) -> u32 {
    {
        let m = &cx.clamp_build.seq;
        let mut m = m.borrow_mut();
        let n = m.entry(key).or_insert(0);
        let v = *n;
        *n += 1;
        v
    }
}

/// Number the next paragraph of the clamp container would get.
pub fn clamp_peek_seq(key: u64, cx: &LayoutCx) -> u32 {
    cx.clamp_build.seq.borrow().get(&key).copied().unwrap_or(0)
}

/// Paragraph-number span of a box being built inside a clamp container:
/// opened before its children, closed after them.
pub type ClampSpan = std::rc::Rc<std::cell::Cell<(u32, u32)>>;

pub fn open_clamp_span(cx: &crate::layout::cx::LayoutCx) -> Option<ClampSpan> {
    // Atomic descendants belong to their enclosing line, not the next paragraph.
    if cx.clamp_atom_depth != 0 {
        return None;
    }
    let (key, _) = clamp_context(cx)?;
    let first = clamp_peek_seq(key, cx);
    Some(std::rc::Rc::new(std::cell::Cell::new((first, u32::MAX))))
}

pub fn close_clamp_span(span: Option<ClampSpan>, cx: &LayoutCx) {
    if let (Some(span), Some((key, _))) = (span, clamp_context(cx)) {
        span.set((span.get().0, clamp_peek_seq(key, cx)));
    }
}

/// Active container and its BFC counting boundary for this build.
pub fn clamp_context(cx: &LayoutCx) -> Option<(u64, bool)> {
    cx.clamp_build.context
}
