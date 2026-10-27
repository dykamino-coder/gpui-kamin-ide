//! Контекст фрагментации сборки (`FragCx`: копия ребёнка стопки, ряд внешней колонки) и ось стопки.

use crate::layout::fragment::token::space::sequence::Nested;
use gpui::{Bounds, Pixels, point, px, size};
use std::rc::Rc;

use crate::layout::options::RenderOpts;

/// Контекст фрагментации сборки — часть `RenderOpts`, которую ставит не
/// приложение, а код, строящий фрагменты (Blink несёт то же в
/// `ConstraintSpace` каждого вызова раскладки, а не в глобальном состоянии).
/// TRANSITIONAL(F2-7): FragCx несёт implicit in_stack_copy вместо явного resume; удалить поле в P22 после последних consumers P10/P21.
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
    /// Адрес начала и последовательность реальных внешних Spaces: первый
    /// ряд получает остаток, последующие — собственные ограничения (css-break-4 §2.1: «when a
    /// multi-column container breaks across pages, it generates a new row of
    /// columns on the next page»; Blink `column_layout_algorithm.cc:1741-1748`
    /// `ConstrainColumnBlockSize` → `min(size, available_outer_space)`).
    /// Адресована одному элементу — тому, чей `element()` вызван с ней; его
    /// дети её не видят (`RenderOpts::take_outer_space`).
    pub(crate) outer_space: Option<Rc<Nested>>,
    /// Строится копия ребёнка, разбиваемого на фрагменты собственной
    /// раскладкой (`fragment::paginate`): `render::element` оборачивает её
    /// кандидатов в разрыв, и запись узлов их раскладки читает решатель.
    /// BandFlow exports measured line edges through the same recorder.
    pub(crate) watch: Option<std::rc::Rc<crate::layout::fragment::paginate::watch::Watched>>,
    /// Numeric pagination displacements of floats in this rebuilt copy.
    pub(crate) float_struts: std::rc::Rc<crate::layout::fragment::paginate::Struts>,
    /// Нижние границы начала float в логической оси его BFC: рост обычного
    /// потока уже мог довести float до этой границы (css-break-3 §2.1).
    pub(crate) float_floors: std::rc::Rc<std::collections::HashMap<u64, f32>>,
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
    pub(crate) fn with_outer_space(&self, row: Rc<Nested>) -> RenderOpts {
        let mut opts = self.clone();
        opts.frag.outer_space = Some(row);
        opts
    }

    /// Забрать внешнюю последовательность, адресованную этому элементу: она и
    /// настройки без неё для всего, что строится внутри.
    pub(crate) fn take_outer_space(&self) -> (Option<Rc<Nested>>, Option<Box<RenderOpts>>) {
        match self.frag.outer_space.clone() {
            None => (None, None),
            Some(row) => {
                let mut inner = Box::new(self.clone());
                inner.frag.outer_space = None;
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

    fn nested(block: f32, offset: f32) -> Rc<Nested> {
        use crate::layout::fragment::token::{Space, space::sequence::Sequence};
        Sequence::columns(Space {
            block_size: Some(block),
            ..Space::columns()
        })
        .nested(0, offset)
        .unwrap()
    }

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
    fn outer_space_is_consumed_for_children_without_changing_enclosing_handoff() {
        let outer = opts().with_outer_space(nested(120.0, 0.0));
        let inner = outer.with_outer_space(nested(60.0, 0.5));
        let (row, children) = inner.take_outer_space();
        assert_eq!(row.unwrap().at(0).unwrap().block_size, Some(59.5));
        let children = children.unwrap();
        let (row, cleared) = children.take_outer_space();
        assert!(row.is_none());
        assert!(cleared.is_none());
        assert_eq!(
            outer
                .take_outer_space()
                .0
                .unwrap()
                .at(0)
                .unwrap()
                .block_size,
            Some(120.0)
        );
    }

    #[test]
    fn outer_space_survives_unwind_of_nested_build() {
        let outer = opts().with_outer_space(nested(120.0, 0.0));
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let inner = outer.with_outer_space(nested(60.0, 0.5));
                assert_eq!(
                    inner
                        .take_outer_space()
                        .0
                        .unwrap()
                        .at(0)
                        .unwrap()
                        .block_size,
                    Some(59.5)
                );
                panic!("render failed");
            }))
            .is_err()
        );
        assert_eq!(
            outer
                .take_outer_space()
                .0
                .unwrap()
                .at(0)
                .unwrap()
                .block_size,
            Some(120.0)
        );
    }
}
