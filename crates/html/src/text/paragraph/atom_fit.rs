//! Ширина атомарных строчных элементов по shrink-to-fit (CSS 2.1 §10.3.9,
//! §10.3.10, §17.5.2: `min(max(min-content, available), max-content)`).
//!
//! Атом раскладывается ДО замера абзаца, по max-content (`lay_atoms`): внутри
//! замера движок раскладки занят. Его доступная ширина — содержащий блок, то
//! есть ширина самого абзаца, а не остаток строки — известна только внутри
//! замера. Поэтому каждый атом, чей min-content уже max-content, хранит
//! независимую копию своего дерева раскладки (`LayoutMeasurement`): замер
//! меряет копию на ширине shrink-to-fit, а prepaint раскладывает настоящий
//! атом на той же ширине. Blink делает то же за один проход: атомарный
//! строчный элемент раскладывается с размером содержащего блока строки как
//! доступным (`inline_layout_algorithm.cc` → `LayoutAtomicInline`).

use super::*;
use crate::tolerance::COORD_EPS;
use gpui::{App, Window, px, size};
use std::cell::RefCell;
use std::rc::Rc;

mod memo;
use memo::AtomMemo;

pub(super) struct FitAtom {
    /// Index into `atoms` / `atom_boxes`.
    k: usize,
    /// Spacer byte of the atom (`AtomBox::at`).
    at: usize,
    /// Baseline of the real max-content layout in this paragraph.
    base: f32,
    /// Measurements of the atom's subtree, shared with later frames.
    memo: Rc<AtomMemo>,
    /// Copy of the atom tree for unseen widths (this frame's measures: not kept).
    snap: gpui::LayoutMeasurement,
}

/// Resolved atom geometry for one available width: (index, width, height, baseline).
pub(super) type Fitted = Vec<(usize, f32, f32, f32)>;

#[derive(Default)]
pub(super) struct AtomFit {
    atoms: Vec<FitAtom>,
    cache: Vec<(f32, Fitted)>,
}

/// The wrapper (`Paragraph::atoms`) is a row of [probe, atom]: the atom is child 1.
const ATOM_CHILD: usize = 1;

impl AtomFit {
    pub(super) fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }

    /// Atom geometry when the containing block is `avail` wide (`f32::INFINITY`
    /// for max-content, 0 for min-content).
    pub(super) fn fit(&mut self, avail: f32, window: &mut Window, cx: &mut App) -> Fitted {
        if let Some((_, f)) = self
            .cache
            .iter()
            .find(|(a, _)| (a - avail).abs() < crate::tolerance::TEXT_COORD_EPS)
        {
            return f.clone();
        }
        let mut out = Vec::new();
        for a in self.atoms.iter_mut() {
            let memo = &*a.memo;
            // `avail` at max-content or wider also gives `w` there.
            let w = avail.max(memo.min);
            if w >= memo.max - COORD_EPS {
                continue;
            }
            let Some((snap_h, snap_base)) = memo.snap else {
                continue;
            };
            let mut seen = memo.at_width.take();
            let (h, b) = match seen.iter().find(|m| m.0 == w) {
                Some(&(_, h, b)) => (h, b),
                None => {
                    a.snap.cap_child_outer_width(ATOM_CHILD, px(w));
                    let (s, b, _) = a.snap.measure(
                        size(
                            gpui::AvailableSpace::Definite(px(w)),
                            gpui::AvailableSpace::MaxContent,
                        ),
                        window,
                        cx,
                    );
                    let (h, b) = (f32::from(s.height), b.map(f32::from));
                    AtomMemo::remember(&mut seen, (w, h, b));
                    (h, b)
                }
            };
            memo.at_width.set(seen);
            let base = match (b, snap_base) {
                (Some(b), Some(b0)) => a.base + (b - b0),
                _ => a.base + (h - snap_h),
            };
            out.push((a.k, w, h, base));
        }
        self.cache.push((avail, out.clone()));
        out
    }
}

