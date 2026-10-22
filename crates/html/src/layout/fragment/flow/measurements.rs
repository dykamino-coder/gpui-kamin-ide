//! Чтение измеренной геометрии разрывов до того, как временный движок layout будет освобождён.

use super::FlowGeometry;
use crate::layout::fragment::types::StackAxis;
use gpui::{LayoutId, Pixels, Point, Size, Window};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

type ParagraphGrowth = Rc<RefCell<Vec<f32>>>;

#[derive(Clone, Debug)]
pub(crate) struct FlowNode {
    pub(crate) measurements: Rc<FlowMeasurements>,
    pub(crate) geometry: Rc<RefCell<FlowGeometry>>,
    pub(crate) axis: StackAxis,
    pub(crate) growth: Rc<RefCell<Vec<f32>>>,
    pub(crate) starts: Rc<RefCell<Vec<f32>>>,
    pub(crate) content: Rc<RefCell<Vec<super::ContentLine>>>,
    pub(crate) resume: Rc<Cell<usize>>,
    pub(crate) ordinal: usize,
}

impl FlowNode {
    pub(crate) fn new(measurements: Rc<FlowMeasurements>, axis: StackAxis) -> Self {
        let ordinal = measurements.next.get();
        let resume = measurements.paragraph_resume(ordinal);
        let growth = measurements.paragraph_growth();
        Self {
            growth,
            ordinal,
            resume,
            content: Default::default(),
            starts: Default::default(),
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
            growth: self.growth.clone(),
            starts: self.starts.clone(),
            content: self.content.clone(),
        });
    }
}

// The entries are scoped to the layout engine that owns their IDs. Only
// FlowGeometry (plain numbers) may be retained in a BandFlow plan.
#[derive(Debug, Default)]
pub(crate) struct FlowMeasurements {
    pub(super) records: RefCell<Vec<Record>>,
    pub(super) resumes: RefCell<Vec<Rc<Cell<usize>>>>,
    pub(crate) lines: bool,
    paragraphs: Rc<RefCell<Vec<ParagraphGrowth>>>,
    next: Cell<usize>,
}

#[derive(Debug)]
pub(super) struct Record {
    pub(super) id: LayoutId,
    axis: StackAxis,
    geometry: Rc<RefCell<FlowGeometry>>,
    growth: Rc<RefCell<Vec<f32>>>,
    starts: Rc<RefCell<Vec<f32>>>,
    pub(super) content: Rc<RefCell<Vec<super::ContentLine>>>,
}

impl FlowMeasurements {
    fn paragraph_growth(&self) -> Rc<RefCell<Vec<f32>>> {
        let index = self.next.get();
        self.next.set(index + 1);
        let mut paragraphs = self.paragraphs.borrow_mut();
        if index == paragraphs.len() {
            paragraphs.push(Rc::default());
        }
        paragraphs[index].clone()
    }

    /// Rebuilding the same local flow preserves each paragraph's consumed space.
    pub(crate) fn rewind(&self) {
        self.next.set(0);
    }

    pub(crate) fn constraints_only(&self) -> Self {
        Self {
            lines: self.lines,
            paragraphs: self.paragraphs.clone(),
            ..Self::default()
        }
    }

    pub(crate) fn line_pushes(
        &self,
        origin: Point<Pixels>,
        window: &mut Window,
    ) -> Vec<super::LinePush> {
        self.line_pushes_axis(origin, Size::default(), StackAxis::Horizontal, window)
    }

    /// Строки и их распорки в той же логической оси, что геометрия потока.
    pub(crate) fn line_pushes_axis(
        &self,
        origin: Point<Pixels>,
        size: Size<Pixels>,
        axis: StackAxis,
        window: &mut Window,
    ) -> Vec<super::LinePush> {
        self.records
            .borrow()
            .iter()
            .flat_map(|r| {
                let Some(offset) = r.offset(origin, size, axis, window) else {
                    return Vec::new();
                };
                r.starts
                    .borrow()
                    .iter()
                    .enumerate()
                    .map(|(line, at)| super::LinePush {
                        at: offset + at,
                        line,
                        growth: r.growth.clone(),
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

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
        for record in self.records.borrow().iter() {
            if let Some(offset) = record.offset(origin, size, axis, window) {
                result.append(&record.geometry.borrow(), offset);
            }
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

impl Record {
    pub(super) fn offset(
        &self,
        origin: Point<Pixels>,
        size: Size<Pixels>,
        axis: StackAxis,
        window: &mut Window,
    ) -> Option<f32> {
        if self.axis != axis {
            return None;
        }
        Some(super::coordinates::block_offset(
            origin,
            size,
            window.layout_origin_unrounded(self.id),
            window.layout_size_unrounded(self.id),
            axis,
        ))
    }
}
