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
    drop: f32,
    /// Copy of the atom wrapper tree and its max-content height/baseline.
    snap: gpui::LayoutMeasurement,
    snap_h: f32,
    snap_base: Option<f32>,
}

/// Resolved atom geometry for one available width: (index, width, height,
/// baseline, downward shift in the wrapper row — AtomBox::drop).
pub(super) type Fitted = Vec<(usize, f32, f32, f32, f32)>;

#[derive(Default)]
pub(super) struct AtomFit {
    atoms: Vec<FitAtom>,
    cache: Vec<(f32, Fitted)>,
}

/// The wrapper (`Paragraph::atoms`) is a baseline row whose only child is the
/// atom.
const ATOM_CHILD: usize = 0;

impl AtomFit {
    pub(super) fn is_empty(&self) -> bool {
        self.atoms.is_empty()
    }

    /// Atom geometry when the containing block is `avail` wide (`f32::INFINITY`
    /// for max-content, 0 for min-content).
    pub(super) async fn fit(
        &mut self,
        avail: f32,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) -> Fitted {
        if let Some((_, f)) = self
            .cache
            .iter()
            .find(|(a, _)| (a - avail).abs() < crate::tolerance::TEXT_COORD_EPS)
        {
            return f.clone();
        }
        let mut out = Vec::new();
        for a in self.atoms.iter_mut() {
            if avail >= a.max - COORD_EPS {
                continue;
            }
            let w = avail.max(a.min);
            if w >= a.max - COORD_EPS {
                continue;
            }
            a.snap.cap_child_outer_width(ATOM_CHILD, px(w));
            let (s, b, _) = a
                .snap
                .measure_async(
                    size(
                        gpui::AvailableSpace::Definite(px(w)),
                        gpui::AvailableSpace::MaxContent,
                    ),
                    window,
                    cx,
                    stack,
                )
                .await;
            let h = f32::from(s.height);
            let base = match (b, a.snap_base) {
                (Some(b), Some(b0)) => a.base + (f32::from(b) - b0),
                _ => a.base + (h - a.snap_h),
            };
            out.push((a.k, w, h, base, a.drop));
        }
        self.cache.push((avail, out.clone()));
        out
    }
}

impl Paragraph {
    /// Collect the atoms whose width depends on the containing block. Called
    /// at the end of `lay_atoms`, while the layout engine is free.
    pub(super) async fn prepare_atom_fit(
        &mut self,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
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
                let mut snap = window.snapshot_layout_measurement(root);
                let (smin, _, _) = snap
                    .measure_async(
                        size(
                            gpui::AvailableSpace::MinContent,
                            gpui::AvailableSpace::MaxContent,
                        ),
                        window,
                        cx,
                        stack,
                    )
                    .await;
                let min = f32::from(smin.width);
                if min >= max - COORD_EPS {
                    continue;
                }
                debug_assert!(
                    !slot.fixed_inline,
                    "style-fixed atom width must not depend on the containing block: min {min}, max {max}"
                );
                let (smax, sbase, _) = snap
                    .measure_async(
                        size(
                            gpui::AvailableSpace::MaxContent,
                            gpui::AvailableSpace::MaxContent,
                        ),
                        window,
                        cx,
                        stack,
                    )
                    .await;
                fit.atoms.push(FitAtom {
                    k,
                    at: b.at,
                    min,
                    max,
                    base: b.base,
                    drop: b.drop,
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
        self.invalidate_prepared();
        let ats: Vec<(usize, usize)> = {
            let fit = self.atom_fit.borrow();
            fit.atoms.iter().map(|a| (a.k, a.at)).collect()
        };
        for &(k, w, h, base, drop) in fitted {
            let Some(&(_, at)) = ats.iter().find(|(i, _)| *i == k) else {
                continue;
            };
            if let Some(b) = self.atom_boxes.get_mut(k) {
                b.h = h;
                b.base = base;
                b.drop = drop;
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
}

mod refit;
