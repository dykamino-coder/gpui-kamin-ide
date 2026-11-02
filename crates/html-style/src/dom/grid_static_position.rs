//! Статическая позиция абсолюта в сетке: от края содержимого, линии — только у
//! своего содержащего блока.
//!
//! Проход по собранному дереву (css-grid-2 §abspos-items, §static-position):
//! у сетки, не являющейся содержащим блоком, линии `grid-row`/`grid-column`
//! абсолютного ребёнка снимаются; у ребёнка на статической позиции (`auto` с
//! обеих сторон оси) к полям добавляются отступы сетки. Гибкий контейнер не
//! трогается: taffy сам отсчитывает от `content_box_inset`.
use super::Node;
use crate::style::computed::{Display, Position};
use crate::style::values::value::Len;

pub(super) fn adjust(
    admission: &crate::budget::admission::Admission,
    nodes: &mut [Node],
) -> Result<(), crate::budget::admission::Error> {
    crate::dom::traversal::postorder_inherited(
        admission,
        nodes,
        false,
        |el, containing_grid| {
            if crate::style::computed::box_predicates::establishes_cb(el.style.cascaded()) {
                matches!(
                    el.style.cascaded().reset_box_layout.display,
                    Some(Display::Grid | Display::InlineGrid)
                )
            } else {
                *containing_grid
            }
        },
        |el, containing_grid| {
            let grid = matches!(
                el.style.cascaded().reset_box_layout.display,
                Some(Display::Grid | Display::InlineGrid)
            );
            let establishes_cb =
                crate::style::computed::box_predicates::establishes_cb(el.style.cascaded());
            // Гибкий контейнер сюда не входит: taffy (`flexbox.rs`,
            // `perform_absolute_layout_on_absolute_children`) сам отсчитывает
            // статическую позицию от `content_box_inset` (css-flexbox-1 §4.1), и
            // добавочное поле удваивало отбивку контейнера
            // (`flex-abspos-staticpos-margin-001`: коробка на отбивку правее и
            // ниже).
            if !grid {
                return Ok(());
            }
            let static_grid = grid && !establishes_cb && !*containing_grid;
            let pad = el.style.cascaded().reset_box_layout.padding;
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
                // Элемент с заданными линиями стоит не на статической позиции, а в
                // СВОЕЙ области сетки — поля туда добавлять нечего.
                if child.style.cascaded().reset_grid.grid_col.is_some()
                    || child.style.cascaded().reset_grid.grid_row.is_some()
                {
                    continue;
                }
                // `left: auto` — это ОТСУТСТВИЕ края, а не заданный край: именно
                // при `auto` с обеих сторон элемент стоит на статической позиции.
                let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
                let inset = child.style.cascaded().reset_positioned.inset;
                if auto(inset.left) && auto(inset.right) {
                    child.style.cascaded_mut().reset_box_layout.margin.left = add_len(
                        child.style.cascaded().reset_box_layout.margin.left,
                        pad.left,
                    );
                    child.style.cascaded_mut().reset_box_layout.margin.right = add_len(
                        child.style.cascaded().reset_box_layout.margin.right,
                        pad.right,
                    );
                }
                if auto(inset.top) && auto(inset.bottom) {
                    child.style.cascaded_mut().reset_box_layout.margin.top =
                        add_len(child.style.cascaded().reset_box_layout.margin.top, pad.top);
                    child.style.cascaded_mut().reset_box_layout.margin.bottom = add_len(
                        child.style.cascaded().reset_box_layout.margin.bottom,
                        pad.bottom,
                    );
                }
            }
            Ok(())
        },
    )
}

fn add_len(a: Option<Len>, b: Option<Len>) -> Option<Len> {
    match (a, b) {
        (Some(Len::Px(x)), Some(Len::Px(y))) => Some(Len::Px(x + y)),
        (None, b) => b,
        (a, _) => a,
    }
}
