//! Сбор строк flex-ряда по гипотетическим главным размерам (css-flexbox-1 §9.3 шаг 5): элемент
//! шире контейнера занимает строку один и сжимается по §9.7 до известного min-content, но
//! собирается по базе. Копия элемента получает ширину в точках; порядок — `order`.
//! Элементы с ростом, ограничениями ширины, `auto`-полями по поперечной оси и иным
//! выравниванием выводят из этого пути (`None`).

use super::super::items::row_item_width;
use super::geometry::row_container;
use super::intrinsic::intrinsic_main;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::box_kind::out_of_flow;
use crate::style::computed::Align;
use crate::style::values::value::{Len, px_auto_zero};

pub(super) type Lines = Vec<Vec<(usize, Element)>>;

pub(super) fn collect_lines(c: &Element, main: f32, depth: u8) -> Option<Lines> {
    if depth == 0 || !row_container(c) || main <= 0.0 {
        return None;
    }
    let gap = px_auto_zero(c.style.gap.and_then(|g| g.1))?.max(0.0);
    let mut items = Vec::new();
    for (source, n) in c.children.iter().enumerate().filter(|(_, n)| !is_blank(n)) {
        let Node::Element(k) = n else { return None };
        if k.style.display == Some(crate::style::computed::Display::None) {
            continue;
        }
        let s = &k.style;
        if k.inline
            || out_of_flow(s)
            || s.float.unwrap_or(0) != 0
            || s.flex_grow.is_some_and(|g| g > 0.0)
            || s.min_width.is_some()
            || s.max_width.is_some()
            // Cross-axis auto margins absorb free space instead of stretching
            // the item (§9.6). Bounded stretch also needs max-size resolution.
            || s.margin.top == Some(Len::Auto)
            || s.margin.bottom == Some(Len::Auto)
            || (matches!(s.height, None | Some(Len::Auto)) && s.max_height.is_some())
            || !matches!(
                s.align_self,
                None | Some(Align::Stretch) | Some(Align::Start)
            )
            || s.basis_content == Some(true)
        {
            return None;
        }
        let definite = row_item_width(s, main);
        let (hypothetical, minimum) = match definite {
            Some(w) => (w, w),
            None if matches!(s.width, None | Some(Len::Auto))
                && matches!(s.flex_basis, None | Some(Len::Auto)) =>
            {
                intrinsic_main(k, main, depth - 1)?
            }
            None => return None,
        };
        let b = s.borders();
        let edges = if s.border_box == Some(true) {
            0.0
        } else {
            px_auto_zero(s.padding.left)?
                + px_auto_zero(s.padding.right)?
                + px_auto_zero(b.left)?
                + px_auto_zero(b.right)?
        } + px_auto_zero(s.margin.left)?
            + px_auto_zero(s.margin.right)?;
        let outer = hypothetical + edges;
        let actual = if outer > main + 0.01 {
            // This item necessarily occupies a line alone. Its known intrinsic
            // minimum lets §9.7 shrink it, but §9.3 still collects by its base.
            if definite.is_some() || s.flex_shrink == Some(0.0) || minimum + edges > main + 0.01 {
                return None;
            }
            (main - edges).max(minimum)
        } else {
            hypothetical
        };
        let mut element = k.clone();
        element.style.width = Some(Len::Px(actual));
        element.style.flex_basis = None;
        items.push((outer, source, element));
    }
    items.sort_by_key(|(_, _, item)| item.style.order.unwrap_or(0));
    let mut lines: Lines = Vec::new();
    let mut used = 0.0;
    for (outer, source, item) in items {
        if !lines.is_empty() && used + gap + outer <= main + 0.01 {
            lines.last_mut()?.push((source, item));
            used += gap + outer;
        } else {
            lines.push(vec![(source, item)]);
            used = outer;
        }
    }
    Some(lines)
}
