//! Статические места позиционированных детей, div стопки колонок и его завершение.

use crate::dom::{Element, Node};
use crate::hooks::IntoHooked;
use crate::layout::multicol::spanner::{intrinsic_inline_size, spanner_box};
use crate::layout::options::RenderOpts;
use crate::layout::positioned::predicates::edge_set;
use crate::style::computed::{BoxStyle, Computed};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled};

pub(super) fn place_oof_statics(
    merged: &Computed,
    opts: &RenderOpts,
    oof_static: &[(usize, Element)],
    kid_starts: Vec<usize>,
    children: Vec<crate::layout::fragment::types::StackChild>,
) -> (
    Vec<crate::layout::fragment::types::StackChild>,
    Vec<std::rc::Rc<std::cell::Cell<crate::layout::positioned::containing_block::Spot>>>,
) {
    let mut children = children;
    let oof_spots: Vec<crate::layout::positioned::containing_block::SpotCell> =
        oof_static.iter().map(|_| Default::default()).collect();
    for (i, (at, oof)) in oof_static.iter().enumerate().rev() {
        // Заданную ось считает раскладка от содержащего
        // блока, щуп правит только ПУСТУЮ (CSS 2.1
        // §10.3.7) — тот же гейт `fixed_axes`, что у слоёв
        // в `blocks()`.
        oof_spots[i].set(crate::layout::positioned::containing_block::Spot {
            fixed_axes: (
                edge_set(oof.style.computed().inset.left)
                    || edge_set(oof.style.computed().inset.right),
                edge_set(oof.style.computed().inset.top)
                    || edge_set(oof.style.computed().inset.bottom),
            ),
            rtl: merged.wm().rtl(),
            vertical: merged.wm().vertical(),
            vertical_rl: merged.wm().block_rl(),
            own_vertical: oof.style.cascaded().wm().vertical(),
            ..Default::default()
        });
        let probe = crate::layout::fragment::types::StackChild {
            #[cfg(feature = "wpt-harness")]
            probe_pending: false,
            #[cfg(feature = "wpt-harness")]
            probe: opts.frag.probe.clone(),
            watched: None,
            breaks_w: None,
            regrow: None,
            measure: None,
            el: crate::layout::positioned::containing_block::spot_probe(
                oof_spots[i].clone(),
                true,
                None,
                &opts.cx,
            ),
            frags: Vec::new(),
            monolith: false,
            fit_whole: false,
            cuts: Vec::new(),
            force_before: false,
            force_after: false,
            avoid_before: false,
            avoid_after: false,
            forced: Vec::new(),
            solid: Vec::new(),
            avoid: Vec::new(),
            keep: Vec::new(),
            h: 0.0,
            mt: 0.0,
            mb: 0.0,
            span: false,
            over: 0.0,
            rel: (0.0, 0.0),
            clone_dec: None,
            overflow_top: false,
            nested_cols: false,
            repeat: None,
            par: crate::layout::fragment::types::Par::default(),
            slack: None,
            laid_w: Default::default(),
            paginate: None,
            positioned: false,
        };
        // Номер — среди ДЕТЕЙ ДО раскрытия строк flex (`split_flex_lines`).
        let at = kid_starts
            .get(*at)
            .copied()
            .unwrap_or(children.len())
            .min(children.len());
        children.insert(at, probe);
    }
    (children, oof_spots)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn column_stack_div(
    d: gpui::Div,
    e: &Element,
    inherited: &Computed,
    merged: &Computed,
    cols: u16,
    column_width: Option<Len>,
    used_gap: f32,
    col_axis: crate::layout::fragment::types::StackAxis,
    col_vert: bool,
    col_rl: bool,
    rows: Option<crate::layout::fragment::types::Rows>,
    nest_rows: Option<f32>,
    nest_phase: f32,
    fixed: Option<f32>,
    gap_items: &Option<crate::layout::multicol::gap_rules::GapItems>,
    rule: Option<(f32, gpui::Hsla)>,
    children: Vec<crate::layout::fragment::types::StackChild>,
    opts: &RenderOpts,
    deferred: Option<std::rc::Rc<super::super::deferred::Recipe>>,
) -> gpui::Div {
    let d = if col_vert {
        let d = d.flex();
        if col_rl {
            d.flex_row_reverse()
        } else {
            d.flex_row()
        }
    } else {
        d
    };

    d.child(
        crate::layout::multicol::column_stack::ColumnStack::new(
            children,
            cols as usize,
            used_gap,
            fixed,
            rule,
            rows,
            gap_items.clone(),
            intrinsic_inline_size(&e.style.computed(), inherited).then_some({
                crate::layout::fragment::types::Intrinsic(match column_width {
                    Some(Len::Px(w)) if w > 0.0 => Some(w),
                    _ => None,
                })
            }),
            opts.cx.deferred.paint.clone(),
        )
        .with_deferred(deferred)
        .with_static_coordinates(crate::layout::fragment::static_coordinates::needed(e))
        .with_axis(col_axis)
        .with_fill_shrink(
            fixed.is_some()
                && rows.is_none()
                && nest_rows.is_none()
                && !col_vert
                && e.style.computed().column_height.is_none()
                && matches!(e.style.computed().height, None | Some(Len::Auto)),
        )
        .with_row_phase(if nest_rows.is_some() { nest_phase } else { 0.0 })
        // Линейки последней линии — до низа содержимого коробки
        // заданной высоты (Blink `PaintColumnRules`), без
        // спаннеров и рядов (`multicol-rule-nested-balancing-001`).
        .with_rule_stretch(match merged.height {
            Some(Len::Px(h))
                if h > 0.0
                    && !col_vert
                    && nest_rows.is_none()
                    && e.style.computed().border_box != Some(true)
                    && !e
                        .children
                        .iter()
                        .any(|n| matches!(n, Node::Element(c) if spanner_box(c))) =>
            {
                Some(h)
            }
            _ => None,
        })
        .into_hooked(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn finish_column_stack(
    e: &Element,
    merged: BoxStyle,
    opts: &RenderOpts,
    col_vert: bool,
    oof_static: Vec<(usize, Element)>,
    oof_spots: Vec<std::rc::Rc<std::cell::Cell<crate::layout::positioned::containing_block::Spot>>>,
    mut d: gpui::Div,
) -> AnyElement {
    let cb_h: Option<f32> = (e.style.computed().position.is_some()
        && e.style.computed().position != Some(crate::style::computed::Position::Static)
        && !col_vert)
        .then(|| {
            let px = |l: Option<Len>| match l {
                None => Some(0.0),
                Some(Len::Px(v)) => Some(v),
                _ => None,
            };
            let b = e.style.computed().borders();
            let pad = px(e.style.computed().padding.top)? + px(e.style.computed().padding.bottom)?;
            let bor = px(b.top)? + px(b.bottom)?;
            match e.style.computed().height {
                Some(Len::Px(h)) if e.style.computed().border_box == Some(true) => {
                    Some((h - bor).max(pad))
                }
                Some(Len::Px(h)) => Some(h.max(0.0) + pad),
                _ => None,
            }
        })
        .flatten();
    for (i, (_, oof)) in oof_static.iter().enumerate() {
        let mut oof = oof.clone();
        if let Some(ch) = cb_h
            && oof.style.computed().position == Some(crate::style::computed::Position::Absolute)
        {
            let style = &mut *oof.style.used_mut();
            for l in [
                &mut style.height,
                &mut style.min_height,
                &mut style.max_height,
            ] {
                if let Some(Len::Pct(k)) = *l {
                    *l = Some(Len::Px(k * ch));
                }
            }
        }
        d = d.child(crate::layout::positioned::containing_block::spot_place(
            oof_spots[i].clone(),
            opts.cx.contracts.build.element(&oof, &merged, opts),
        ));
    }
    d.into_any_element()
}
