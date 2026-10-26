//! ColumnFlow как элемент gpui: структура и раскладка колонок.

use super::measure::measure_columns;
use crate::hooks::Hooks;
use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Font, LayoutId, Pixels, SharedString, Window, point,
    px, size,
};
use std::rc::Rc;

type ColumnBuilder = Rc<dyn Fn(&[usize], usize, Pixels, &[super::rows::Row]) -> AnyElement>;

/// Многоколоночный поток: текст режется на колонки по строкам.
///
/// Сетка из детей давала то же расположение только когда детей много: один
/// длинный абзац оставался в первой колонке целиком. Настоящий поток режет
/// сам текст — сколько строк на колонку, столько и уходит, остальное в
/// следующую. Где кончается строка, знает перенос, а он зависит от ширины
/// колонки: значит, снова замер.
pub struct ColumnFlow {
    pub(super) build: ColumnBuilder,
    pub(super) outer: Option<Rc<crate::layout::fragment::token::space::sequence::Nested>>,
    pub(super) rows: Rc<std::cell::RefCell<Vec<super::rows::Row>>>,
    pub(super) text: SharedString,
    pub(super) count: Option<usize>,
    pub(super) col_w: Option<f32>,
    pub(super) gap: f32,
    pub(super) font: Font,
    pub(super) font_size: f32,
    pub(super) line_height: f32,
    /// `column-fill: auto` с заданной высотой: колонки заполняются подряд до
    /// этой высоты, а не делятся поровну (css-multicol-1 §3.3).
    pub(super) fill_height: Option<f32>,
    /// Текст — единственного ребёнка-монолита (`render::column_flow_in`): в
    /// узкой колонке его строки не режутся (`measure_columns`).
    pub(super) whole: bool,
    /// `orphans`/`widows` блока со строками (css-break-3 §4.4).
    pub(super) line_breaks: (usize, usize),
    pub(super) cuts: Rc<std::cell::RefCell<(Vec<usize>, Pixels, usize)>>,
}

impl ColumnFlow {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        build: ColumnBuilder,
        text: SharedString,
        count: Option<usize>,
        col_w: Option<f32>,
        gap: f32,
        font: Font,
        font_size: f32,
        line_height: f32,
        fill_height: Option<f32>,
        whole: bool,
    ) -> Self {
        ColumnFlow {
            build,
            outer: None,
            rows: Default::default(),
            text,
            count,
            col_w,
            gap,
            font,
            font_size,
            line_height,
            fill_height,
            whole,
            line_breaks: (1, 1),
            cuts: Rc::new(std::cell::RefCell::new((Vec::new(), px(0.), 1))),
        }
    }

    pub(crate) fn in_outer(
        mut self,
        outer: Option<Rc<crate::layout::fragment::token::space::sequence::Nested>>,
    ) -> Self {
        self.outer = outer;
        self
    }

    /// `orphans`/`widows` строк потока (css-break-3 §4.4): сколько строк блока
    /// должно остаться в колонке до разрыва и после него.
    pub fn line_breaks(mut self, orphans: usize, widows: usize) -> Self {
        self.line_breaks = (orphans.max(1), widows.max(1));
        self
    }
}

impl Hooks for ColumnFlow {
    type Layout = LayoutId;
    type Prepaint = AnyElement;

    fn request_layout(&mut self, window: &mut Window, _cx: &mut App) -> (LayoutId, Self::Layout) {
        let layout_id = self.measure_layout(window);
        (layout_id, layout_id)
    }

    fn prepaint(
        &mut self,
        bounds: Bounds<Pixels>,
        state: &mut Self::Layout,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let stale = self.cuts.borrow().1 != bounds.size.width;
        if stale && bounds.size.width > px(0.) {
            let (at, used, _, _) = measure_columns(
                &self.text,
                self.count,
                self.col_w,
                self.gap,
                &self.font,
                self.font_size,
                self.line_height,
                self.fill_height,
                self.whole,
                self.line_breaks,
                bounds.size.width,
                self.outer.as_deref(),
                &mut self.rows.borrow_mut(),
                window,
            )
            .expect("resolved outer fragmentainer sequence");
            *self.cuts.borrow_mut() = (at, bounds.size.width, used);
        }
        let (cuts, used) = {
            let b = self.cuts.borrow();
            (b.0.clone(), b.2)
        };
        let mut child = (self.build)(&cuts, used, bounds.size.width, &self.rows.borrow());
        // The columns are a separate layout tree rooted at the host's EXACT
        // origin: rooting them at the device-rounded bounds dropped the
        // fractional part of the host's position, so content after a
        // spanner with fractional margins sat up to half a pixel higher than
        // the same content in normal flow (`multicol-span-all-006`).
        let origin = window.layout_origin_unrounded(*state);
        child.layout_as_root_at(
            origin,
            size(
                AvailableSpace::Definite(bounds.size.width),
                AvailableSpace::MinContent,
            ),
            window,
            cx,
        );
        child.prepaint_at(point(px(0.0), px(0.0)), window, cx);
        child
    }

    fn paint(
        &mut self,
        _bounds: Bounds<Pixels>,
        _state: &mut Self::Layout,
        child: &mut Self::Prepaint,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}
