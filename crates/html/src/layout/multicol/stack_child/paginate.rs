//! Owned rebuild of a paginated stack child's copies (`fragment::paginate`).

use super::build::build_copy;
use crate::dom::Element;
use crate::layout::fragment::line_shape::side_margin_wrap;
use crate::layout::fragment::paginate::{Paginate, apply, avoid_row, candidates, watch};
use crate::render::RenderOpts;
use crate::style::computed::Computed;

/// The rebuild of `copy` (already prepared for the stack: margins and the
/// relative offset taken off) when the measure pass marked it.
pub(super) fn paginate_of(
    opts: &RenderOpts,
    col_vert: bool,
    col_rl: bool,
    line_col_w: Option<f32>,
    merged: &Computed,
    copy: &Element,
) -> Option<Paginate> {
    if col_vert || !copy.marks.paginate {
        return None;
    }
    let cands = candidates(copy)?;
    let avoid_root = avoid_row(copy);
    let ids: Vec<u64> = cands.iter().map(|c| c.0).collect();
    let watched = watch::Watched::new(&ids);
    // Copies are built with the recorder: `render::element` wraps the
    // candidates (`watch::wrap`).
    let mut opts = opts.clone();
    opts.frag.watch = Some(watched.clone());
    let (merged, copy) = (merged.clone(), copy.clone());
    Some(Paginate {
        avoid_root,
        cands,
        watched,
        rebuild: Box::new(move |struts, n| {
            let mut c2 = copy.clone();
            if !apply(&mut c2, struts) {
                return None;
            }
            // The root's own style may carry struts too (row gaps).
            let mut inner = crate::style::cascade::inherit::inherit(&merged, &c2.style);
            inner.fragment_clip_ancestor = false;
            {
                let b = |first: bool, part: usize| {
                    side_margin_wrap(
                        build_copy(
                            &opts,
                            col_vert,
                            col_rl,
                            line_col_w,
                            &[],
                            None,
                            &merged,
                            &c2,
                            0.0,
                            0.0,
                            None,
                            0.0,
                            &inner,
                            false,
                            first,
                            part,
                            false,
                        ),
                        &c2,
                        col_vert,
                    )
                };
                Some((b(true, 0), (1..n.max(1)).map(|i| b(false, i)).collect()))
            }
        }),
    })
}
