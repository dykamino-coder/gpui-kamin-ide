//! Owned rebuild of a paginated stack child's copies (`fragment::paginate`).

use super::build::build_copy;
use crate::dom::Element;
use crate::layout::fragment::line_shape::side_margin_wrap;
use crate::layout::fragment::paginate::{MARK, Paginate, apply, candidates, watch};
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
    inner: &Computed,
) -> Option<Paginate> {
    if col_vert || copy.attr(MARK).is_none() {
        return None;
    }
    let cands = candidates(copy)?;
    let ids: Vec<u64> = cands.iter().map(|c| c.0).collect();
    let (opts, merged, copy, inner) = (opts.clone(), merged.clone(), copy.clone(), inner.clone());
    Some(Paginate {
        cands,
        rebuild: Box::new(move |struts, n| {
            let mut c2 = copy.clone();
            if !apply(&mut c2, struts) {
                return None;
            }
            watch::building(&ids, || {
                let copy_ix = std::cell::Cell::new(0usize);
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
                            &copy_ix,
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
            })
        }),
    })
}