impl Paragraph {
    /// Collect the atoms whose width depends on the containing block. Called
    /// at the end of `lay_atoms`, while the layout engine is free.
    pub(super) fn prepare_atom_fit(&mut self, window: &mut Window, cx: &mut App) {
        let mut fit = AtomFit::default();
        if !self.vertical {
            for (k, slot) in self.atoms.iter().enumerate() {
                let (Some(root), Some(b)) = (slot.root.get(), self.atom_boxes.get(k).copied())
                else {
                    continue;
                };
                // A width fixed by style never shrinks to fit (§10.3.9): no
                // copy and no intrinsic measurement. Debug builds still
                // measure to check that min-content equals max-content.
                if slot.fixed_inline && !cfg!(debug_assertions) {
                    continue;
                }
                let max = f32::from(window.layout_exact(root).1.width);
                let kept = window
                    .layout_memo(root)
                    .and_then(|m| m.downcast::<AtomMemo>().ok())
                    .filter(|m| m.max == max);
                if kept.as_ref().is_some_and(|m| m.snap.is_none()) {
                    continue;
                }
                let mut snap = window.snapshot_layout_measurement(root);
                let memo = kept.unwrap_or_else(|| {
                    let memo = Rc::new(AtomMemo::measure(&mut snap, max, window, cx));
                    window.set_layout_memo(root, memo.clone());
                    memo
                });
                if memo.snap.is_none() {
                    continue;
                }
                debug_assert!(
                    !slot.fixed_inline,
                    "style-fixed atom width must not depend on the containing block: min {}, max {max}",
                    memo.min
                );
                fit.atoms.push(FitAtom {
                    k,
                    at: b.at,
                    base: b.base,
                    memo,
                    snap,
                });
            }
        }
        self.atom_fit = Rc::new(RefCell::new(fit));
    }

    /// Put fitted atom sizes into the spacer advances and atom boxes.
    pub(super) fn apply_atom_fit(&mut self, fitted: &Fitted) {
        self.invalidate_prepared();
        let ats: Vec<(usize, usize)> = {
            let fit = self.atom_fit.borrow();
            fit.atoms.iter().map(|a| (a.k, a.at)).collect()
        };
        for &(k, w, h, base) in fitted {
            let Some(&(_, at)) = ats.iter().find(|(i, _)| *i == k) else {
                continue;
            };
            if let Some(b) = self.atom_boxes.get_mut(k) {
                b.h = h;
                b.base = base;
            }
            let len = self.text[at..].chars().next().map_or(0, char::len_utf8);
            if let Some(span) = self
                .letter_spans
                .iter_mut()
                .find(|(r, _)| r.start == at && r.end == at + len)
            {
                span.1 = px(w);
            }
        }
    }

    /// Prepaint: lay the real atoms out at the width the measurement used.
    pub(super) fn refit_atoms(&mut self, avail: f32, window: &mut Window, cx: &mut App) {
        if self.atom_fit.borrow().is_empty() {
            return;
        }
        let fitted = self.atom_fit.borrow_mut().fit(avail, window, cx);
        for (k, slot) in self.atoms.iter_mut().enumerate() {
            let Some(&(_, w, _, _)) = fitted.iter().find(|f| f.0 == k) else {
                continue;
            };
            let Some(root) = slot.root.get() else {
                continue;
            };
            window.cap_layout_child_outer_width(root, ATOM_CHILD, px(w));
            slot.el.layout_as_root(
                size(
                    gpui::AvailableSpace::Definite(px(w)),
                    gpui::AvailableSpace::MaxContent,
                ),
                window,
                cx,
            );
        }
        // Real geometry wins over the copy's for placement and painting.
        let fitted: Fitted = fitted
            .iter()
            .map(|&(k, w, h, base)| {
                let slot = &self.atoms[k];
                let real_h = slot
                    .root
                    .get()
                    .map_or(h, |id| f32::from(window.layout_exact(id).1.height));
                let real_base = slot
                    .probe
                    .get()
                    .map_or(base, |id| f32::from(window.layout_exact(id).0.y));
                let real_base = if real_base <= 0.0 && real_h > 0.0 {
                    real_h
                } else {
                    real_base
                };
                (k, w, real_h, real_base)
            })
            .collect();
        self.apply_atom_fit(&fitted);
    }
}
