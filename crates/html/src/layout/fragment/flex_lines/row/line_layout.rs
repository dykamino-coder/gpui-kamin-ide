//! Сбор строк flex-ряда по гипотетическим главным размерам (css-flexbox-1 §9.3 шаг 5): элемент
//! шире контейнера занимает строку один и сжимается по §9.7 до известного min-content, но
//! собирается по базе. Копия элемента получает ширину в точках; порядок — `order`.
//! Элементы с ростом, ограничениями ширины, `auto`-полями по поперечной оси и иным
//! выравниванием выводят из этого пути (`None`).

use super::super::items::row_item_width;
use super::container::row_container;
use super::intrinsic::intrinsic_main;
use crate::dom::whitespace::is_blank;
use crate::dom::{Element, Node};
use crate::layout::block::sizing::BoxDimensions;
use crate::layout::box_kind::out_of_flow;
use crate::style::computed::Align;
use crate::style::values::value::{Len, px_auto_zero};

pub(super) type Lines = Vec<Vec<(usize, Element)>>;

pub(super) fn collect_lines(c: &Element, main: f32, depth: u8) -> Option<Lines> {
    if depth == 0 || !row_container(c) || main <= 0.0 {
        return None;
    }
    let gap = px_auto_zero(c.style.used().reset_flex.gap.and_then(|g| g.1))?.max(0.0);
    let mut items = Vec::new();
    for (source, n) in c.children.iter().enumerate().filter(|(_, n)| !is_blank(n)) {
        let Node::Element(k) = n else { return None };
        if k.style.used().reset_box_layout.display == Some(crate::style::computed::Display::None) {
            continue;
        }
        let s = &*k.style.used();
        let sizes = BoxDimensions::for_element(k, s);
        if k.inline
            || out_of_flow(s)
            || s.reset_box_layout.float.unwrap_or(0) != 0
            || s.reset_flex.flex_grow.is_some_and(|g| g > 0.0)
            || s.reset_box_layout.min_width.is_some()
            || s.reset_box_layout.max_width.is_some()
            // Cross-axis auto margins absorb free space instead of stretching
            // the item (§9.6). Bounded stretch also needs max-size resolution.
            || s.reset_box_layout.margin.top == Some(Len::Auto)
            || s.reset_box_layout.margin.bottom == Some(Len::Auto)
            || (matches!(s.reset_box_layout.height, None | Some(Len::Auto)) && sizes.maximum[1].is_some())
            || !matches!(
                s.reset_flex.align_self,
                None | Some(Align::Stretch) | Some(Align::Start)
            )
            || s.reset_flex.basis_content == Some(true)
        {
            return None;
        }
        let definite = row_item_width(s, main);
        let (hypothetical, minimum) = match definite {
            Some(w) => (w, w),
            None if matches!(s.reset_box_layout.width, None | Some(Len::Auto))
                && matches!(s.reset_flex.flex_basis, None | Some(Len::Auto)) =>
            {
                intrinsic_main(k, main, depth - 1)?
            }
            None => return None,
        };
        let b = s.borders();
        let edges = if sizes.border_box {
            0.0
        } else {
            px_auto_zero(s.reset_box_layout.padding.left)?
                + px_auto_zero(s.reset_box_layout.padding.right)?
                + px_auto_zero(b.left)?
                + px_auto_zero(b.right)?
        } + px_auto_zero(s.reset_box_layout.margin.left)?
            + px_auto_zero(s.reset_box_layout.margin.right)?;
        let outer = hypothetical + edges;
        let actual = if outer > main + 0.01 {
            // This item necessarily occupies a line alone. Its known intrinsic
            // minimum lets §9.7 shrink it, but §9.3 still collects by its base.
            if definite.is_some()
                || s.reset_flex.flex_shrink == Some(0.0)
                || minimum + edges > main + 0.01
            {
                return None;
            }
            (main - edges).max(minimum)
        } else {
            hypothetical
        };
        let mut element = k.clone();
        element.style.used_mut().reset_box_layout.width = Some(Len::Px(actual));
        element.style.used_mut().reset_flex.flex_basis = None;
        items.push((outer, source, element));
    }
    items.sort_by_key(|(_, _, item)| item.style.used().reset_flex.order.unwrap_or(0));
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
