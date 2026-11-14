//! Входы и числовые значения ограниченного кэша баланса.

use crate::layout::fragment::flow::GrowthSnapshot;
use crate::layout::fragment::types::Kid;
use crate::layout::multicol::column_stack::ColumnStack;
use gpui::{Pixels, Window};
use std::cell::{Cell, RefCell};

pub(super) type Extent = (f32, Option<Pixels>, Option<Pixels>);

#[derive(Default)]
pub(in crate::layout::multicol::deferred) struct Cache {
    pub(super) window: Cell<Option<(f32, f32)>>,
    pub(super) plans: RefCell<Vec<Plan>>,
    pub(super) extents: RefCell<Vec<(Key, Extent)>>,
}

#[derive(PartialEq)]
pub(super) struct Key {
    pub(super) width: f32,
    pub(super) count: usize,
    pub(super) gap: f32,
    pub(super) content_extent: bool,
}

pub(super) struct Plan {
    pub(super) key: Key,
    pub(super) height: Option<f32>,
    pub(super) kids: Vec<Child>,
}

pub(super) struct Child {
    pub(super) source: Option<u64>,
    pub(super) geometry: Kid,
    pub(super) growth: Option<GrowthSnapshot>,
}

impl Cache {
    pub(super) fn validate(&self, window: &Window) {
        let inputs = (f32::from(window.rem_size()), window.scale_factor());
        if self.window.replace(Some(inputs)) != Some(inputs) {
            self.plans.borrow_mut().clear();
            self.extents.borrow_mut().clear();
        }
    }
}

impl Key {
    pub(super) fn new(stack: &ColumnStack, width: f32) -> Option<Self> {
        if stack.fixed_height.is_some()
            || stack.rows.is_some()
            || stack.row_phase != 0.
            || stack.row_limit.is_some()
            || stack.fill_shrink
            || stack.discard
            || stack.nest_report.is_some()
            || stack.lead_report.is_some()
        {
            return None;
        }
        Some(Self {
            width,
            count: stack.count,
            gap: stack.gap,
            content_extent: stack.row_extent
                == crate::layout::multicol::measure::RowExtent::Content,
        })
    }
}
