//! Адрес строки по содержимому сохраняется при смене ширины фрагментаинера.
use super::FlowMeasure;
use crate::layout::fragment::types::StackAxis;
use gpui::{Pixels, Point, Size, Window};
use std::cell::Cell;
use std::rc::Rc;

#[derive(Clone, Debug, Default)]
pub(crate) struct ContentLine {
    pub(crate) paragraph: usize,
    pub(crate) byte: usize,
    pub(crate) top: f32,
    pub(crate) bottom: f32,
}

impl FlowMeasure {
    pub(super) fn paragraph_resume(&self, ordinal: usize) -> Rc<Cell<usize>> {
        let mut resumes = self.resumes.borrow_mut();
        let len = resumes.len().max(ordinal + 1);
        resumes.resize_with(len, Rc::default);
        resumes[ordinal].clone()
    }

    pub(crate) fn resume_lines(&self, lines: &[ContentLine], from: f32) {
        for cell in self.resumes.borrow().iter() {
            cell.set(0);
        }
        let mut first = std::collections::HashMap::<usize, usize>::new();
        for line in lines
            .iter()
            .filter(|l| l.bottom > from + crate::tolerance::COORD_EPS)
        {
            first
                .entry(line.paragraph)
                .and_modify(|b| *b = (*b).min(line.byte))
                .or_insert(line.byte);
        }
        for (paragraph, byte) in first {
            self.paragraph_resume(paragraph).set(byte);
        }
    }

    /// Адреса содержимого используют ту же проекцию, что cuts и распорки.
    pub(crate) fn content_lines_axis(
        &self,
        origin: Point<Pixels>,
        size: Size<Pixels>,
        axis: StackAxis,
        window: &mut Window,
    ) -> Vec<ContentLine> {
        let mut out = Vec::new();
        self.records.for_each(window, |bounds, record| {
            let Some(offset) = record.offset(origin, size, axis, bounds) else {
                return;
            };
            out.extend(record.content.borrow().iter().cloned().map(|mut l| {
                l.top += offset;
                l.bottom += offset;
                l
            }));
        });
        out
    }
}
