//! План клонов (box-decoration-break: clone) по колонкам и правило колонок (column-rule).

use crate::document::state::ValueCx;
use crate::dom::Element;
use crate::layout::fragment::ShapeCx;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::{clone_dec, solid_box};
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::probe::plain_block_tree;
use crate::layout::fragment::table_bands::repeat_leads;
use crate::layout::multicol::spanner::parallel_items_inside;
use crate::layout::page::paged::visible_overflow;
use crate::render::RenderOpts;
use crate::style::computed::Computed;
use crate::style::values::value::Len;

// The clone plan preserves each child's measured shape and painting callback.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn clone_plan_of(
    cols: u16,
    col_vert: bool,
    rows: Option<crate::layout::fragment::types::Rows>,
    copies: usize,
    fixed: Option<f32>,
    kid_par: &[crate::layout::fragment::types::Par],
    kids: &[(Element, crate::layout::fragment::Shape)],
    vcx: ValueCx<'_>,
) -> Vec<Vec<(f32, f32)>> {
    let clone_plan: Vec<Vec<(f32, f32)>> =
        if !col_vert && kids.iter().any(|(c, _)| clone_dec(c).is_some()) {
            let probe: Vec<crate::layout::fragment::types::Kid> = kids
                .iter()
                .enumerate()
                .map(|(pi, (c, s))| {
                    let mut m = c.clone();
                    m.style.margin.top = None;
                    m.style.margin.bottom = None;
                    let (over, cuts, forced, solid) = match shape_full(
                        &m,
                        4,
                        ShapeCx {
                            unclamped: true,
                            ..ShapeCx::COLUMNS
                        },
                        vcx,
                    )
                    .filter(|_| {
                        fixed.is_some() && plain_block_tree(&m, 4) && visible_overflow(&m.style)
                    })
                    .filter(|u| u.height > s.height + 0.01)
                    {
                        Some(u) => (u.height, u.cuts, u.forced, u.solid),
                        None => (s.height, s.cuts.clone(), s.forced.clone(), s.solid.clone()),
                    };
                    crate::layout::fragment::types::Kid {
                        h: s.height,
                        mt: s.margin_top,
                        mb: s.margin_bottom,
                        monolith: solid_box(c),
                        cuts,
                        force_before: edge_break(c, false, vcx),
                        force_after: edge_break(c, true, vcx),
                        avoid_before: edge_avoid(c, false, vcx),
                        avoid_after: edge_avoid(c, true, vcx),
                        forced,
                        solid,
                        span: c.style.column_span == Some(true) && !c.inline,
                        over,
                        clone_dec: clone_dec(c),
                        // Тот же предикат, что у `StackChild` ниже:
                        // иначе план соседа разошёлся бы с укладкой.
                        overflow_top: fixed.is_some()
                            && rows.is_none()
                            && !parallel_items_inside(c, 4, vcx),
                        repeat: repeat_leads(c, fixed, rows, vcx),
                        par: kid_par[pi],
                    }
                })
                .collect();
            crate::layout::multicol::column_stack::ColumnStack::frags_of(
                &probe,
                cols as usize,
                fixed,
                rows,
                copies,
            )
        } else {
            Vec::new()
        };
    clone_plan
}

pub(super) fn column_rule_of(
    e: &Element,
    merged: &Computed,
    opts: &RenderOpts,
) -> Option<(f32, gpui::Hsla)> {
    if e.style.column_rule_visible == Some(true) {
        Some((
            match e.style.column_rule_width {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => {
                    k * match e.style.font_size {
                        Some(Len::Px(fs)) => fs,
                        _ => opts.base_size(),
                    }
                }
                _ => 3.0,
            },
            e.style
                .column_rule_color
                .or(merged.color)
                .unwrap_or(crate::style::values::value::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                })
                .to_hsla(),
        ))
    } else {
        None
    }
}
