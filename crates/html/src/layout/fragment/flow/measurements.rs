//! Чтение измеренной геометрии разрывов до того, как временный движок layout будет освобождён.

use super::FlowGeometry;
use crate::layout::fragment::types::StackAxis;
use crate::layout::probe::{Measure, MeasureScope};
use gpui::{LayoutId, Pixels, Point, Size, Window};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub(crate) struct FlowNode {
    pub(crate) measurements: Rc<FlowMeasure>,
    pub(crate) geometry: Rc<RefCell<FlowGeometry>>,
    pub(crate) axis: StackAxis,
}

impl FlowNode {
    pub(crate) fn new(measurements: Rc<FlowMeasure>, axis: StackAxis) -> Self {
        Self {
            measurements,
            geometry: Default::default(),
            axis,
        }
    }

    pub(crate) fn record(&self, id: LayoutId) {
        self.measurements.records.record(
            id,
            Record {
                axis: self.axis,
                geometry: self.geometry.clone(),
            },
        );
    }
}

// Только числовая FlowGeometry переживает временный layout-движок.
#[derive(Debug, Default)]
pub(crate) struct FlowMeasure {
    records: Measure<Record>,
    pub(crate) lines: bool,
}

#[derive(Debug)]
pub(crate) struct Record {
    axis: StackAxis,
    geometry: Rc<RefCell<FlowGeometry>>,
}

impl FlowMeasure {
    pub(crate) fn reset(&self) {
        self.records.reset();
    }

    pub(crate) fn for_lines() -> Self {
        Self {
            lines: true,
            ..Self::default()
        }
    }

    pub(crate) fn enter(&self) -> MeasureScope<Record> {
        self.records.enter()
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
        self.records.for_each(
            window,
            |bounds,
             Record {
                 axis: own_axis,
                 geometry,
             }| {
                if *own_axis != axis {
                    return;
                }
                let at = bounds.origin;
                let offset = match axis {
                    StackAxis::Horizontal => at.y - origin.y,
                    StackAxis::VerticalLr => at.x - origin.x,
                    StackAxis::VerticalRl => origin.x + size.width - at.x - bounds.size.width,
                };
                result.append(&geometry.borrow(), f32::from(offset));
            },
        );
        result.normalize();
        result
    }
}
