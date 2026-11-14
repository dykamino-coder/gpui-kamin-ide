//! Числовые распорки строк: адреса абзацев локальны неизменному дереву потока.

use super::FlowMeasure;
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct GrowthSnapshot {
    paragraphs: Vec<Vec<f32>>,
    contexts: BTreeMap<usize, GrowthSnapshot>,
}

impl FlowMeasure {
    pub(crate) fn snapshot_growth(&self) -> GrowthSnapshot {
        GrowthSnapshot {
            paragraphs: self
                .paragraphs
                .borrow()
                .iter()
                .map(|p| p.borrow().clone())
                .collect(),
            contexts: self
                .contexts
                .borrow()
                .iter()
                .map(|(&key, flow)| (key, flow.snapshot_growth()))
                .collect(),
        }
    }

    /// Только для нового потока того же дерева; рекордеры и курсоры не переносятся.
    pub(crate) fn restore_growth(&self, snapshot: &GrowthSnapshot) {
        let mut paragraphs = self.paragraphs.borrow_mut();
        for (i, value) in snapshot.paragraphs.iter().enumerate() {
            if i == paragraphs.len() {
                paragraphs.push(Rc::default());
            }
            let paragraph: &Rc<std::cell::RefCell<Vec<f32>>> = &paragraphs[i];
            *paragraph.borrow_mut() = value.clone();
        }
        for (&key, snapshot) in &snapshot.contexts {
            self.context(key).restore_growth(snapshot);
        }
    }
}
