//! Колонки меры при известной ширине: prepaint берёт их, а не готовит заново.
//!
//! Мера стопки, чью ширину решает раскладка, готовит колонки (`Recipe::prepare`)
//! в собственном движке раскладки (`gpui::RetainedLayout`) и в контексте места
//! стопки, поэтому её элементы те же, что построил бы prepaint. Prepaint при той
//! же ширине колонки ставит их в стопку и раскладывает куски в этом движке:
//! колонки раскладываются один раз за кадр (Blink хранит `LayoutResult`
//! раскладки колонок и рисует его фрагменты, мера и отрисовка набора колонок —
//! одна раскладка).

use super::Recipe;
use crate::layout::fragment::types::StackChild;
use crate::layout::multicol::column_stack::ColumnStack;
use crate::layout::positioned::containing_block::metrics::Snapshot;
use gpui::{App, ElementLayoutContext, Pixels, RetainedLayout, Window};
use std::rc::Rc;

type Extent = (f32, Option<Pixels>, Option<Pixels>);

/// Колонки, подготовленные мерой при ширине контейнера `width`.
pub(crate) struct Measured {
    width: u32,
    col_w: u32,
    children: Vec<StackChild>,
    geometry: Rc<Snapshot>,
    copies: usize,
    forced_h: Option<f32>,
    balance_failure: Option<crate::layout::fragment::token::Error>,
    /// Ответ меры: повторный запрос при той же ширине берёт его.
    extent: Extent,
    engine: RetainedLayout,
}

impl Measured {
    /// Put the prepared columns into `stack`, as `Recipe::prepare` would.
    pub(crate) fn install(self, stack: &mut ColumnStack) -> RetainedLayout {
        stack.col_w.set(f32::from_bits(self.col_w));
        stack.children = self.children;
        stack.oof.geometry = self.geometry;
        stack.copies = self.copies;
        stack.forced_h = self.forced_h;
        stack.balance_failure = self.balance_failure;
        self.engine
    }
}

impl Recipe {
    /// Мера при известной ширине `width` в контексте места стопки: колонки
    /// остаются prepaint (`take_measured`); мера при другой ширине их заменяет.
    pub(super) fn measure_kept(
        &self,
        stack: &mut ColumnStack,
        (width, col_w): (f32, f32),
        context: &ElementLayoutContext,
        window: &mut Window,
        cx: &mut App,
    ) -> Extent {
        if let Some(extent) = self.measured_at(width) {
            crate::work::add!(MulticolMeasureReuses, 1);
            return extent;
        }
        let mut engine = RetainedLayout::default();
        let extent = window.with_retained_layout(&mut engine, |window| {
            window.with_element_layout_context(context, |window| {
                self.prepare(stack, col_w, window, cx);
                self.measured_extent(stack, width, window, cx)
            })
        });
        let children = std::mem::take(&mut stack.children);
        let geometry = std::mem::take(&mut stack.oof.geometry);
        // Repeated table bands were requested, not laid out, by the baseline
        // measure: prepaint builds those columns again.
        let kept = (!children.iter().any(|c| c.repeat.is_some())).then(|| Measured {
            width: width.to_bits(),
            col_w: col_w.to_bits(),
            children,
            geometry,
            copies: stack.copies,
            forced_h: stack.forced_h,
            balance_failure: stack.balance_failure,
            extent,
            engine,
        });
        self.measured.replace(kept);
        extent
    }

    /// The answer of the kept measure at this container width.
    fn measured_at(&self, width: f32) -> Option<Extent> {
        let measured = self.measured.borrow();
        measured
            .as_ref()
            .filter(|m| m.width == width.to_bits())
            .map(|m| m.extent)
    }

    /// The columns the last kept measure prepared at column width `col_w`.
    pub(crate) fn take_measured(&self, col_w: f32) -> Option<Measured> {
        self.measured.take().filter(|m| m.col_w == col_w.to_bits())
    }
}
