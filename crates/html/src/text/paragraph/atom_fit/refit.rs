//! Prepaint of a paragraph with shrink-to-fit atoms (`Paragraph::refit_atoms`):
//! the copies of the atom trees go, the real atoms take their fitted widths.

use super::{ATOM_CHILD, Fitted};
use crate::text::paragraph::{Paragraph, atoms};
use gpui::{App, Window, px, size};

impl Paragraph {
    /// Prepaint: lay the real atoms out at the width the measurement used.
    /// The layout engine is free here, so nothing is read from the copies:
    /// the geometry of the real atoms places and paints them.
    pub(in crate::text::paragraph) fn refit_atoms(
        &mut self,
        avail: f32,
        window: &mut Window,
        cx: &mut App,
    ) {
        if self.atom_fit.borrow().is_empty() {
            return;
        }
        #[cfg(test)]
        let copied = self.atom_fit.borrow_mut().fit(avail, window, cx);
        let widths = {
            let mut fit = self.atom_fit.borrow_mut();
            fit.release();
            fit.widths(avail)
        };
        let mut fitted = Fitted::new();
        for (k, w) in widths {
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
            let row_h = f32::from(window.layout_exact(root).1.height);
            let (h, base, drop) = atoms::row_geometry::row_geometry(Some(root), row_h, window);
            let base = if base <= 0.0 && h > 0.0 { h } else { base };
            fitted.push((k, w, h, base, drop));
        }
        self.apply_atom_fit(&fitted);
        #[cfg(test)]
        {
            let fit = self.atom_fit.borrow();
            let left = fit.atoms.iter().filter(|a| a.snap.is_some()).count();
            super::check::record(copied, fitted, left);
        }
    }
}
