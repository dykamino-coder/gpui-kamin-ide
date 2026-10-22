//! Дети стопки колонок: копии детей при заданной высоте и элементы StackChild.

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::fragment::breaks::edge_break;
use crate::layout::fragment::line_shape::nested_rows_box;
use crate::layout::fragment::push::{avoid_only_monolith, grow_pushed, measured_push};
use crate::layout::multicol::measure::kid_layout_w;
use crate::layout::multicol::stack_child::multicol_stack_child;
use crate::layout::options::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

#[allow(clippy::too_many_arguments)]
// Child records keep their layout and paint callbacks with measured geometry.
#[allow(clippy::type_complexity)]
pub(in crate::layout::multicol) fn column_kids(
    _merged: &Computed,
    kids: Vec<(Element, crate::layout::fragment::Shape)>,
    cols: u16,
    _used_gap: f32,
    col_vert: bool,
    rows: Option<crate::layout::fragment::types::Rows>,
    copies: usize,
    fixed: Option<f32>,
    (line_col_w, layout_col_w): (Option<f32>, Option<f32>),
    vcx: ValueCx<'_>,
) -> (
    Vec<Option<Computed>>,
    Vec<usize>,
    Vec<crate::layout::fragment::types::Par>,
    Vec<(Element, crate::layout::fragment::Shape)>,
) {
    // Линии остаются в Taffy-дереве контейнера; копия сохраняет flex sizing,
    // фон и static position. Regrow читает их после layout.
    let n = kids.len();
    let kid_parent = vec![None; n];
    let kid_starts = (0..=n).collect();
    let kid_par = vec![crate::layout::fragment::types::Par::default(); n];
    // `break-inside: avoid` без настоящего монолита — у любого
    // ребёнка колонок (`flow::Par::avoid_only`): с верха колонки
    // коробка выше колонки рвётся, а не переполняет её.
    let mut kid_par = kid_par;
    for (p, (c, _)) in kid_par.iter_mut().zip(kids.iter()) {
        if p.group == 0 {
            p.avoid_only = c.style.break_inside_avoid && avoid_only_monolith(c);
            p.float = c.marks.float_block;
            p.clears = c.style.clear.is_some();
        }
    }
    let mut kids = if col_vert {
        kids
    } else {
        let measured =
            |c: &Element| measured_push(c, kid_layout_w(c, line_col_w, layout_col_w), col_vert);
        grow_pushed(
            kids,
            cols as usize,
            fixed,
            rows,
            copies,
            &kid_par,
            &measured,
            vcx,
        )
    };
    if !col_vert
        && rows.is_none()
        && let Some(height) = fixed.filter(|h| *h > 0.01)
    {
        super::parallel::grow(&mut kids, height, cols as usize, copies, vcx);
    }
    (kid_parent, kid_starts, kid_par, kids)
}

#[allow(clippy::too_many_arguments)]
// The stack is assembled from the same measured child/callback records.
#[allow(clippy::type_complexity)]
pub(in crate::layout::multicol) fn stack_children(
    e: &Element,
    merged: &Computed,
    opts: &RenderOpts,
    cols: u16,
    col_vert: bool,
    col_rl: bool,
    (line_col_w, layout_col_w): (Option<f32>, Option<f32>),
    rows: Option<crate::layout::fragment::types::Rows>,
    nested_auto: std::cell::RefCell<Vec<u64>>,
    nested_whole: std::cell::RefCell<Vec<u64>>,
    measured_kids: std::cell::RefCell<Vec<(u64, f32)>>,
    copies: usize,
    fixed: Option<f32>,
    kid_parent: Vec<Option<Computed>>,
    kid_par: Vec<crate::layout::fragment::types::Par>,
    kids: Vec<(Element, crate::layout::fragment::Shape)>,
    clone_plan: Vec<Vec<(f32, f32)>>,
) -> Vec<crate::layout::fragment::types::StackChild> {
    let vcx = opts.cx.vcx();
    let balanced_frag: Option<f32> =
        (fixed.is_none() && rows.is_none_or(|r| r.cap) && cols > 1 && kids.len() == 1)
            .then(|| {
                let (c, s) = &kids[0];
                (nested_rows_box(c)
                    && matches!(c.style.height, Some(Len::Px(_)))
                    && s.cuts.is_empty()
                    && s.margin_top.abs() < 0.01)
                    .then(|| {
                        let per = s.height / cols as f32;
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
            let lead = if i == 0 {
                s.margin_top
            } else {
                prev_mb.max(s.margin_top)
            };
            let hh = fixed_nest.unwrap_or(0.0);
            v.push((ok && fixed_nest.is_some() && y + lead < hh - 0.01).then_some(y + lead));
            if edge_break(c, false, vcx)
                || edge_break(c, true, vcx)
                || kid_par.get(i).is_some_and(|p| p.group != 0)
                || y + lead + s.height > hh + 0.01
            {
                ok = false;
            }
            y += lead + s.height;
            prev_mb = s.margin_bottom;
        }
        v
    };
    let children: Vec<crate::layout::fragment::types::StackChild> = kids
        .into_iter()
        .enumerate()
        .map(
            |(
                ix,
                (
                    c,
                    crate::layout::fragment::Shape {
                        height: h,
                        margin_top: mt,
                        margin_bottom: mb,
                        cuts,
                        forced,
                        solid,
                    },
                ),
            )| {
                let kid_w = kid_layout_w(&c, line_col_w, layout_col_w);
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
                    kid_w,
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
            },
        )
        .collect();
    children
}
