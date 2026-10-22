//! Чтение измеренной геометрии разрывов до того, как временный движок layout будет освобождён.

use super::FlowGeometry;
use crate::layout::fragment::types::StackAxis;
use crate::layout::probe::{Measure, MeasureScope};
use gpui::{LayoutId, Pixels, Point, Size, Window};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

type ParagraphGrowth = Rc<RefCell<Vec<f32>>>;

#[derive(Clone, Debug)]
pub(crate) struct FlowNode {
    pub(crate) measurements: Rc<FlowMeasure>,
    pub(crate) geometry: Rc<RefCell<FlowGeometry>>,
    pub(crate) axis: StackAxis,
    pub(crate) growth: ParagraphGrowth,
    pub(crate) starts: Rc<RefCell<Vec<f32>>>,
    pub(crate) content: Rc<RefCell<Vec<super::ContentLine>>>,
    pub(crate) resume: Rc<Cell<usize>>,
    pub(crate) ordinal: usize,
}

impl FlowNode {
    pub(crate) fn new(measurements: Rc<FlowMeasure>, axis: StackAxis) -> Self {
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
        self.measurements.records.record(
            id,
            Record {
                axis: self.axis,
                geometry: self.geometry.clone(),
                growth: self.growth.clone(),
                starts: self.starts.clone(),
                content: self.content.clone(),
            },
        );
    }
}

// Только числовая FlowGeometry переживает временный layout-движок.
#[derive(Debug, Default)]
pub(crate) struct FlowMeasure {
    pub(super) records: Measure<Record>,
    pub(super) resumes: RefCell<Vec<Rc<Cell<usize>>>>,
    pub(crate) lines: bool,
    paragraphs: Rc<RefCell<Vec<ParagraphGrowth>>>,
    next: Cell<usize>,
}

#[derive(Debug)]
pub(crate) struct Record {
    axis: StackAxis,
    geometry: Rc<RefCell<FlowGeometry>>,
    growth: ParagraphGrowth,
    starts: Rc<RefCell<Vec<f32>>>,
    pub(super) content: Rc<RefCell<Vec<super::ContentLine>>>,
}

impl FlowMeasure {
    fn paragraph_growth(&self) -> Rc<RefCell<Vec<f32>>> {
        let index = self.next.get();
        self.next.set(index + 1);
        let mut paragraphs = self.paragraphs.borrow_mut();
        if index == paragraphs.len() {
            paragraphs.push(Rc::default());
        }
        paragraphs[index].clone()
    }

    /// Повторная сборка потока сохраняет уже потреблённое место каждого абзаца.
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

    /// Геометрия строк остаётся в области текущего layout-движка.
    pub(crate) fn line_pushes_axis(
        &self,
        origin: Point<Pixels>,
        size: Size<Pixels>,
        axis: StackAxis,
        window: &mut Window,
    ) -> Vec<super::LinePush> {
        let mut pushes = Vec::new();
        self.records.for_each(window, |bounds, record| {
            let Some(offset) = record.offset(origin, size, axis, bounds) else { return; };
            pushes.extend(record.starts.borrow().iter().enumerate().map(|(line, at)| {
                super::LinePush { at: offset + at, line, growth: record.growth.clone() }
            }));
        });
        pushes
    }

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
        self.records.for_each(window, |bounds, record| {
            if let Some(offset) = record.offset(origin, size, axis, bounds) {
                result.append(&record.geometry.borrow(), offset);
            }
        });
        result.normalize();
        result
    }
}

impl Record {
    pub(super) fn offset(
        &self,
        origin: Point<Pixels>,
        size: Size<Pixels>,
        axis: StackAxis,
        bounds: gpui::Bounds<Pixels>,
    ) -> Option<f32> {
        if self.axis != axis { return None; }
        Some(super::coordinates::block_offset(origin, size, bounds.origin, bounds.size, axis))
    }
}
