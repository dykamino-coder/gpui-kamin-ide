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
use gpui::{App, Window, px, size};

pub(super) struct FitAtom {
    /// Index into `atoms` / `atom_boxes`.
    k: usize,
    /// Spacer byte of the atom (`AtomBox::at`).
    at: usize,
    /// Margin-box min-content width.
    min: f32,
    /// Margin-box max-content width and baseline of the real layout.
    max: f32,
    base: f32,
    /// Copy of the atom wrapper tree and its max-content height/baseline.
    snap: gpui::LayoutMeasurement,
    snap_h: f32,
    snap_base: Option<f32>,
}

/// Resolved atom geometry for one available width: (index, width, height,
/// baseline).
pub(super) type Fitted = Vec<(usize, f32, f32, f32)>;

#[derive(Default)]
pub(super) struct AtomFit {
    atoms: Vec<FitAtom>,
    cache: Vec<(f32, Fitted)>,
}

/// The wrapper (`Paragraph::atoms`) is a row of [probe, atom]: the atom is
/// child 1.
const ATOM_CHILD: usize = 1;

impl AtomFit {
    pub(super) fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }

    /// Atom geometry when the containing block is `avail` wide (`f32::INFINITY`
    /// for max-content, 0 for min-content).
    pub(super) fn fit(&mut self, avail: f32, window: &mut Window, cx: &mut App) -> Fitted {
        if let Some((_, f)) = self.cache.iter().find(|(a, _)| (a - avail).abs() < 1e-3) {
            return f.clone();
        }
        let mut out = Vec::new();
        for a in self.atoms.iter_mut() {
            if avail >= a.max - 0.01 {
                continue;
            }
            let w = avail.max(a.min);
            if w >= a.max - 0.01 {
                continue;
            }
            a.snap.cap_child_outer_width(ATOM_CHILD, px(w));
            let (s, b, _) = a.snap.measure(
                size(
                    gpui::AvailableSpace::Definite(px(w)),
                    gpui::AvailableSpace::MaxContent,
                ),
                window,
                cx,
            );
            let h = f32::from(s.height);
            let base = match (b, a.snap_base) {
                (Some(b), Some(b0)) => a.base + (f32::from(b) - b0),
                _ => a.base + (h - a.snap_h),
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
                let max = f32::from(window.layout_exact(root).1.width);
                let mut snap = window.snapshot_layout_measurement(root);
                let (smin, _, _) = snap.measure(
                    size(
                        gpui::AvailableSpace::MinContent,
                        gpui::AvailableSpace::MaxContent,
                    ),
                    window,
                    cx,
                );
                let min = f32::from(smin.width);
                if min >= max - 0.01 {
                    continue;
                }
                let (smax, sbase, _) = snap.measure(
                    size(
                        gpui::AvailableSpace::MaxContent,
                        gpui::AvailableSpace::MaxContent,
                    ),
                    window,
                    cx,
                );
                fit.atoms.push(FitAtom {
                    k,
                    at: b.at,
                    min,
                    max,
                    base: b.base,
                    snap,
                    snap_h: f32::from(smax.height),
                    snap_base: sbase.map(f32::from),
                });
            }
        }
        self.atom_fit = std::rc::Rc::new(std::cell::RefCell::new(fit));
    }

    /// Put fitted atom sizes into the spacer advances and atom boxes.
    pub(super) fn apply_atom_fit(&mut self, fitted: &Fitted) {
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
        for &(k, w, _, _) in &fitted {
            let Some(slot) = self.atoms.get_mut(k) else {
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
