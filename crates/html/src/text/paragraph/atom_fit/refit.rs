//! Prepaint атомов на ширине независимого измерения.

use super::*;

impl Paragraph {
    /// Prepaint: lay the real atoms out at the width the measurement used.
    pub(in crate::text::paragraph) async fn refit_atoms(
        &mut self,
        avail: f32,
        window: &mut Window,
        cx: &mut App,
        stack: &mut gpui::PaintStack,
    ) {
        if self.atom_fit.borrow().is_empty() {
            return;
        }
        let fitted = self
            .atom_fit
            .borrow_mut()
            .fit(avail, window, cx, stack)
            .await;
        for &(k, w, _, _, _) in &fitted {
            let Some(slot) = self.atoms.get_mut(k) else {
                continue;
            };
            let Some(root) = slot.root.get() else {
                continue;
            };
            window.cap_layout_child_outer_width(root, ATOM_CHILD, px(w));
            slot.el
                .layout_as_root_async(
                    size(
                        gpui::AvailableSpace::Definite(px(w)),
                        gpui::AvailableSpace::MaxContent,
                    ),
                    window,
                    cx,
                    stack,
                )
                .await;
        }
        // Real geometry wins over the copy's for placement and painting.
        let fitted: Fitted = fitted
            .iter()
            .map(|&(k, w, h, base, drop)| {
                let root = self.atoms[k].root.get();
                let (real_h, real_base, drop) = match root {
                    Some(id) => {
                        let row_h = f32::from(window.layout_exact(id).1.height);
                        atoms::row_geometry::row_geometry(Some(id), row_h, window)
                    }
                    None => (h, base, drop),
                };
                let real_base = if real_base <= 0.0 && real_h > 0.0 {
                    real_h
                } else {
                    real_base
                };
                (k, w, real_h, real_base, drop)
            })
            .collect();
        self.apply_atom_fit(&fitted);
    }
}
