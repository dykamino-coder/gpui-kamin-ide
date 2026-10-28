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

pub(super) fn adjust(nodes: &mut [Node]) {
    walk(nodes, false);
}

fn walk(nodes: &mut [Node], containing_grid: bool) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        let grid = matches!(
            el.style.cascaded().display,
            Some(Display::Grid | Display::InlineGrid)
        );
        let establishes_cb =
            crate::style::computed::box_predicates::establishes_cb(el.style.cascaded());
        walk(
            &mut el.children,
            if establishes_cb {
                grid
            } else {
                containing_grid
            },
        );
        // Гибкий контейнер сюда не входит: taffy (`flexbox.rs`,
        // `perform_absolute_layout_on_absolute_children`) сам отсчитывает
        // статическую позицию от `content_box_inset` (css-flexbox-1 §4.1), и
        // добавочное поле удваивало отбивку контейнера
        // (`flex-abspos-staticpos-margin-001`: коробка на отбивку правее и
        // ниже).
        if !grid {
            continue;
        }
        let static_grid = grid && !establishes_cb && !containing_grid;
        let pad = el.style.cascaded().padding;
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else {
                continue;
            };
            if child.style.cascaded().position != Some(Position::Absolute) {
                continue;
            }
            // CSS Grid 2 sections abspos-items/static-position: placement lines
            // apply only to a grid containing block. Retain them when an outer
            // grid is that block; this pass must not discard its placement.
            if static_grid {
                child.style.cascaded_mut().grid_col = None;
                child.style.cascaded_mut().grid_row = None;
            }
            // Элемент с заданными линиями стоит не на статической позиции, а в
            // СВОЕЙ области сетки — поля туда добавлять нечего.
            if child.style.cascaded().grid_col.is_some()
                || child.style.cascaded().grid_row.is_some()
            {
                continue;
            }
            // `left: auto` — это ОТСУТСТВИЕ края, а не заданный край: именно
            // при `auto` с обеих сторон элемент стоит на статической позиции.
            let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
            let inset = child.style.cascaded().inset;
            if auto(inset.left) && auto(inset.right) {
                child.style.cascaded_mut().margin.left =
                    add_len(child.style.cascaded().margin.left, pad.left);
                child.style.cascaded_mut().margin.right =
                    add_len(child.style.cascaded().margin.right, pad.right);
            }
            if auto(inset.top) && auto(inset.bottom) {
                child.style.cascaded_mut().margin.top =
                    add_len(child.style.cascaded().margin.top, pad.top);
                child.style.cascaded_mut().margin.bottom =
                    add_len(child.style.cascaded().margin.bottom, pad.bottom);
            }
        }
    }
}

fn add_len(a: Option<Len>, b: Option<Len>) -> Option<Len> {
    match (a, b) {
        (Some(Len::Px(x)), Some(Len::Px(y))) => Some(Len::Px(x + y)),
        (None, b) => b,
        (a, _) => a,
    }
}
