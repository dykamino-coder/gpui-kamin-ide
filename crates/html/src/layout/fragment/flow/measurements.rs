//! Чтение измеренной геометрии разрывов до того, как временный движок layout будет освобождён.

use super::FlowGeometry;
use crate::layout::fragment::types::StackAxis;
use gpui::{LayoutId, Pixels, Point, Size, Window};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub(crate) struct FlowNode {
    pub(crate) measurements: Rc<FlowMeasurements>,
    pub(crate) geometry: Rc<RefCell<FlowGeometry>>,
    pub(crate) axis: StackAxis,
}

impl FlowNode {
    pub(crate) fn new(measurements: Rc<FlowMeasurements>, axis: StackAxis) -> Self {
        Self {
            measurements,
            geometry: Default::default(),
            axis,
        }
    }

    pub(crate) fn record(&self, id: LayoutId) {
        self.measurements.records.borrow_mut().push(Record {
            id,
            axis: self.axis,
            geometry: self.geometry.clone(),
        });
    }
}

// The entries are scoped to the layout engine that owns their IDs. Only
// FlowGeometry (plain numbers) may be retained in a BandFlow plan.
#[derive(Debug, Default)]
pub(crate) struct FlowMeasurements {
    records: RefCell<Vec<Record>>,
    pub(crate) lines: bool,
}

#[derive(Debug)]
struct Record {
    id: LayoutId,
    axis: StackAxis,
    geometry: Rc<RefCell<FlowGeometry>>,
}

impl FlowMeasurements {
    pub(crate) fn reset(&self) {
        self.records.borrow_mut().clear();
    }

    pub(crate) fn for_lines() -> Self {
        Self {
            lines: true,
            ..Self::default()
        }
    }

    pub(crate) fn enter(self: &Rc<Self>) -> FlowScope {
        let saved = std::mem::take(&mut *self.records.borrow_mut());
        FlowScope {
            owner: self.clone(),
            saved,
        }
    }

    pub(crate) fn relative(
        &self,
        root: LayoutId,
        axis: StackAxis,
        window: &mut Window,
    ) -> FlowGeometry {
        let origin = window.layout_origin_unrounded(root);
        let size = window.layout_size_unrounded(root);
        self.at(origin, size, axis, window)
    }

    pub(crate) fn at(
        &self,
        origin: Point<Pixels>,
        size: Size<Pixels>,
        axis: StackAxis,
        window: &mut Window,
    ) -> FlowGeometry {
        let mut result = FlowGeometry::default();
        for Record {
            id,
            axis: own_axis,
            geometry,
        } in self.records.borrow().iter()
        {
            if *own_axis != axis {
                continue;
            }
            let at = window.layout_origin_unrounded(*id);
            let offset = match axis {
                StackAxis::Horizontal => at.y - origin.y,
                StackAxis::VerticalLr => at.x - origin.x,
                StackAxis::VerticalRl => {
                    origin.x + size.width - at.x - window.layout_size_unrounded(*id).width
                }
            };
            result.append(&geometry.borrow(), f32::from(offset));
        }
        result.normalize();
        result
    }
}

pub(crate) struct FlowScope {
    owner: Rc<FlowMeasurements>,
    saved: Vec<Record>,
}

impl Drop for FlowScope {
    fn drop(&mut self) {
        *self.owner.records.borrow_mut() = std::mem::take(&mut self.saved);
    }
}
