//! Стили элементов GPUI, уже переведённые из стилей коробок в этой сборке документа: вход
//! (значения коробки, её размеры, контекст значений) → общий `StyleRefinement`.
//!
//! Владелец — сборка (`BuildContext`), а не поток: перевод читает арену `calc()` кадра,
//! шрифты и семейство документа, которые между сборками меняются. Каждая сборка документа
//! (`render::render`, `render::render_block`) начинает с пустой памяти.

use crate::layout::block::sizing::BoxDimensions;
use crate::style::computed::BoxStyle;
use crate::values::ValueCx;
use gpui::StyleRefinement;
use std::rc::Rc;

/// Distinct recent inputs kept for reuse. Boxes of a page repeat a few styles, often
/// interleaved (table cells, list items with nested blocks): as in GPUI's refinement share
/// and Stylo's sharing cache, a bounded set of the newest distinct inputs.
const CANDIDATES: usize = 32;

#[derive(Default)]
pub(crate) struct StyleConversions {
    recent: Vec<Conversion>,
    /// Slot of the next new entry once `recent` is full.
    next: usize,
}

struct Conversion {
    input: BoxStyle,
    dimensions: BoxDimensions,
    values: Values,
    style: Rc<StyleRefinement>,
}

/// The parts of the value context a conversion reads.
#[derive(Clone, Copy, PartialEq)]
struct Values {
    doc: *const crate::values::DocumentValues,
    frame: bool,
    viewport: (f32, f32),
    scheme: crate::style::values::value::Scheme,
    clearance_possible: bool,
}

impl Values {
    fn of(vcx: ValueCx<'_>) -> Self {
        Self {
            doc: vcx.doc,
            frame: vcx.frame,
            viewport: vcx.viewport,
            scheme: vcx.scheme,
            clearance_possible: vcx.clearance_possible,
        }
    }
}

impl StyleConversions {
    /// The element style converted earlier in this build from an equal input.
    pub(crate) fn find(
        &self,
        input: &BoxStyle,
        dimensions: &BoxDimensions,
        vcx: ValueCx<'_>,
    ) -> Option<Rc<StyleRefinement>> {
        let values = Values::of(vcx);
        let len = self.recent.len();
        (1..=len)
            .map(|back| &self.recent[(self.next + len - back) % len])
            .find(|c| {
                c.values == values && c.dimensions == *dimensions && c.input.same_box_values(input)
            })
            .map(|c| c.style.clone())
    }

    /// Remember the element style converted from `input`.
    pub(crate) fn record(
        &mut self,
        input: &BoxStyle,
        dimensions: BoxDimensions,
        vcx: ValueCx<'_>,
        style: Rc<StyleRefinement>,
    ) {
        let entry = Conversion {
            input: input.clone(),
            dimensions,
            values: Values::of(vcx),
            style,
        };
        if self.recent.len() < CANDIDATES {
            self.recent.push(entry);
            self.next = self.recent.len() % CANDIDATES;
        } else {
            self.recent[self.next] = entry;
            self.next = (self.next + 1) % CANDIDATES;
        }
    }
}

#[cfg(test)]
#[path = "conversions_tests.rs"]
mod tests;
