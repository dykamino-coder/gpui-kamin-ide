//! Math function trees (`Len::Math`, css-values-4 §10.10) in the explicit
//! length context and in the viewport and container folds of the cascade.

use super::len_cx::LenCx;
use super::len_px::FontCx;
use crate::style::values::value::{Len, Sum};
use crate::values::ValueCx;

impl LenCx<'_> {
    /// The length-percentage terms of a tree the cascade did not fold (a node
    /// outside inheritance): the context viewport and font are substituted
    /// without arena writes.
    pub(super) fn math_terms(self, l: Len, vcx: ValueCx<'_>) -> Option<(f32, f32)> {
        let mut viewport = false;
        let sum = l.math_sum(vcx, |s| {
            let folded = self
                .viewport
                .is_some_and(|vp| s.fold_viewport(vp.size, vp.vertical, vp.container));
            viewport |= folded;
            let font = self.font.is_some_and(|f| {
                s.fold_font(f.font_px, || glyph_units(f, vcx))
                    | f.line_px.is_some_and(|line| s.fold_line(line))
            });
            folded | font
        });
        if viewport {
            crate::values::viewport_reads::note(vcx);
        }
        sum?.length_percentage()
    }
}

/// A viewport or container fold of a tree: a folded leaf is a viewport read
/// (an axis without a query container is the viewport's).
pub(super) fn fold_math(l: Len, vcx: ValueCx<'_>, mut fold: impl FnMut(&mut Sum) -> bool) -> Len {
    let mut folded = false;
    let resolved = l.resolve_math(vcx, |s| {
        let one = fold(s);
        folded |= one;
        one
    });
    if folded {
        crate::values::viewport_reads::note(vcx);
    }
    resolved
}

/// Glyph metrics of the context as `[cap, ch, ex, ic]`, the `glyph_units` order.
fn glyph_units(font: FontCx<'_>, vcx: ValueCx<'_>) -> [f32; 4] {
    font.units.unwrap_or_else(|| {
        let (ch, ex) = crate::style::font_metrics::horizontal(font.family, font.metric_px, vcx);
        [
            crate::style::font_metrics::vertical(font.family, font.metric_px, vcx).2,
            ch,
            ex,
            crate::style::font_metrics::ideographic(font.family, font.metric_px, vcx),
        ]
    })
}
