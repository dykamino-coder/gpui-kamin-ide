//! Статическая позиция абсолюта в сетке: от края содержимого, линии — только у
//! своего содержащего блока.
//!
//! Проход по собранному дереву (css-grid-2 §abspos-items, §static-position):
//! у сетки, не являющейся содержащим блоком, линии `grid-row`/`grid-column`
//! абсолютного ребёнка снимаются; у ребёнка на статической позиции (`auto` с
//! обеих сторон оси) к полям добавляются отступы сетки. Гибкий контейнер не
//! трогается: taffy сам отсчитывает от `content_box_inset`.
use super::normalization::{Transform, apply};
use super::{Element, Node};
use crate::budget::admission::{Admission, Error};
use crate::style::computed::{Display, Position};
use crate::style::values::value::Len;

pub(super) fn adjust(admission: &Admission, nodes: Vec<Node>) -> Result<Vec<Node>, Error> {
    apply(&GridStaticPosition, nodes, false, admission)
}

pub(crate) struct GridStaticPosition;

impl Transform for GridStaticPosition {
    type Context = bool;

    fn enter(
        &self,
        el: &mut Element,
        containing_grid: &bool,
        _: &Admission,
    ) -> Result<bool, Error> {
        Ok(
            if crate::style::computed::box_predicates::establishes_cb(el.style.cascaded()) {
                matches!(
                    el.style.cascaded().reset_box_layout.display,
                    Some(Display::Grid | Display::InlineGrid)
                )
            } else {
                *containing_grid
            },
        )
    }

    fn leave(&self, el: &mut Element, containing_grid: &bool, _: &Admission) -> Result<(), Error> {
        let grid = matches!(
            el.style.cascaded().reset_box_layout.display,
            Some(Display::Grid | Display::InlineGrid)
        );
        let establishes_cb =
            crate::style::computed::box_predicates::establishes_cb(el.style.cascaded());
        // Taffy's flex absolute-position layout already includes content-box
        // insets (CSS Flexbox 1 section 4.1); adding them here doubles padding.
        if !grid {
            return Ok(());
        }
        let static_grid = grid && !establishes_cb && !*containing_grid;
        let pad = el.style.cascaded().reset_box_layout.padding.clone();
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else {
                continue;
            };
            if child.style.cascaded().reset_positioned.position != Some(Position::Absolute) {
                continue;
            }
            // CSS Grid 2 sections abspos-items/static-position: placement lines
            // apply only to a grid containing block. Retain them when an outer
            // grid is that block; this pass must not discard its placement.
            if static_grid {
                child.style.cascaded_mut().reset_grid.grid_col = None;
                child.style.cascaded_mut().reset_grid.grid_row = None;
            }
            // Explicit placement uses its grid area rather than static position.
            if child.style.cascaded().reset_grid.grid_col.is_some()
                || child.style.cascaded().reset_grid.grid_row.is_some()
            {
                continue;
            }
            // Both absent and explicit auto insets use static positioning.
            let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
            let inset = child.style.cascaded().reset_positioned.inset.clone();
            if auto(inset.left) && auto(inset.right) {
                child.style.cascaded_mut().reset_box_layout.margin.left = add_len(
                    child.style.cascaded().reset_box_layout.margin.left.clone(),
                    pad.left.clone(),
                );
                child.style.cascaded_mut().reset_box_layout.margin.right = add_len(
                    child.style.cascaded().reset_box_layout.margin.right.clone(),
                    pad.right.clone(),
                );
            }
            if auto(inset.top) && auto(inset.bottom) {
                child.style.cascaded_mut().reset_box_layout.margin.top = add_len(
                    child.style.cascaded().reset_box_layout.margin.top.clone(),
                    pad.top.clone(),
                );
                child.style.cascaded_mut().reset_box_layout.margin.bottom = add_len(
                    child
                        .style
                        .cascaded()
                        .reset_box_layout
                        .margin
                        .bottom
                        .clone(),
                    pad.bottom.clone(),
                );
            }
        }
        Ok(())
    }
}

fn add_len(a: Option<Len>, b: Option<Len>) -> Option<Len> {
    match (a, b) {
        (Some(Len::Px(x)), Some(Len::Px(y))) => Some(Len::Px(x + y)),
        (None, b) => b,
        (a, _) => a,
    }
}
