//! Результаты текстовой коробки живут до смены её подготовленного IFC.
//! Отложенный абзац держит Rc владельца, не разыменовывает прежний BoxId.

#[cfg(feature = "heap-report")]
mod heap;
pub(super) mod inputs;
mod paint;
#[cfg(test)]
mod tests;

use super::{Line, Paragraph, Seg};
use gpui::{Pixels, Window, WindowTextSystem};
use inputs::Inputs;
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Weak},
};

#[derive(Default)]
pub(crate) struct Retained {
    state: RefCell<State>,
}

#[derive(Default)]
struct State {
    system: Weak<WindowTextSystem>,
    generation: usize,
    shape: Option<(Inputs, Vec<Seg>)>,
    lines: Option<(Inputs, Vec<(Constraint, Rc<Vec<Line>>)>)>,
    paint: Vec<paint::Piece>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Constraint(Option<u32>, Option<u32>);

impl Constraint {
    fn of(p: &Paragraph, limit: Option<Pixels>) -> Self {
        Self(
            limit.map(|v| f32::from(v).to_bits()),
            p.indent_basis.map(|v| f32::from(v).to_bits()),
        )
    }
}

impl State {
    fn validate(&mut self, system: &Arc<WindowTextSystem>) {
        let weak = Arc::downgrade(system);
        let generation = system.font_generation();
        if !self.system.ptr_eq(&weak) || self.generation != generation {
            *self = Self {
                system: weak,
                generation,
                ..Default::default()
            };
        }
    }

    fn matches(&self, p: &Paragraph) -> bool {
        self.shape
            .as_ref()
            .is_some_and(|(inputs, _)| inputs.matches_shape(p))
    }
}

impl Retained {
    pub(super) fn measured(&self, p: &Paragraph, window: &Window) -> Option<Vec<Seg>> {
        let mut state = self.state.borrow_mut();
        state.validate(window.text_system());
        state
            .matches(p)
            .then(|| state.shape.as_ref().unwrap().1.clone())
    }

    pub(super) fn remember_measure(&self, p: &Paragraph, window: &Window, segs: &[Seg]) {
        let mut state = self.state.borrow_mut();
        state.validate(window.text_system());
        if !state.matches(p) {
            state.lines = None;
            state.paint.clear();
            state.shape = Some((Inputs::shape(p), segs.to_vec()));
        }
    }

    pub(super) fn lines(
        &self,
        p: &Paragraph,
        limit: Option<Pixels>,
        window: &Window,
    ) -> Option<Vec<Line>> {
        let mut state = self.state.borrow_mut();
        state.validate(window.text_system());
        if !state.matches(p) {
            return None;
        }
        let (inputs, entries) = state.lines.as_ref()?;
        if !inputs.matches_lines(p) {
            return None;
        }
        entries
            .iter()
            .find(|(key, _)| *key == Constraint::of(p, limit))
            .map(|(_, lines)| (**lines).clone())
    }

    pub(super) fn remember_lines(&self, p: &Paragraph, limit: Option<Pixels>, lines: &[Line]) {
        let mut state = self.state.borrow_mut();
        // split_uncached measured this exact paragraph before producing its lines.
        if !state.matches(p) {
            return;
        }
        if !state
            .lines
            .as_ref()
            .is_some_and(|(inputs, _)| inputs.matches_lines(p))
        {
            state.lines = Some((Inputs::lines(p), Vec::new()));
        }
        let entries = &mut state.lines.as_mut().unwrap().1;
        let key = Constraint::of(p, limit);
        if entries.iter().any(|(old, _)| *old == key) {
            return;
        }
        // Min/max/definite probes; unusual extra widths do not grow retention.
        if entries.len() == 3 {
            entries.remove(0);
        }
        entries.push((key, Rc::new(lines.to_vec())));
    }
}

impl Paragraph {
    pub(crate) fn retained(mut self, owner: Option<Rc<Retained>>) -> Self {
        self.retained = owner;
        self
    }

    pub(super) fn retained_lines(&self) -> Option<&Retained> {
        // Float damage and resumable fragmentation keep the existing path.
        (self.flow.0.is_empty() && self.flow.1.is_empty() && self.fragment.is_none())
            .then_some(self.retained.as_deref())
            .flatten()
    }
}
