//! Контекст фрагментации сборки (`FragCx`: копия ребёнка стопки, ряд внешней колонки) и ось стопки.

use gpui::{Bounds, Pixels, point, px, size};

use crate::layout::options::RenderOpts;

/// Контекст фрагментации сборки — часть `RenderOpts`, которую ставит не
/// приложение, а код, строящий фрагменты (Blink несёт то же в
/// `ConstraintSpace` каждого вызова раскладки, а не в глобальном состоянии).
/// TRANSITIONAL(F2-7): FragCx несёт implicit in_stack_copy/outer_row вместо явного Space/resume; удалить эти поля в P22 после последних consumers P09/P10/P21.
#[derive(Clone, Debug, Default)]
pub struct FragCx {
    /// Сборщик диагностики конкретного прогона; хост забирает снимок через take.
    #[cfg(feature = "wpt-harness")]
    pub probe: crate::FragmentProbes,
    /// Строится копия ребёнка стопки колонок (`multicol_stack_child`).
    /// Многоколоночник со спаннером внутри другой стопки остаётся на
    /// сегментном пути: единой стопке нужен перенос ряда/спаннера во ВНЕШНЮЮ
    /// колонку (Blink `LayoutSpanner`: «The new row doesn't fit in the outer
    /// fragmentainer»), которого нет (`column-height-029`).
    pub(crate) in_stack_copy: bool,
    /// Высота ряда и фаза вложенного многоколоночника, заданные ВНЕШНЕЙ
    /// колонкой: копия строится рядами этой высоты (css-break-4 §2.1: «when a
    /// multi-column container breaks across pages, it generates a new row of
    /// columns on the next page»; Blink `column_layout_algorithm.cc:1741-1748`
    /// `ConstrainColumnBlockSize` → `min(size, available_outer_space)`).
    /// Адресована одному элементу — тому, чей `element()` вызван с ней; его
    /// дети её не видят (`RenderOpts::take_outer_row`).
    pub(crate) outer_row: Option<(f32, f32)>,
    /// Строится копия ребёнка, разбиваемого на фрагменты собственной
    /// раскладкой (`fragment::paginate`): `render::element` оборачивает её
    /// кандидатов в разрыв, и запись узлов их раскладки читает решатель.
    /// BandFlow exports measured line edges through the same recorder.
    pub(crate) watch: Option<std::rc::Rc<crate::layout::fragment::paginate::watch::Watched>>,
    /// Numeric pagination displacements of floats in this rebuilt copy.
    pub(crate) float_struts: std::rc::Rc<crate::layout::fragment::paginate::Struts>,
    /// Строятся дети стопки страниц (`PagedBuild`).
    pub(crate) paged: Option<std::rc::Rc<crate::layout::page::paged::PagedBuild>>,
}

impl RenderOpts {
    /// Настройки для сборки копий детей стопки колонок.
    pub(crate) fn in_stack_copy(&self) -> RenderOpts {
        let mut opts = self.clone();
        opts.frag.in_stack_copy = true;
        opts
    }

    /// Настройки для сборки вложенного многоколоночника рядами внешней колонки.
    pub(crate) fn with_outer_row(&self, row: (f32, f32)) -> RenderOpts {
        let mut opts = self.clone();
        opts.frag.outer_row = Some(row);
        opts
    }

    /// Забрать высоту ряда, адресованную этому элементу: она сама и
    /// настройки без неё для всего, что строится внутри.
    pub(crate) fn take_outer_row(&self) -> (Option<(f32, f32)>, Option<RenderOpts>) {
        match self.frag.outer_row {
            None => (None, None),
            Some(row) => {
                let mut inner = self.clone();
                inner.frag.outer_row = None;
                (Some(row), Some(inner))
            }
        }
    }
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
    /// The block-flow axis of a writing mode (§3.1): `sideways-lr` stacks like `vertical-lr`.
    pub(crate) fn of(wm: crate::style::computed::writing_mode::WritingMode) -> Self {
        match (wm.vertical(), wm.block_rl()) {
            (false, _) => StackAxis::Horizontal,
            (true, true) => StackAxis::VerticalRl,
            (true, false) => StackAxis::VerticalLr,
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> RenderOpts {
        RenderOpts {
            text: Default::default(),
            viewport: (800.0, 600.0),
            normal_line_height: 1.2,
            cx: Default::default(),
            frag: Default::default(),
        }
    }

    #[test]
    fn outer_row_is_consumed_for_children_without_changing_enclosing_handoff() {
        let outer = opts().with_outer_row((120.0, 0.0));
        let inner = outer.with_outer_row((60.0, 0.5));
        let (row, children) = inner.take_outer_row();
        assert_eq!(row, Some((60.0, 0.5)));
        let children = children.unwrap();
        let (row, cleared) = children.take_outer_row();
        assert_eq!(row, None);
        assert!(cleared.is_none());
        assert_eq!(outer.take_outer_row().0, Some((120.0, 0.0)));
    }

    #[test]
    fn outer_row_survives_unwind_of_nested_build() {
        let outer = opts().with_outer_row((120.0, 0.0));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let inner = outer.with_outer_row((60.0, 0.5));
                assert_eq!(inner.take_outer_row().0, Some((60.0, 0.5)));
                panic!("render failed");
            }))
            .is_err()
        );
        assert_eq!(outer.take_outer_row().0, Some((120.0, 0.0)));
    }
}
