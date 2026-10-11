//! Статические места позиционированных детей, div стопки колонок и его завершение.

use crate::dom::{Element, Node};
use crate::layout::multicol::spanner::{intrinsic_inline_size, spanner_box};
use crate::layout::positioned::predicates::edge_set;
use crate::render::{RenderOpts, element};
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, Styled};

pub(super) fn place_oof_statics(
    merged: &Computed,
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
                edge_set(oof.style.inset.left) || edge_set(oof.style.inset.right),
                edge_set(oof.style.inset.top) || edge_set(oof.style.inset.bottom),
            ),
            rtl: merged.rtl == Some(true),
            vertical: merged.vertical == Some(true),
            vertical_rl: merged.vertical_rl == Some(true),
            own_vertical: oof.style.vertical == Some(true),
            ..Default::default()
        });
        let probe = crate::layout::fragment::types::StackChild {
            measure: None,
            el: crate::layout::positioned::containing_block::spot_probe(oof_spots[i].clone(), true),
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
    gap_items: &Option<std::rc::Rc<std::cell::RefCell<Vec<gpui::Bounds<gpui::Pixels>>>>>,
    rule: Option<(f32, gpui::Hsla)>,
    children: Vec<crate::layout::fragment::types::StackChild>,
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
            intrinsic_inline_size(&e.style, inherited).then_some({
                crate::layout::fragment::types::Intrinsic(match column_width {
                    Some(Len::Px(w)) if w > 0.0 => Some(w),
                    _ => None,
                })
            }),
        )
        .with_axis(col_axis)
        .with_fill_shrink(
            fixed.is_some()
                && rows.is_none()
                && nest_rows.is_none()
                && !col_vert
                && e.style.column_height.is_none()
                && matches!(e.style.height, None | Some(Len::Auto)),
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
                    && e.style.border_box != Some(true)
                    && !e
                        .children
                        .iter()
                        .any(|n| matches!(n, Node::Element(c) if spanner_box(c))) =>
            {
                Some(h)
            }
            _ => None,
        }),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn finish_column_stack(
    e: &Element,
    merged: Computed,
    opts: &RenderOpts,
    col_vert: bool,
    oof_static: Vec<(usize, Element)>,
    gap_spec: Option<crate::paint::gap_rules::GapRuleSpec>,
    gap_items: Option<std::rc::Rc<std::cell::RefCell<Vec<gpui::Bounds<gpui::Pixels>>>>>,
    oof_spots: Vec<std::rc::Rc<std::cell::Cell<crate::layout::positioned::containing_block::Spot>>>,
    mut d: gpui::Div,
) -> AnyElement {
    let cb_h: Option<f32> = (e.style.position.is_some()
        && e.style.position != Some(crate::style::computed::Position::Static)
        && !col_vert)
        .then(|| {
            let px = |l: Option<Len>| match l {
                None => Some(0.0),
                Some(Len::Px(v)) => Some(v),
                _ => None,
            };
            let b = e.style.borders();
            let pad = px(e.style.padding.top)? + px(e.style.padding.bottom)?;
            let bor = px(b.top)? + px(b.bottom)?;
            match e.style.height {
                Some(Len::Px(h)) if e.style.border_box == Some(true) => Some((h - bor).max(pad)),
                Some(Len::Px(h)) => Some(h.max(0.0) + pad),
                _ => None,
            }
        })
        .flatten();
    for (i, (_, oof)) in oof_static.iter().enumerate() {
        let mut oof = oof.clone();
        if let Some(ch) = cb_h
            && oof.style.position == Some(crate::style::computed::Position::Absolute)
        {
            for l in [
                &mut oof.style.height,
                &mut oof.style.min_height,
                &mut oof.style.max_height,
            ] {
                if let Some(Len::Pct(k)) = *l {
                    *l = Some(Len::Px(k * ch));
                }
            }
        }
        d = d.child(crate::layout::positioned::containing_block::spot_place(
            oof_spots[i].clone(),
            element(&oof, &merged, opts),
        ));
    }
    if let (Some(buf), Some(spec)) = (gap_items, gap_spec) {
        d = d.child(
            crate::paint::gap_rules::painter::GapRulePainter::new(buf, spec).into_any_element(),
        );
    }
    d.into_any_element()
}
