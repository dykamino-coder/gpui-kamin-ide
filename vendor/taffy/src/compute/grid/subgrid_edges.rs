//! Subgrid edges phase. CSS Grid 2 §9.

use crate::geometry::Rect;
use crate::geometry::Size;
use crate::style::Overflow;
use crate::style::SUBGRID_COLUMNS;
use crate::style::SUBGRID_COLUMN_GAP_NORMAL;
use crate::style::SUBGRID_ROWS;
use crate::style::SUBGRID_ROW_GAP_NORMAL;
use crate::tree::LayoutPartialTreeExt;
use crate::tree::NodeId;
use crate::util::ResolveOrZero;
use crate::CoreStyle;
use crate::GridContainerStyle;
use crate::LayoutGridContainer;

/// Края подсетки по физическим сторонам: поле + рамка + отбивка (+ жёлоб
/// полосы прокрутки) — css-grid-2 §subgrid-margins: «the sum of the subgrid's
/// margin, padding, scrollbar gutter, and border at each edge». Доли — от
/// ширины области содержимого контейнера (`auto`-поле — ноль).
pub(in super::super) fn subgrid_edges<Tree: LayoutGridContainer>(
    tree: &Tree,
    style: &Tree::GridContainerStyle<'_>,
    basis: Option<f32>,
) -> Rect<f32> {
    let basis = basis.or(Some(0.0));
    let margin = style
        .margin()
        .map(|m| m.resolve_or_zero(basis, |val, b| tree.calc(val, b)));
    let border = style
        .border()
        .map(|b| b.resolve_or_zero(basis, |val, bb| tree.calc(val, bb)));
    let padding = style
        .padding()
        .map(|p| p.resolve_or_zero(basis, |val, bb| tree.calc(val, bb)));
    let overflow = style.overflow();
    let scrollbar = style.scrollbar_width();
    Rect {
        left: margin.left + border.left + padding.left,
        right: margin.right
            + border.right
            + padding.right
            + if overflow.y == Overflow::Scroll {
                scrollbar
            } else {
                0.0
            },
        top: margin.top + border.top + padding.top,
        bottom: margin.bottom
            + border.bottom
            + padding.bottom
            + if overflow.x == Overflow::Scroll {
                scrollbar
            } else {
                0.0
            },
    }
}

/// Использованный зазор подсетки по осям: `normal` — зазор родителя
/// (css-grid-2 §subgrid-gaps: «A value of `normal` indicates that the subgrid
/// has the same size gutters as its parent grid»).
pub(in super::super) fn subgrid_gaps<Tree: LayoutGridContainer>(
    tree: &Tree,
    style: &Tree::GridContainerStyle<'_>,
    bits: u8,
    parent_gap: Size<f32>,
    basis: Size<Option<f32>>,
) -> Size<f32> {
    let own = style.gap();
    Size {
        width: if bits & SUBGRID_COLUMN_GAP_NORMAL != 0 {
            parent_gap.width
        } else {
            own.width
                .resolve_or_zero(basis.width, |val, b| tree.calc(val, b))
        },
        height: if bits & SUBGRID_ROW_GAP_NORMAL != 0 {
            parent_gap.height
        } else {
            own.height
                .resolve_or_zero(basis.height, |val, b| tree.calc(val, b))
        },
    }
}

/// Биты подсеточных осей ребёнка, которые сетка вправду связывает со своими
/// дорожками. Лунки-подсетка (`grid-lanes` с `subgrid`) раскладывается своим
/// кодом — её элементы сюда не сплющиваются.
pub(in super::super) fn linked_axes<Tree: LayoutGridContainer>(tree: &Tree, node: NodeId) -> u8 {
    let style = tree.get_grid_container_style(node);
    if style.grid_lanes().is_some() {
        return 0;
    }
    style.subgrid() & (SUBGRID_COLUMNS | SUBGRID_ROWS)
}
