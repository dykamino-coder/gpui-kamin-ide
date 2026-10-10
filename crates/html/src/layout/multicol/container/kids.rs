//! Дети стопки колонок: копии детей при заданной высоте и элементы StackChild.

use crate::dom::Element;
use crate::layout::fragment::breaks::edge_break;
use crate::layout::fragment::flex_lines::split_flex_lines;
use crate::layout::fragment::line_shape::nested_rows_box;
use crate::layout::fragment::push::{avoid_only_monolith, grow_pushed};
use crate::layout::multicol::stack_child::multicol_stack_child;
use crate::render::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
pub(super) fn column_kids(
    merged: &Computed,
    kids: Vec<(
        Element,
        (f32, f32, f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>),
    )>,
    cols: u16,
    used_gap: f32,
    col_vert: bool,
    rows: Option<crate::layout::fragment::types::Rows>,
    copies: usize,
    fixed: Option<f32>,
) -> (
    Vec<Option<Computed>>,
    Vec<usize>,
    Vec<crate::layout::fragment::types::Par>,
    Vec<(
        Element,
        (f32, f32, f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>),
    )>,
) {
    let (kids, kid_par, kid_parent, kid_starts) = if fixed.is_some() && rows.is_none() && !col_vert
    {
        let col_w = match merged.width {
            Some(Len::Px(w)) if merged.border_box != Some(true) && cols > 0 => {
                Some((w - used_gap * (cols as f32 - 1.0)) / cols as f32)
            }
            _ => None,
        };
        split_flex_lines(kids, col_w, merged)
    } else {
        let n = kids.len();
        (
            kids,
            vec![crate::layout::fragment::types::Par::default(); n],
            vec![None; n],
            (0..=n).collect(),
        )
    };
    // `break-inside: avoid` без настоящего монолита — у любого
    // ребёнка колонок (`flow::Par::avoid_only`): с верха колонки
    // коробка выше колонки рвётся, а не переполняет её.
    let mut kid_par = kid_par;
    for (p, (c, _)) in kid_par.iter_mut().zip(kids.iter()) {
        if p.group == 0 {
            p.avoid_only = c.style.break_inside_avoid && avoid_only_monolith(c);
            p.float = c.attr("kamin-float-block").is_some();
            p.clears = c.style.clear.is_some();
        }
    }
    let kids = if col_vert {
        kids
    } else {
        grow_pushed(kids, cols as usize, fixed, rows, copies, &kid_par)
    };
    (kid_parent, kid_starts, kid_par, kids)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn stack_children(
    e: &Element,
    merged: &Computed,
    opts: &RenderOpts,
    cols: u16,
    col_vert: bool,
    col_rl: bool,
    line_col_w: Option<f32>,
    rows: Option<crate::layout::fragment::types::Rows>,
    nested_auto: std::cell::RefCell<Vec<u64>>,
    nested_whole: std::cell::RefCell<Vec<u64>>,
    measured_kids: std::cell::RefCell<Vec<(u64, f32)>>,
    copies: usize,
    fixed: Option<f32>,
    kid_parent: Vec<Option<Computed>>,
    kid_par: Vec<crate::layout::fragment::types::Par>,
    kids: Vec<(
        Element,
        (f32, f32, f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>),
    )>,
    clone_plan: Vec<Vec<(f32, f32)>>,
) -> Vec<crate::layout::fragment::types::StackChild> {
    let balanced_frag: Option<f32> =
        (fixed.is_none() && rows.is_none_or(|r| r.cap) && cols > 1 && kids.len() == 1)
            .then(|| {
                let (c, s) = &kids[0];
                (nested_rows_box(c)
                    && matches!(c.style.height, Some(Len::Px(_)))
                    && s.3.is_empty()
                    && s.1.abs() < 0.01)
                    .then(|| {
                        let per = s.0 / cols as f32;
                        rows.and_then(|r| r.h).map_or(per, |cap| per.min(cap))
                    })
            })
            .flatten()
            .filter(|h| *h > 1.0);
    let fixed_nest = fixed.or(balanced_frag);
    let nest_at: Vec<Option<f32>> = {
        let mut v = Vec::with_capacity(kids.len());
        let (mut y, mut prev_mb, mut ok) = (0.0f32, 0.0f32, true);
        for (i, (c, s)) in kids.iter().enumerate() {
            let lead = if i == 0 { s.1 } else { prev_mb.max(s.1) };
            let hh = fixed_nest.unwrap_or(0.0);
            v.push((ok && fixed_nest.is_some() && y + lead < hh - 0.01).then_some(y + lead));
            if edge_break(c, false)
                || edge_break(c, true)
                || kid_par.get(i).is_some_and(|p| p.group != 0)
                || y + lead + s.0 > hh + 0.01
            {
                ok = false;
            }
            y += lead + s.0;
            prev_mb = s.2;
        }
        v
    };
    let children: Vec<crate::layout::fragment::types::StackChild> = kids
        .into_iter()
        .enumerate()
        .map(|(ix, (c, (h, mt, mb, cuts, forced, solid)))| {
            multicol_stack_child(
                ix,
                c,
                h,
                mt,
                mb,
                cuts,
                forced,
                solid,
                e,
                merged,
                opts,
                &clone_plan,
                &kid_parent,
                &kid_par,
                col_vert,
                col_rl,
                line_col_w,
                rows,
                copies,
                fixed,
                fixed_nest,
                balanced_frag,
                &nest_at,
                &nested_auto,
                &nested_whole,
                &measured_kids,
            )
        })
        .collect();
    children
}
