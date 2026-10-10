//! Состояние обхода стопки (глубина вложенности, внешний ряд) и ось стопки.

use gpui::{Bounds, Pixels, point, px, size};

thread_local! {
    /// Глубина построения копий детей стопки (`render.rs`, `StackChild`).
    static STACK_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Сторож «строится копия ребёнка стопки». Многоколоночник со спаннером
/// внутри другой стопки остаётся на сегментном пути: единой стопке нужен
/// перенос ряда/спаннера во ВНЕШНЮЮ колонку (Blink `LayoutSpanner`: «The new
/// row doesn't fit in the outer fragmentainer»), которого нет
/// (`column-height-029`, `target/scout-columnwrap-2026-09b.md` §2.3).
pub struct StackScope;

impl StackScope {
    pub fn enter() -> Self {
        STACK_DEPTH.with(|d| d.set(d.get() + 1));
        StackScope
    }
}

impl Drop for StackScope {
    fn drop(&mut self) {
        STACK_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

thread_local! {
    /// Высота ряда вложенного многоколоночника, заданная ВНЕШНЕЙ колонкой
    /// (`set_outer_row` → `take_outer_row` первой строкой `render::element`).
    static OUTER_ROW: std::cell::Cell<Option<(f32, f32)>> = const { std::cell::Cell::new(None) };
    /// Сколько первых колонок укладки стоят НЕ с верха фрагментаинера (первый
    /// ряд вложенного многоколоночника, начатого ниже верха внешней колонки):
    /// не влезший с верха такой колонки монолит уходит дальше, а не
    /// переполняет её (Blink: `is_at_fragmentainer_start` ложно —
    /// `BreakBeforeChildIfNeeded`, css-break-3 §4.1 «may be pushed»).
    pub(crate) static NOT_TOP: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Передать следующему `element()` высоту внешнего фрагментаинера: копия
/// вложенного многоколоночника строится рядами этой высоты (css-break-4 §2.1:
/// «when a multi-column container breaks across pages, it generates a new row
/// of columns on the next page»; Blink `column_layout_algorithm.cc:1741-1748`
/// `ConstrainColumnBlockSize` → `min(size, available_outer_space)`).
/// Сторож `NOT_TOP` на время укладки.
pub(crate) struct NotTop(pub(crate) usize);

impl NotTop {
    pub(crate) fn set(n: usize) -> Self {
        NotTop(NOT_TOP.with(|c| c.replace(n)))
    }
}

impl Drop for NotTop {
    fn drop(&mut self) {
        NOT_TOP.with(|c| c.set(self.0));
    }
}

pub fn set_outer_row(h: Option<(f32, f32)>) {
    OUTER_ROW.with(|r| r.set(h));
}

/// Забрать переданную высоту (одноразово).
pub fn take_outer_row() -> Option<(f32, f32)> {
    OUTER_ROW.with(|r| r.take())
}

pub fn in_stack() -> bool {
    STACK_DEPTH.with(|d| d.get() > 0)
}

/// Ось стопки колонок. css-multicol-1 §2 (`Overview.bs:375-379`): «The
/// column boxes are ordered in the inline base direction of the multicol
/// container … The column width is the length of the column box in the inline
/// direction. The column height is the length of the column box in the block
/// direction»; note `:544-549`: «In text set using a vertical writing mode, the
/// block direction runs horizontally». Blink держит укладку логической
/// (`column_layout_algorithm.cc:982` `LogicalOffset logical_offset(
/// column_inline_offset, line_offset)`) и переводит в физику при сборке
/// фрагмента (`WritingModeConverter`). Вся арифметика стопки (`fill_at`,
/// `balance*`, `Rows`, `place`) у нас тоже логическая: «высота» в ней — блочный
/// размер, «x колонки» — строчное смещение. Физика — только в раскладке и
/// отрисовке (`prepaint_axis`/`paint_axis`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StackAxis {
    /// `horizontal-tb`: прогрессия колонок вправо, блочная ось вниз.
    Horizontal,
    /// `vertical-lr`: прогрессия колонок вниз, блочная ось вправо.
    VerticalLr,
    /// `vertical-rl`/`sideways-rl`: прогрессия вниз, блочная ось ВЛЕВО от
    /// правого края коробки (css-writing-modes-4 §3.1 «block flow direction»).
    VerticalRl,
}

impl StackAxis {
    pub fn is_vertical(self) -> bool {
        !matches!(self, StackAxis::Horizontal)
    }
}

/// Логическая коробка стопки → физическая. `io`/`ie` — смещение и размер по
/// СТРОЧНОЙ оси (по ней идёт прогрессия колонок), `bo`/`be` — по БЛОЧНОЙ;
/// `block` — блочный размер всей стопки (нужен `vertical-rl`: там блочная ось
/// отсчитывается от правого края). Blink `WritingModeConverter::ToPhysical`
/// (`writing_mode_converter.cc`): у `vertical-rl` `x = outer.width - offset -
/// inner.width`.
pub fn axis_box(
    axis: StackAxis,
    origin: gpui::Point<Pixels>,
    block: f32,
    io: f32,
    ie: f32,
    bo: f32,
    be: f32,
) -> Bounds<Pixels> {
    match axis {
        StackAxis::Horizontal => Bounds {
            origin: point(origin.x + px(io), origin.y + px(bo)),
            size: size(px(ie), px(be)),
        },
        StackAxis::VerticalLr => Bounds {
            origin: point(origin.x + px(bo), origin.y + px(io)),
            size: size(px(be), px(ie)),
        },
        StackAxis::VerticalRl => Bounds {
            origin: point(origin.x + px(block - bo - be), origin.y + px(io)),
            size: size(px(be), px(ie)),
        },
    }
}
