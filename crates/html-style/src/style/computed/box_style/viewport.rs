//! Viewport units of a box style: resolved without copying the shared computed style when
//! nothing needs resolving; the insets of propagated decorations (`decors`) are resolved in
//! the box's own list.

use super::*;

impl BoxStyle {
    /// `Computed::resolve_viewport` without a copy of the shared style when nothing needs resolving.
    pub fn resolve_viewport(&mut self, viewport: (f32, f32), vcx: crate::values::ValueCx<'_>) {
        // A paint with no lengths to resolve stays shared (`SharedPaints`).
        if let Some(paint) = &mut self.paint
            && paint.background.has_lengths()
        {
            let mut own = (**paint).clone();
            own.background.resolve_viewport(&self.style, viewport, vcx);
            *paint = vcx.doc.traversal.paints.share(own);
        }
        if self.style.has_viewport_lengths(vcx) {
            Rc::make_mut(&mut self.style).resolve_viewport(viewport, vcx);
        }
        self.resolve_decors_viewport(viewport, vcx);
    }

    /// `Computed::resolve_decor_inset_viewport` without a copy of the shared style when nothing needs resolving.
    pub fn resolve_decor_inset_viewport(
        &mut self,
        viewport: (f32, f32),
        vcx: crate::values::ValueCx<'_>,
    ) {
        if self.style.has_decor_insets() {
            Rc::make_mut(&mut self.style).resolve_decor_inset_viewport(viewport, vcx);
        }
        self.resolve_decors_viewport(viewport, vcx);
    }

    fn resolve_decors_viewport(&mut self, viewport: (f32, f32), vcx: crate::values::ValueCx<'_>) {
        if self.decors.iter().all(|d| d.inset.is_none()) {
            return;
        }
        let vp = self.style.viewport_cx(viewport, vcx);
        let fix = |length: &mut Option<crate::style::values::value::Len>| {
            if let Some(v) = *length {
                *length = Some(v.fold_viewport(vp, vcx));
            }
        };
        super::super::resolve::decor_inset::resolve_decor_list(
            &mut self.decors,
            |l| l.folds_viewport(vcx),
            fix,
            vcx,
        );
    }

    /// A propagated underline reaches the box (css-text-decor-3 §2.1).
    pub fn underline(&self) -> bool {
        self.decors.iter().any(|d| d.lines & DECOR_UNDER != 0)
    }

    /// A propagated line-through reaches the box (css-text-decor-3 §2.1).
    pub fn line_through(&self) -> bool {
        self.decors.iter().any(|d| d.lines & DECOR_THROUGH != 0)
    }
}
